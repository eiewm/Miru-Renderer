use crate::modes::mania::judgment::{ScoreJudgmentEvent, ScoreJudgmentPart};
use crate::types::{HitObject, JudgmentKind};
use crate::utils::mods::{AdaptivePlaybackProfile, AdaptivePlaybackSegment, PlaybackRateProfile};
use std::collections::VecDeque;
const ADAPTIVE_DAMP_HALF_TIME_MS: f64 = 50.0;
const ADAPTIVE_RECENT_RATE_COUNT: usize = 8;
const ADAPTIVE_MIN_RATE: f64 = 0.4;
const ADAPTIVE_MAX_RATE: f64 = 2.5;
const ADAPTIVE_MIN_RATE_CHANGE: f64 = 0.9;
const ADAPTIVE_MAX_RATE_CHANGE: f64 = 1.11;
const ADAPTIVE_MISS_RATE_CHANGE: f64 = 0.95;
const ADAPTIVE_BINARY_SEARCH_EPSILON_MS: f64 = 0.001;
#[derive(Debug, Clone)]
pub struct PlaybackClock {
    timeline_start_ms: i32,
    profile: PlaybackRateProfile,
}
impl PlaybackClock {
    pub fn new(timeline_start_ms: i32, profile: PlaybackRateProfile) -> Self {
        Self {
            timeline_start_ms,
            profile: sanitize_profile(profile),
        }
    }
    #[inline]
    pub fn profile(&self) -> &PlaybackRateProfile {
        &self.profile
    }
    #[inline]
    pub fn clock_rate(&self) -> f64 {
        self.profile.initial_rate()
    }
    #[inline]
    pub fn rate_at_beatmap_time_ms(&self, beatmap_time_ms: f64) -> f64 {
        self.profile.rate_at_beatmap_time_ms(beatmap_time_ms)
    }
    pub fn beatmap_time_for_output_elapsed_ms(&self, output_elapsed_ms: f64) -> f64 {
        let output_elapsed_ms = output_elapsed_ms.max(0.0);
        if self.profile.uses_wall_clock_preroll() {
            let preempt_ms = if self.timeline_start_ms < 0 {
                (-self.timeline_start_ms) as f64
            } else {
                0.0
            };
            // Negative timeline preroll plays in wall-clock time before rate changes start at beatmap 0.
            if preempt_ms > 0.0 && output_elapsed_ms <= preempt_ms {
                self.timeline_start_ms as f64 + output_elapsed_ms
            } else if preempt_ms > 0.0 {
                self.profile
                    .beatmap_time_after_output_elapsed_ms(0.0, output_elapsed_ms - preempt_ms)
            } else {
                self.profile.beatmap_time_after_output_elapsed_ms(
                    self.timeline_start_ms as f64,
                    output_elapsed_ms,
                )
            }
        } else {
            self.profile.beatmap_time_after_output_elapsed_ms(
                self.timeline_start_ms as f64,
                output_elapsed_ms,
            )
        }
    }
    pub fn beatmap_time_ms_for_output_elapsed(&self, output_elapsed_ms: f64) -> i32 {
        self.beatmap_time_for_output_elapsed_ms(output_elapsed_ms) as i32
    }
    pub fn output_elapsed_ms_for_beatmap_time(&self, beatmap_time_ms: f64) -> f64 {
        let beatmap_time_ms = beatmap_time_ms.max(self.timeline_start_ms as f64);
        if self.profile.uses_wall_clock_preroll() {
            let preempt_ms = if self.timeline_start_ms < 0 {
                (-self.timeline_start_ms) as f64
            } else {
                0.0
            };
            if preempt_ms > 0.0 && beatmap_time_ms <= 0.0 {
                (beatmap_time_ms - self.timeline_start_ms as f64).max(0.0)
            } else if preempt_ms > 0.0 {
                preempt_ms + self.profile.output_elapsed_ms_between(0.0, beatmap_time_ms)
            } else {
                self.profile
                    .output_elapsed_ms_between(self.timeline_start_ms as f64, beatmap_time_ms)
                    .max(0.0)
            }
        } else {
            self.profile
                .output_elapsed_ms_between(self.timeline_start_ms as f64, beatmap_time_ms)
                .max(0.0)
        }
    }
    pub fn output_preroll_ms(&self) -> f64 {
        self.output_elapsed_ms_for_beatmap_time(0.0)
    }
    pub fn source_duration_ms_for_output_duration(&self, output_duration_ms: f64) -> f64 {
        self.profile
            .beatmap_time_after_output_elapsed_ms(0.0, output_duration_ms.max(0.0))
    }
    pub fn output_seconds_expr_for_beatmap_time_expr(&self, beatmap_time_expr_sec: &str) -> String {
        let beatmap_time_expr_sec = format!("({beatmap_time_expr_sec})");
        if matches!(self.profile, PlaybackRateProfile::Adaptive { .. }) {
            let rate = self.profile.initial_rate();
            if self.timeline_start_ms < 0 {
                let timeline_start_sec = self.timeline_start_ms as f64 / 1000.0;
                format!("((({beatmap_time_expr_sec})-({timeline_start_sec:.6}))/{rate:.6})")
            } else {
                let start_sec = self.timeline_start_ms as f64 / 1000.0;
                format!("((({beatmap_time_expr_sec})-({start_sec:.6}))/{rate:.6})")
            }
        } else if self.timeline_start_ms < 0 {
            let timeline_start_sec = self.timeline_start_ms as f64 / 1000.0;
            let preempt_sec = (-self.timeline_start_ms) as f64 / 1000.0;
            let positive_expr = self
                .profile
                .cumulative_output_seconds_expr_for_positive_time(&beatmap_time_expr_sec);
            ffmpeg_if(
                &ffmpeg_lt(&beatmap_time_expr_sec, "0"),
                &format!("(({beatmap_time_expr_sec})-({timeline_start_sec:.6}))"),
                &format!("{preempt_sec:.6}+({positive_expr})"),
            )
        } else {
            let start_expr = self
                .profile
                .cumulative_output_seconds_expr_for_positive_time(&format!(
                    "{:.6}",
                    self.timeline_start_ms as f64 / 1000.0
                ));
            let beatmap_expr = self
                .profile
                .cumulative_output_seconds_expr_for_positive_time(&beatmap_time_expr_sec);
            format!("({beatmap_expr})-({start_expr})")
        }
    }
}
impl PlaybackRateProfile {
    #[inline]
    pub fn is_adaptive(&self) -> bool {
        matches!(self, Self::Adaptive { .. })
    }
    #[inline]
    pub fn uses_wall_clock_preroll(&self) -> bool {
        !self.is_adaptive()
    }
    pub fn key_beatmap_boundaries_ms(&self) -> Vec<i32> {
        match self {
            Self::Constant { .. } => Vec::new(),
            Self::LinearRamp {
                begin_ms, end_ms, ..
            } => vec![*begin_ms, *end_ms],
            Self::Adaptive { profile } => profile.key_beatmap_boundaries_ms(),
        }
    }
    pub fn rate_at_beatmap_time_ms(&self, beatmap_time_ms: f64) -> f64 {
        match self {
            Self::Constant { rate } => *rate,
            Self::LinearRamp {
                initial_rate,
                final_rate,
                begin_ms,
                end_ms,
            } => {
                if beatmap_time_ms <= *begin_ms as f64 {
                    *initial_rate
                } else if beatmap_time_ms >= *end_ms as f64 {
                    *final_rate
                } else {
                    let duration_ms = (*end_ms - *begin_ms).max(1) as f64;
                    let progress =
                        ((beatmap_time_ms - *begin_ms as f64) / duration_ms).clamp(0.0, 1.0);
                    *initial_rate + (*final_rate - *initial_rate) * progress
                }
            }
            Self::Adaptive { profile } => profile.rate_at_beatmap_time_ms(beatmap_time_ms),
        }
    }
    pub fn output_elapsed_ms_between(&self, start_beatmap_ms: f64, end_beatmap_ms: f64) -> f64 {
        if end_beatmap_ms <= start_beatmap_ms {
            return 0.0;
        }
        match self {
            Self::Adaptive { profile } => {
                profile.cumulative_output_elapsed_ms_at(end_beatmap_ms)
                    - profile.cumulative_output_elapsed_ms_at(start_beatmap_ms)
            }
            _ => {
                if end_beatmap_ms <= 0.0 {
                    return end_beatmap_ms - start_beatmap_ms;
                }
                if start_beatmap_ms < 0.0 {
                    return (-start_beatmap_ms)
                        + self.output_elapsed_ms_between(0.0, end_beatmap_ms);
                }
                self.cumulative_output_elapsed_ms_for_positive_time(end_beatmap_ms)
                    - self.cumulative_output_elapsed_ms_for_positive_time(start_beatmap_ms)
            }
        }
    }
    pub fn beatmap_time_after_output_elapsed_ms(
        &self,
        start_beatmap_ms: f64,
        output_elapsed_ms: f64,
    ) -> f64 {
        let output_elapsed_ms = output_elapsed_ms.max(0.0);
        if output_elapsed_ms <= 0.0 {
            return start_beatmap_ms;
        }
        match self {
            Self::Adaptive { profile } => {
                let start_cumulative = profile.cumulative_output_elapsed_ms_at(start_beatmap_ms);
                profile.beatmap_time_at_output_elapsed_ms(start_cumulative + output_elapsed_ms)
            }
            _ => {
                if start_beatmap_ms < 0.0 {
                    let to_zero_ms = -start_beatmap_ms;
                    if output_elapsed_ms <= to_zero_ms {
                        return start_beatmap_ms + output_elapsed_ms;
                    }
                    return self.positive_beatmap_time_for_cumulative_output_elapsed_ms(
                        output_elapsed_ms - to_zero_ms,
                    );
                }
                let start_cumulative =
                    self.cumulative_output_elapsed_ms_for_positive_time(start_beatmap_ms);
                self.positive_beatmap_time_for_cumulative_output_elapsed_ms(
                    start_cumulative + output_elapsed_ms,
                )
            }
        }
    }
    pub fn cumulative_output_seconds_expr_for_positive_time(
        &self,
        beatmap_time_expr_sec: &str,
    ) -> String {
        let beatmap_time_expr_sec = ffmpeg_if(
            &ffmpeg_lt(beatmap_time_expr_sec, "0"),
            "0",
            beatmap_time_expr_sec,
        );
        match self {
            Self::Constant { rate } => {
                format!("({beatmap_time_expr_sec})/{rate:.6}")
            }
            Self::LinearRamp {
                initial_rate,
                final_rate,
                begin_ms,
                end_ms,
            } => {
                let begin_sec = *begin_ms as f64 / 1000.0;
                let end_sec = *end_ms as f64 / 1000.0;
                let ramp_duration_sec = (end_sec - begin_sec).max(0.001);
                let delta = *final_rate - *initial_rate;
                if delta.abs() < 1e-9 {
                    return format!("({beatmap_time_expr_sec})/{initial_rate:.6}");
                }
                // Linear rate ramps integrate to a logarithm in output-time space.
                let cumulative_at_begin = begin_sec / *initial_rate;
                let cumulative_at_end = cumulative_at_begin
                    + (ramp_duration_sec / delta) * (*final_rate / *initial_rate).ln();
                let ramp_rate_expr = format!(
                    "{initial_rate:.12}+({delta:.12})*((({beatmap_time_expr_sec})-{begin_sec:.6})/{ramp_duration_sec:.6})"
                );
                let ramp_expr = format!(
                    "{cumulative_at_begin:.12}+({ramp_duration_sec:.12}/{delta:.12})*log(({ramp_rate_expr})/{initial_rate:.12})"
                );
                let after_expr = format!(
                    "{cumulative_at_end:.12}+((({beatmap_time_expr_sec})-{end_sec:.6})/{final_rate:.12})"
                );
                ffmpeg_if(
                    &ffmpeg_lt(&beatmap_time_expr_sec, &format!("{begin_sec:.6}")),
                    &format!("({beatmap_time_expr_sec})/{initial_rate:.12}"),
                    &ffmpeg_if(
                        &ffmpeg_lt(&beatmap_time_expr_sec, &format!("{end_sec:.6}")),
                        &ramp_expr,
                        &after_expr,
                    ),
                )
            }
            Self::Adaptive { .. } => {
                format!("({beatmap_time_expr_sec})/{:.12}", self.initial_rate())
            }
        }
    }
    fn cumulative_output_elapsed_ms_for_positive_time(&self, beatmap_time_ms: f64) -> f64 {
        let beatmap_time_ms = beatmap_time_ms.max(0.0);
        match self {
            Self::Constant { rate } => beatmap_time_ms / *rate,
            Self::LinearRamp {
                initial_rate,
                final_rate,
                begin_ms,
                end_ms,
            } => {
                let begin_ms = *begin_ms as f64;
                let end_ms = *end_ms as f64;
                if beatmap_time_ms <= begin_ms {
                    beatmap_time_ms / *initial_rate
                } else if beatmap_time_ms <= end_ms {
                    let ramp_duration_ms = (end_ms - begin_ms).max(1.0);
                    let delta = *final_rate - *initial_rate;
                    if delta.abs() < 1e-9 {
                        beatmap_time_ms / *initial_rate
                    } else {
                        let rate_at_time = *initial_rate
                            + delta * ((beatmap_time_ms - begin_ms) / ramp_duration_ms);
                        begin_ms / *initial_rate
                            + (ramp_duration_ms / delta) * (rate_at_time / *initial_rate).ln()
                    }
                } else {
                    let ramp_duration_ms = (end_ms - begin_ms).max(1.0);
                    let ramp_elapsed_ms = if (*final_rate - *initial_rate).abs() < 1e-9 {
                        ramp_duration_ms / *initial_rate
                    } else {
                        (ramp_duration_ms / (*final_rate - *initial_rate))
                            * (*final_rate / *initial_rate).ln()
                    };
                    begin_ms / *initial_rate
                        + ramp_elapsed_ms
                        + (beatmap_time_ms - end_ms) / *final_rate
                }
            }
            Self::Adaptive { profile } => profile.cumulative_output_elapsed_ms_at(beatmap_time_ms),
        }
    }
    fn positive_beatmap_time_for_cumulative_output_elapsed_ms(
        &self,
        output_elapsed_ms: f64,
    ) -> f64 {
        let output_elapsed_ms = output_elapsed_ms.max(0.0);
        match self {
            Self::Constant { rate } => output_elapsed_ms * *rate,
            Self::LinearRamp {
                initial_rate,
                final_rate,
                begin_ms,
                end_ms,
            } => {
                let begin_ms = *begin_ms as f64;
                let end_ms = *end_ms as f64;
                let ramp_duration_ms = (end_ms - begin_ms).max(1.0);
                let delta = *final_rate - *initial_rate;
                let elapsed_at_begin = begin_ms / *initial_rate;
                let elapsed_at_end = if delta.abs() < 1e-9 {
                    end_ms / *initial_rate
                } else {
                    elapsed_at_begin
                        + (ramp_duration_ms / delta) * (*final_rate / *initial_rate).ln()
                };
                if output_elapsed_ms <= elapsed_at_begin {
                    output_elapsed_ms * *initial_rate
                } else if output_elapsed_ms <= elapsed_at_end && delta.abs() >= 1e-9 {
                    let ramp_elapsed_ms = output_elapsed_ms - elapsed_at_begin;
                    let rate = *initial_rate * ((delta / ramp_duration_ms) * ramp_elapsed_ms).exp();
                    begin_ms + ramp_duration_ms * ((rate - *initial_rate) / delta)
                } else if output_elapsed_ms <= elapsed_at_end {
                    output_elapsed_ms * *initial_rate
                } else {
                    end_ms + (output_elapsed_ms - elapsed_at_end) * *final_rate
                }
            }
            Self::Adaptive { profile } => {
                profile.beatmap_time_at_output_elapsed_ms(output_elapsed_ms)
            }
        }
    }
}
pub fn build_adaptive_playback_profile(
    initial_rate: f64,
    hit_objects: &[HitObject],
    score_judgments: &[ScoreJudgmentEvent],
) -> PlaybackRateProfile {
    let initial_rate = sanitize_rate(initial_rate).clamp(0.5, 2.0);
    let canonical_end_times = score_judgments
        .iter()
        .map(|judgment| canonical_end_time_for_judgment(hit_objects, judgment))
        .collect::<Vec<_>>();
    let mut distinct_end_times = canonical_end_times
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    distinct_end_times.sort_by(|lhs, rhs| lhs.total_cmp(rhs));
    distinct_end_times.dedup_by(|lhs, rhs| (*lhs - *rhs).abs() <= f64::EPSILON);
    let mut segments = Vec::new();
    let mut recent_rates = VecDeque::from(vec![initial_rate; ADAPTIVE_RECENT_RATE_COUNT]);
    let mut current_beatmap_ms = 0.0;
    let mut output_elapsed_ms = 0.0;
    let mut current_rate = initial_rate;
    let mut target_rate = initial_rate;
    // Adaptive speed treats judgment spacing as the signal for whether playback should speed up or slow down.
    for (judgment, canonical_end_time) in score_judgments.iter().zip(canonical_end_times.iter()) {
        let event_time_ms = judgment.event_time as f64;
        if event_time_ms > current_beatmap_ms {
            let segment_output_elapsed = solve_output_elapsed_for_beatmap_delta(
                event_time_ms - current_beatmap_ms,
                current_rate,
                target_rate,
            );
            segments.push(AdaptivePlaybackSegment {
                beatmap_start_ms: current_beatmap_ms,
                beatmap_end_ms: Some(event_time_ms),
                output_start_ms: output_elapsed_ms,
                output_end_ms: Some(output_elapsed_ms + segment_output_elapsed),
                start_rate: current_rate,
                target_rate,
            });
            output_elapsed_ms += segment_output_elapsed;
            current_rate =
                rate_after_output_elapsed(current_rate, target_rate, segment_output_elapsed);
            current_beatmap_ms = event_time_ms;
        }
        let Some(canonical_end_time) = *canonical_end_time else {
            continue;
        };
        let Some(preceding_end_time) =
            preceding_distinct_end_time(&distinct_end_times, canonical_end_time)
        else {
            continue;
        };
        let relative_change = if judgment.kind == JudgmentKind::Miss {
            ADAPTIVE_MISS_RATE_CHANGE
        } else {
            let denominator = event_time_ms - preceding_end_time;
            if denominator.abs() <= f64::EPSILON {
                ADAPTIVE_MAX_RATE_CHANGE
            } else {
                // Compare intended note spacing with actual judgment spacing and clamp sudden jumps.
                ((canonical_end_time - preceding_end_time) / denominator)
                    .clamp(ADAPTIVE_MIN_RATE_CHANGE, ADAPTIVE_MAX_RATE_CHANGE)
            }
        };
        let new_recent_rate =
            (relative_change * current_rate).clamp(ADAPTIVE_MIN_RATE, ADAPTIVE_MAX_RATE);
        recent_rates.pop_front();
        recent_rates.push_back(new_recent_rate);
        let consistency = recent_rates
            .iter()
            .zip(recent_rates.iter().skip(1))
            .map(|(lhs, rhs)| signum_f64(rhs - lhs))
            .sum::<i32>();
        let weight =
            (consistency.abs() as f64) / (ADAPTIVE_RECENT_RATE_COUNT.saturating_sub(1) as f64);
        let average_rate = recent_rates.iter().sum::<f64>() / recent_rates.len() as f64;
        target_rate = lerp(target_rate, average_rate, weight);
    }
    segments.push(AdaptivePlaybackSegment {
        beatmap_start_ms: current_beatmap_ms,
        beatmap_end_ms: None,
        output_start_ms: output_elapsed_ms,
        output_end_ms: None,
        start_rate: current_rate,
        target_rate,
    });
    PlaybackRateProfile::Adaptive {
        profile: AdaptivePlaybackProfile {
            initial_rate,
            tail_rate: target_rate,
            segments,
        },
    }
}
impl AdaptivePlaybackProfile {
    fn key_beatmap_boundaries_ms(&self) -> Vec<i32> {
        let mut boundaries = self
            .segments
            .iter()
            .flat_map(|segment| {
                std::iter::once(segment.beatmap_start_ms.round() as i32)
                    .chain(segment.beatmap_end_ms.map(|time| time.round() as i32))
            })
            .collect::<Vec<_>>();
        boundaries.sort_unstable();
        boundaries.dedup();
        boundaries
    }
    fn rate_at_beatmap_time_ms(&self, beatmap_time_ms: f64) -> f64 {
        if beatmap_time_ms <= 0.0 {
            return self.initial_rate;
        }
        let Some(segment) = self.segment_for_beatmap_time(beatmap_time_ms) else {
            return self.tail_rate;
        };
        let local_beatmap_delta_ms = (beatmap_time_ms - segment.beatmap_start_ms).max(0.0);
        let local_output_elapsed_ms =
            segment.solve_output_elapsed_for_beatmap_delta(local_beatmap_delta_ms);
        rate_after_output_elapsed(
            segment.start_rate,
            segment.target_rate,
            local_output_elapsed_ms,
        )
    }
    fn cumulative_output_elapsed_ms_at(&self, beatmap_time_ms: f64) -> f64 {
        if beatmap_time_ms <= 0.0 {
            return beatmap_time_ms / self.initial_rate;
        }
        let Some(segment) = self.segment_for_beatmap_time(beatmap_time_ms) else {
            return beatmap_time_ms / self.initial_rate;
        };
        let local_beatmap_delta_ms = (beatmap_time_ms - segment.beatmap_start_ms).max(0.0);
        segment.output_start_ms
            + segment.solve_output_elapsed_for_beatmap_delta(local_beatmap_delta_ms)
    }
    fn beatmap_time_at_output_elapsed_ms(&self, output_elapsed_ms: f64) -> f64 {
        if output_elapsed_ms <= 0.0 {
            return output_elapsed_ms * self.initial_rate;
        }
        let Some(segment) = self.segment_for_output_elapsed(output_elapsed_ms) else {
            return output_elapsed_ms * self.initial_rate;
        };
        let local_output_elapsed_ms = (output_elapsed_ms - segment.output_start_ms).max(0.0);
        segment.beatmap_start_ms
            + beatmap_delta_from_output_elapsed(
                segment.start_rate,
                segment.target_rate,
                local_output_elapsed_ms,
            )
    }
    fn segment_for_beatmap_time(&self, beatmap_time_ms: f64) -> Option<&AdaptivePlaybackSegment> {
        self.segments.iter().find(|segment| {
            beatmap_time_ms >= segment.beatmap_start_ms
                && segment
                    .beatmap_end_ms
                    .map(|end_ms| beatmap_time_ms < end_ms)
                    .unwrap_or(true)
        })
    }
    fn segment_for_output_elapsed(
        &self,
        output_elapsed_ms: f64,
    ) -> Option<&AdaptivePlaybackSegment> {
        self.segments.iter().find(|segment| {
            output_elapsed_ms >= segment.output_start_ms
                && segment
                    .output_end_ms
                    .map(|end_ms| output_elapsed_ms < end_ms)
                    .unwrap_or(true)
        })
    }
}
impl AdaptivePlaybackSegment {
    fn solve_output_elapsed_for_beatmap_delta(&self, beatmap_delta_ms: f64) -> f64 {
        solve_output_elapsed_for_beatmap_delta(beatmap_delta_ms, self.start_rate, self.target_rate)
    }
}
fn canonical_end_time_for_judgment(
    hit_objects: &[HitObject],
    judgment: &ScoreJudgmentEvent,
) -> Option<f64> {
    let hit_object = hit_objects.get(judgment.note_index)?;
    Some(match judgment.part {
        ScoreJudgmentPart::Tap | ScoreJudgmentPart::LnHead => hit_object.time as f64,
        ScoreJudgmentPart::LnTail => hit_object.end_time.unwrap_or(hit_object.time) as f64,
    })
}
fn preceding_distinct_end_time(distinct_end_times: &[f64], end_time_ms: f64) -> Option<f64> {
    match distinct_end_times.binary_search_by(|candidate| candidate.total_cmp(&end_time_ms)) {
        Ok(0) | Err(0) => None,
        Ok(index) | Err(index) => distinct_end_times.get(index - 1).copied(),
    }
}
fn signum_f64(value: f64) -> i32 {
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}
fn lerp(start: f64, end: f64, amount: f64) -> f64 {
    start + (end - start) * amount.clamp(0.0, 1.0)
}
fn adaptive_decay(output_elapsed_ms: f64) -> f64 {
    // Rate changes ease exponentially with a fixed half-time to avoid audible jumps.
    0.5f64.powf(output_elapsed_ms / ADAPTIVE_DAMP_HALF_TIME_MS)
}
fn rate_after_output_elapsed(start_rate: f64, target_rate: f64, output_elapsed_ms: f64) -> f64 {
    if (start_rate - target_rate).abs() < 1e-12 {
        target_rate
    } else {
        target_rate + (start_rate - target_rate) * adaptive_decay(output_elapsed_ms.max(0.0))
    }
}
fn beatmap_delta_from_output_elapsed(
    start_rate: f64,
    target_rate: f64,
    output_elapsed_ms: f64,
) -> f64 {
    let output_elapsed_ms = output_elapsed_ms.max(0.0);
    if (start_rate - target_rate).abs() < 1e-12 {
        return start_rate * output_elapsed_ms;
    }
    let coefficient = ADAPTIVE_DAMP_HALF_TIME_MS / std::f64::consts::LN_2;
    target_rate * output_elapsed_ms
        + (start_rate - target_rate) * coefficient * (1.0 - adaptive_decay(output_elapsed_ms))
}
fn solve_output_elapsed_for_beatmap_delta(
    beatmap_delta_ms: f64,
    start_rate: f64,
    target_rate: f64,
) -> f64 {
    let beatmap_delta_ms = beatmap_delta_ms.max(0.0);
    if beatmap_delta_ms <= ADAPTIVE_BINARY_SEARCH_EPSILON_MS {
        return 0.0;
    }
    let min_rate = start_rate.min(target_rate).max(ADAPTIVE_MIN_RATE);
    let mut high = (beatmap_delta_ms / min_rate).max(1.0);
    while beatmap_delta_from_output_elapsed(start_rate, target_rate, high) < beatmap_delta_ms {
        high *= 2.0;
    }
    let mut low = 0.0;
    // The adaptive integral is monotonic but has no simple inverse, so solve it numerically.
    for _ in 0..48 {
        let mid = (low + high) / 2.0;
        if beatmap_delta_from_output_elapsed(start_rate, target_rate, mid) < beatmap_delta_ms {
            low = mid;
        } else {
            high = mid;
        }
    }
    (low + high) / 2.0
}
fn sanitize_rate(rate: f64) -> f64 {
    if rate.is_finite() && rate > 0.0 {
        rate
    } else {
        1.0
    }
}
fn sanitize_profile(profile: PlaybackRateProfile) -> PlaybackRateProfile {
    match profile {
        PlaybackRateProfile::Constant { rate } => PlaybackRateProfile::Constant {
            rate: sanitize_rate(rate),
        },
        PlaybackRateProfile::LinearRamp {
            initial_rate,
            final_rate,
            begin_ms,
            end_ms,
        } => PlaybackRateProfile::LinearRamp {
            initial_rate: sanitize_rate(initial_rate),
            final_rate: sanitize_rate(final_rate),
            begin_ms,
            end_ms: end_ms.max(begin_ms + 1),
        },
        PlaybackRateProfile::Adaptive { mut profile } => {
            profile.initial_rate = sanitize_rate(profile.initial_rate).clamp(0.5, 2.0);
            profile.tail_rate = profile
                .tail_rate
                .clamp(ADAPTIVE_MIN_RATE, ADAPTIVE_MAX_RATE);
            for segment in &mut profile.segments {
                segment.start_rate = segment
                    .start_rate
                    .clamp(ADAPTIVE_MIN_RATE, ADAPTIVE_MAX_RATE);
                segment.target_rate = segment
                    .target_rate
                    .clamp(ADAPTIVE_MIN_RATE, ADAPTIVE_MAX_RATE);
            }
            PlaybackRateProfile::Adaptive { profile }
        }
    }
}
fn ffmpeg_if(condition: &str, when_true: &str, when_false: &str) -> String {
    format!("if({condition}\\,{when_true}\\,{when_false})")
}
fn ffmpeg_lt(left: &str, right: &str) -> String {
    format!("lt({left}\\,{right})")
}
