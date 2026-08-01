use crate::intro::IntroModBadgeSpec;
use crate::modes::mania::judgment::ScoreJudgmentEvent;
use crate::renderer::{HealthTimeline, LnReleaseInfo, RenderJudgment};
use crate::types::replay::ReplayBasicStatistics;
use crate::types::{HitObject, ReplayOrigin};
use crate::utils::mods::PlaybackRateProfile;
use crate::utils::mods::{has_fade_in_mod, has_flashlight_mod, has_hidden_mod};
pub(crate) const RESULTS_TRANSITION_MS: i32 = 4_500;
pub(crate) const RESULTS_DURATION_MS: i32 = 5_000;
pub(crate) const RESULTS_FADE_MS: i32 = 500;
pub(crate) const GRAPH_SAMPLE_COUNT: usize = 96;
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct ResultsGraphPoint {
    pub(crate) progress: f32,
    pub(crate) life: f32,
}
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ResultsTimingSummary {
    pub(crate) sample_count: usize,
    pub(crate) avg_early_ms: f32,
    pub(crate) avg_late_ms: f32,
    pub(crate) unstable_rate: f32,
}
pub(crate) fn summarize_timing_from_render_data(
    hit_objects: &[HitObject],
    render_judgments: &[RenderJudgment],
    ln_release_by_idx: &[Option<LnReleaseInfo>],
    _score_judgments: &[ScoreJudgmentEvent],
    rate_profile: &PlaybackRateProfile,
) -> ResultsTimingSummary {
    let mut deltas = Vec::with_capacity(render_judgments.len() * 2);
    for judgment in render_judgments {
        let idx = judgment.idx;
        let Some(hit_object) = hit_objects.get(idx) else {
            continue;
        };
        if judgment.kind.score_value() > 0 {
            if let Some(press_time) = judgment.press_time {
                deltas.push(converted_timing_delta(
                    press_time - hit_object.time,
                    press_time,
                    rate_profile,
                ));
            }
        }
        if !judgment.is_ln {
            continue;
        }
        let Some(release) = ln_release_by_idx.get(idx).copied().flatten() else {
            continue;
        };
        let Some(release_time) = release.time else {
            continue;
        };
        if matches!(
            release.kind,
            crate::renderer::ReleaseKind::Miss | crate::renderer::ReleaseKind::None
        ) {
            continue;
        }
        deltas.push(converted_timing_delta(
            release_time - hit_object.end_time.unwrap_or(hit_object.time),
            release_time,
            rate_profile,
        ));
    }
    summarize_timing_from_deltas(&deltas)
}
fn converted_timing_delta(
    delta_ms: i32,
    sample_time_ms: i32,
    rate_profile: &PlaybackRateProfile,
) -> f64 {
    let rate = rate_profile
        .rate_at_beatmap_time_ms(sample_time_ms as f64)
        .abs()
        .max(f64::EPSILON);
    // Timing windows are displayed in real playback milliseconds, not accelerated beatmap time.
    f64::from(delta_ms) / rate
}
fn summarize_timing_from_deltas(deltas: &[f64]) -> ResultsTimingSummary {
    if deltas.is_empty() {
        return ResultsTimingSummary::default();
    }
    let mut early_sum = 0.0f64;
    let mut early_count = 0usize;
    let mut late_sum = 0.0f64;
    let mut late_count = 0usize;
    for &delta in deltas {
        if delta < 0.0 {
            early_sum += delta;
            early_count += 1;
        } else {
            late_sum += delta;
            late_count += 1;
        }
    }
    let mean = deltas.iter().sum::<f64>() / deltas.len() as f64;
    let variance = deltas
        .iter()
        .map(|delta| {
            let centered = *delta - mean;
            centered * centered
        })
        .sum::<f64>()
        / deltas.len() as f64;
    ResultsTimingSummary {
        sample_count: deltas.len(),
        avg_early_ms: if early_count > 0 {
            (early_sum / early_count as f64) as f32
        } else {
            0.0
        },
        avg_late_ms: if late_count > 0 {
            (late_sum / late_count as f64) as f32
        } else {
            0.0
        },
        // osu! reports unstable rate as timing standard deviation multiplied by 10.
        unstable_rate: (variance.sqrt() * 10.0) as f32,
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResultsGrade {
    XH,
    X,
    SH,
    S,
    A,
    B,
    C,
    D,
}
impl ResultsGrade {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::XH => "XH",
            Self::X => "X",
            Self::SH => "SH",
            Self::S => "S",
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
        }
    }
    pub(crate) fn skin_suffix(self) -> &'static str {
        self.label()
    }
    pub(crate) fn fallback_color(self) -> [u8; 4] {
        match self {
            Self::XH | Self::SH => [0xE4, 0xE8, 0xF6, 0xFF],
            Self::X => [0xF7, 0xDC, 0x7A, 0xFF],
            Self::S => [0xF3, 0xC8, 0x53, 0xFF],
            Self::A => [0x67, 0xD8, 0x86, 0xFF],
            Self::B => [0x6D, 0xB6, 0xFF, 0xFF],
            Self::C => [0xB7, 0x8D, 0xF3, 0xFF],
            Self::D => [0xFF, 0x7A, 0x7A, 0xFF],
        }
    }
}
pub(crate) fn silver_grade_from_mods(mods: u32) -> bool {
    has_hidden_mod(mods) || has_flashlight_mod(mods) || has_fade_in_mod(mods)
}
pub(crate) fn grade_for_accuracy(accuracy: f32, silver_grade: bool) -> ResultsGrade {
    let accuracy = accuracy.clamp(0.0, 100.0);
    if accuracy >= 100.0 - 0.0005 {
        if silver_grade {
            ResultsGrade::XH
        } else {
            ResultsGrade::X
        }
    } else if accuracy > 95.0 {
        if silver_grade {
            ResultsGrade::SH
        } else {
            ResultsGrade::S
        }
    } else if accuracy > 90.0 {
        ResultsGrade::A
    } else if accuracy > 80.0 {
        ResultsGrade::B
    } else if accuracy > 70.0 {
        ResultsGrade::C
    } else {
        ResultsGrade::D
    }
}
pub(crate) fn compute_perfect_combo(
    statistics: ReplayBasicStatistics,
    combo_breaks: usize,
    final_combo: u32,
    computed_max_combo: u32,
) -> bool {
    computed_max_combo > 0
        && combo_breaks == 0
        && statistics.miss == 0
        && final_combo >= computed_max_combo
}
pub(crate) fn build_results_graph_points(
    timeline: &HealthTimeline,
    start_ms: i32,
    end_ms: i32,
    max_points: usize,
    score_judgments: &[ScoreJudgmentEvent],
) -> Vec<ResultsGraphPoint> {
    let max_points = max_points.max(2);
    let start_ms = start_ms.min(end_ms);
    let end_ms = end_ms.max(start_ms + 1);
    let selected = score_judgments
        .iter()
        .filter(|judgment| judgment.event_time >= start_ms && judgment.event_time <= end_ms)
        .collect::<Vec<_>>();
    let graph_begin = selected
        .first()
        .map(|judgment| judgment.event_time)
        .unwrap_or(start_ms);
    // The graph spans actual scoring events so long empty lead-ins do not flatten the visible life curve.
    let graph_end = selected
        .last()
        .map(|judgment| judgment.event_time.max(graph_begin + 1))
        .unwrap_or(end_ms.max(graph_begin + 1));
    let span = (graph_end - graph_begin).max(1) as f32;
    let mut points = Vec::with_capacity(score_judgments.len() + 2);
    points.push(ResultsGraphPoint {
        progress: 0.0,
        life: timeline.life_at_time(graph_begin).clamp(0.0, 1.0),
    });
    for judgment in selected {
        let progress = ((judgment.event_time - graph_begin) as f32 / span).clamp(0.0, 1.0);
        let life = timeline.life_at_time(judgment.event_time).clamp(0.0, 1.0);
        if let Some(last) = points.last_mut() {
            if (last.progress - progress).abs() <= f32::EPSILON {
                // Multiple score events can share one timestamp; keep the final life at that instant.
                last.life = life;
                continue;
            }
        }
        points.push(ResultsGraphPoint { progress, life });
    }
    if points
        .last()
        .map(|point| point.progress < 1.0)
        .unwrap_or(true)
    {
        points.push(ResultsGraphPoint {
            progress: 1.0,
            life: timeline.life_at_time(graph_end).clamp(0.0, 1.0),
        });
    }
    while points.len() > max_points {
        // Preserve endpoints and thin interior samples evenly for a stable fixed-size graph.
        for index in (1..points.len().saturating_sub(1)).rev().step_by(2) {
            points.remove(index);
            if points.len() <= max_points {
                break;
            }
        }
    }
    smooth_results_graph_points(&points)
}
fn smooth_results_graph_points(points: &[ResultsGraphPoint]) -> Vec<ResultsGraphPoint> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let mut smoothed = points.to_vec();
    for index in 1..points.len() - 1 {
        smoothed[index].life = (points[index - 1].life * 0.2
            + points[index].life * 0.6
            + points[index + 1].life * 0.2)
            .clamp(0.0, 1.0);
    }
    smoothed
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct EndSequencePlan {
    pub(crate) gameplay_end_ms: i32,
    pub(crate) hud_hide_start_ms: i32,
    pub(crate) fade_out_end_ms: i32,
    pub(crate) results_start_ms: i32,
    pub(crate) results_end_ms: i32,
    pub(crate) main_scene_frames: u64,
    pub(crate) results_frames: u64,
}
impl EndSequencePlan {
    #[inline]
    pub(crate) fn has_results(self) -> bool {
        self.results_frames > 0
    }
    #[inline]
    pub(crate) fn hud_visible_at(self, beatmap_time_ms: i32) -> bool {
        // Results reuse the HUD layer after the gameplay fade so only the transition gap hides it.
        !self.has_results()
            || beatmap_time_ms < self.fade_out_end_ms
            || beatmap_time_ms >= self.results_start_ms
    }
    #[inline]
    pub(crate) fn gameplay_alpha_at(self, beatmap_time_ms: i32) -> f32 {
        if !self.has_results() || beatmap_time_ms < self.hud_hide_start_ms {
            return 1.0;
        }
        if beatmap_time_ms >= self.fade_out_end_ms {
            return 0.0;
        }
        let span = (self.fade_out_end_ms - self.hud_hide_start_ms).max(1) as f32;
        let progress = ((beatmap_time_ms - self.hud_hide_start_ms) as f32 / span).clamp(0.0, 1.0);
        let inv = 1.0 - progress;
        inv * inv * inv
    }
}
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct ResultsScreenData {
    pub(crate) player_name: String,
    pub(crate) artist: String,
    pub(crate) title: String,
    pub(crate) difficulty: String,
    pub(crate) creator: String,
    pub(crate) mod_badges: Vec<IntroModBadgeSpec>,
    pub(crate) mod_origin: ReplayOrigin,
    pub(crate) replay_timestamp: Option<i64>,
    pub(crate) score: u32,
    pub(crate) accuracy: f32,
    pub(crate) max_combo: u32,
    pub(crate) final_combo: u32,
    pub(crate) statistics: ReplayBasicStatistics,
    pub(crate) grade: ResultsGrade,
    pub(crate) perfect_combo: bool,
    pub(crate) graph_points: Vec<ResultsGraphPoint>,
    pub(crate) timing_summary: ResultsTimingSummary,
}

/// A plausible score for the HUD editor preview, used when no replay backs it.
pub(crate) fn sample_results_screen_data() -> ResultsScreenData {
    let statistics = ReplayBasicStatistics {
        max: 1_180,
        hit300: 402,
        hit200: 61,
        hit100: 18,
        hit50: 3,
        miss: 6,
    };
    ResultsScreenData {
        player_name: "Player".to_string(),
        artist: "Artist".to_string(),
        title: "Song Title".to_string(),
        difficulty: "Insane".to_string(),
        creator: "Mapper".to_string(),
        mod_badges: Vec::new(),
        mod_origin: ReplayOrigin::StableLegacy,
        replay_timestamp: None,
        score: 943_512,
        accuracy: 98.41,
        max_combo: 1_204,
        final_combo: 486,
        statistics,
        grade: ResultsGrade::S,
        perfect_combo: false,
        // A curve that dips and recovers, so the graph is not a flat line.
        graph_points: (0..GRAPH_SAMPLE_COUNT)
            .map(|index| {
                let progress = index as f32 / (GRAPH_SAMPLE_COUNT - 1) as f32;
                ResultsGraphPoint {
                    progress,
                    life: (0.55 + 0.45 * (progress * 6.0).sin() * (1.0 - progress * 0.4))
                        .clamp(0.05, 1.0),
                }
            })
            .collect(),
        timing_summary: ResultsTimingSummary {
            sample_count: 484,
            avg_early_ms: -7.4,
            avg_late_ms: 8.1,
            unstable_rate: 92.6,
        },
    }
}
