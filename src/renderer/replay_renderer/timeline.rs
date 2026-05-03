use super::model::{
    ComboEvent, ComboEventType, LnComboBreak, LnComboTick, RawEvent, RawEventKind, RenderPlan,
    ScoreConstants, Windows,
};
use super::render::ReplayRenderer;
use super::state::{
    anim, ComboBreakAnimation, ComboBurstAnimation, ComboIncAnimation, HitErrorJudgment,
    HitErrorWindows, HudBeatmapMetadataState, HudFrameState, LastJudgment,
};
use super::HealthTimeline;
use crate::modes::mania::judgment::{ScoreJudgmentEvent, ScoreMode};
use crate::types::JudgmentKind;

const SCORE_FLOAT_EPSILON: f64 = 1e-6;

impl ReplayRenderer {
    pub fn compute_plan(
        &self,
        first_note_time: i32,
        last_note_time: i32,
        pps_base: f32,
    ) -> Result<RenderPlan, String> {
        if !pps_base.is_finite() || pps_base <= 0.0 {
            return Err("render speed basis must be finite and positive".to_string());
        }
        let frame_time = 1000.0 / self.cfg.fps as f64;
        let travel_ms = (self.cfg.height as f64 / pps_base as f64) * 1000.0;
        // Start before the first note by its full travel time so notes enter from off-screen.
        let timeline_start =
            i64::from(first_note_time) - travel_ms as i64 - i64::from(self.lead_in_ms);
        let timeline_end = i64::from(last_note_time) + 2000_i64;
        let duration = (timeline_end - timeline_start) as f64;
        let total_frames = (duration / frame_time).ceil() as usize;
        Ok(RenderPlan {
            timeline_start: i32::try_from(timeline_start).map_err(|_| {
                format!("render timeline start is out of i32 range: {timeline_start} ms")
            })?,
            timeline_end: i32::try_from(timeline_end).map_err(|_| {
                format!("render timeline end is out of i32 range: {timeline_end} ms")
            })?,
            frame_time,
            total_frames,
            travel_ms,
        })
    }
    pub fn precompute_score_events(
        &self,
        score_judgments: &[ScoreJudgmentEvent],
        ln_ticks: &[LnComboTick],
        ln_breaks: &[LnComboBreak],
        score_mode: ScoreMode,
    ) -> Vec<ComboEvent> {
        let score_const = ScoreConstants::for_mode(score_mode);
        let total_notes = score_judgments.len();
        if total_notes == 0 {
            return Vec::new();
        }
        let max_score = 1_000_000.0;
        // Stable ScoreV1 reserves half the score for base hits and half for the bonus component.
        let unit = (max_score * 0.5) / total_notes as f64;
        let mut raw_events: Vec<RawEvent> =
            Vec::with_capacity(score_judgments.len() + ln_ticks.len() + ln_breaks.len());
        for (idx, j) in score_judgments.iter().enumerate() {
            raw_events.push(RawEvent {
                time: j.event_time,
                kind: RawEventKind::Judgment,
                judgment_idx: Some(idx),
                ln_idx: None,
            });
        }
        for tick in ln_ticks {
            raw_events.push(RawEvent {
                time: tick.time,
                kind: RawEventKind::LnTick,
                judgment_idx: None,
                ln_idx: Some(tick.ln_idx),
            });
        }
        for brk in ln_breaks {
            raw_events.push(RawEvent {
                time: brk.time,
                kind: RawEventKind::LnBreak,
                judgment_idx: None,
                ln_idx: Some(brk.ln_idx),
            });
        }
        raw_events.sort_by_key(|e| e.time);
        // Merge judgments, LN ticks, and LN breaks into one monotonic stream for HUD state queries.
        let mut result = Vec::with_capacity(raw_events.len());
        let mut bonus = 100i32;
        let mut cum_score = 0.0f64;
        let mut combo = 0u32;
        let mut acc_hits = 0u32;
        let mut acc_max_hits = 0u32;
        let mut hit_error_moving_avg: Option<f32> = None;
        for ev in raw_events {
            match ev.kind {
                RawEventKind::Judgment => {
                    let score_judgment_idx = ev.judgment_idx.unwrap();
                    let j = &score_judgments[score_judgment_idx];
                    let delta = match score_mode {
                        ScoreMode::ScoreV1 => {
                            let hv = score_const.hit_value(j.kind) as f64;
                            let hbv = score_const.hit_bonus_value(j.kind) as f64;
                            let base = unit * (hv / 320.0);
                            let bonus_f = bonus.clamp(0, 100) as f64;
                            let bonus_add = unit * (hbv * bonus_f.sqrt() / 320.0);
                            base + bonus_add
                        }
                        ScoreMode::ScoreV2 => {
                            let acc_delta = max_score
                                * score_const.v2_acc_portion
                                * score_const.acc_weight(j.kind) as f64
                                / (total_notes as f64 * score_const.acc_max_per_hit as f64);
                            let combo_success = if j.breaks_combo(score_mode) { 0.0 } else { 1.0 };
                            let combo_delta =
                                max_score * score_const.v2_combo_portion * combo_success
                                    / total_notes as f64;
                            acc_delta + combo_delta
                        }
                        ScoreMode::Lazer => {
                            let acc_delta = max_score
                                * score_const.v2_acc_portion
                                * score_const.acc_weight(j.kind) as f64
                                / (total_notes as f64 * score_const.acc_max_per_hit as f64);
                            let combo_success = if j.breaks_combo(score_mode) { 0.0 } else { 1.0 };
                            let combo_delta =
                                max_score * score_const.v2_combo_portion * combo_success
                                    / total_notes as f64;
                            acc_delta + combo_delta
                        }
                    };
                    cum_score += delta;
                    acc_hits += score_const.acc_weight(j.kind);
                    acc_max_hits += score_const.acc_max_per_hit;
                    let combo_break = match score_mode {
                        ScoreMode::ScoreV1 => {
                            if j.kind == JudgmentKind::Miss {
                                let brk = if combo > 0 { Some(combo) } else { None };
                                combo = 0;
                                brk
                            } else if !j.is_ln {
                                // ScoreV1 does not increment combo on 50s or on the LN head judgment itself.
                                if j.kind != JudgmentKind::Hit50 {
                                    combo += 1;
                                }
                                None
                            } else {
                                None
                            }
                        }
                        ScoreMode::ScoreV2 => {
                            if j.breaks_combo(score_mode) {
                                let brk = if combo > 0 { Some(combo) } else { None };
                                combo = 0;
                                brk
                            } else {
                                combo += 1;
                                None
                            }
                        }
                        ScoreMode::Lazer => {
                            if j.breaks_combo(score_mode) {
                                let brk = if combo > 0 { Some(combo) } else { None };
                                combo = 0;
                                brk
                            } else {
                                combo += 1;
                                None
                            }
                        }
                    };
                    let hit_error_offset_ms = j
                        .hit_error_offset_ms
                        .filter(|_| j.kind != JudgmentKind::Miss);
                    if let Some(offset) = hit_error_offset_ms {
                        hit_error_moving_avg =
                            Some(hit_error_moving_avg.unwrap_or(0.0) * 0.9 + offset as f32 * 0.1);
                    }
                    result.push(ComboEvent {
                        time: ev.time,
                        event_type: ComboEventType::Judgment,
                        score_judgment_idx: Some(score_judgment_idx),
                        score_delta: delta,
                        cumulative_score: cum_score,
                        combo_after: combo,
                        acc_hits,
                        acc_max_hits,
                        hit_error_offset_ms,
                        hit_error_moving_avg_ms: hit_error_moving_avg,
                        combo_break_start: combo_break,
                    });
                    if score_mode == ScoreMode::ScoreV1 {
                        if j.kind == JudgmentKind::Miss {
                            bonus = 0;
                        } else {
                            let add = score_const.hit_bonus_add(j.kind);
                            let punish = score_const.hit_punish(j.kind);
                            bonus = (bonus + add - punish).clamp(0, 100);
                        }
                    }
                }
                RawEventKind::LnTick => {
                    combo += 1;
                    result.push(ComboEvent {
                        time: ev.time,
                        event_type: ComboEventType::LnTick,
                        score_judgment_idx: None,
                        score_delta: 0.0,
                        cumulative_score: cum_score,
                        combo_after: combo,
                        acc_hits,
                        acc_max_hits,
                        hit_error_offset_ms: None,
                        hit_error_moving_avg_ms: hit_error_moving_avg,
                        combo_break_start: None,
                    });
                }
                RawEventKind::LnBreak => {
                    let brk = if combo > 0 { Some(combo) } else { None };
                    combo = 0;
                    result.push(ComboEvent {
                        time: ev.time,
                        event_type: ComboEventType::LnBreak,
                        score_judgment_idx: None,
                        score_delta: 0.0,
                        cumulative_score: cum_score,
                        combo_after: combo,
                        acc_hits,
                        acc_max_hits,
                        hit_error_offset_ms: None,
                        hit_error_moving_avg_ms: hit_error_moving_avg,
                        combo_break_start: brk,
                    });
                }
            }
        }
        result
    }
    pub fn score_state_at_time(events: &[ComboEvent], time: i32) -> Option<&ComboEvent> {
        if events.is_empty() {
            return None;
        }
        let idx = events.partition_point(|e| e.time <= time);
        if idx == 0 {
            return None;
        }
        Some(&events[idx - 1])
    }
    pub fn combo_break_at_time(
        events: &[ComboEvent],
        time: i32,
        anim_duration_ms: i32,
    ) -> Option<(u32, f32)> {
        let start_search = time - anim_duration_ms;
        for ev in events.iter().rev() {
            if ev.time < start_search {
                break;
            }
            if ev.time <= time {
                if let Some(brk_combo) = ev.combo_break_start {
                    let elapsed = time - ev.time;
                    let progress = elapsed as f32 / anim_duration_ms as f32;
                    if progress <= 1.0 {
                        return Some((brk_combo, progress));
                    }
                }
            }
        }
        None
    }
    pub fn calculate_final_score(
        score_judgments: &[ScoreJudgmentEvent],
        score_mode: ScoreMode,
    ) -> u32 {
        if score_judgments.is_empty() {
            return 0;
        }
        let score_const = ScoreConstants::for_mode(score_mode);
        let max_score = 1_000_000.0;
        let total_notes = score_judgments.len() as f64;
        let unit = (max_score * 0.5) / total_notes;
        let mut bonus = 100i32;
        let mut total = 0.0f64;
        for j in score_judgments {
            match score_mode {
                ScoreMode::ScoreV1 => {
                    let hv = score_const.hit_value(j.kind) as f64;
                    let hbv = score_const.hit_bonus_value(j.kind) as f64;
                    let base = unit * (hv / 320.0);
                    let bonus_f = bonus.clamp(0, 100) as f64;
                    let bonus_add = unit * (hbv * bonus_f.sqrt() / 320.0);
                    total += base + bonus_add;
                    if j.kind == JudgmentKind::Miss {
                        bonus = 0;
                    } else {
                        let add = score_const.hit_bonus_add(j.kind);
                        let punish = score_const.hit_punish(j.kind);
                        bonus = (bonus + add - punish).clamp(0, 100);
                    }
                }
                ScoreMode::ScoreV2 => {
                    let acc_delta = max_score
                        * score_const.v2_acc_portion
                        * score_const.acc_weight(j.kind) as f64
                        / (total_notes * score_const.acc_max_per_hit as f64);
                    let combo_success = if j.breaks_combo(score_mode) { 0.0 } else { 1.0 };
                    let combo_delta =
                        max_score * score_const.v2_combo_portion * combo_success / total_notes;
                    total += acc_delta + combo_delta;
                }
                ScoreMode::Lazer => {
                    let acc_delta = max_score
                        * score_const.v2_acc_portion
                        * score_const.acc_weight(j.kind) as f64
                        / (total_notes * score_const.acc_max_per_hit as f64);
                    let combo_success = if j.breaks_combo(score_mode) { 0.0 } else { 1.0 };
                    let combo_delta =
                        max_score * score_const.v2_combo_portion * combo_success / total_notes;
                    total += acc_delta + combo_delta;
                }
            }
        }
        Self::score_from_float(total)
    }
    pub(crate) fn score_from_float(raw: f64) -> u32 {
        if !raw.is_finite() {
            return 0;
        }

        let clamped = raw.clamp(0.0, 1_000_000.0);
        let nearest = clamped.round();
        let normalized = if (clamped - nearest).abs() <= SCORE_FLOAT_EPSILON {
            nearest
        } else {
            clamped
        };
        normalized.floor().clamp(0.0, 1_000_000.0) as u32
    }
    pub fn calculate_accuracy(
        score_judgments: &[ScoreJudgmentEvent],
        score_mode: ScoreMode,
    ) -> f64 {
        if score_judgments.is_empty() {
            return 100.0;
        }
        let score_const = ScoreConstants::for_mode(score_mode);
        let mut hits = 0u32;
        let mut max_hits = 0u32;
        for j in score_judgments {
            hits += score_const.acc_weight(j.kind);
            max_hits += score_const.acc_max_per_hit;
        }
        if max_hits == 0 {
            return 100.0;
        }
        (hits as f64 / max_hits as f64) * 100.0
    }
    pub fn count_judgments(score_judgments: &[ScoreJudgmentEvent]) -> [u32; 6] {
        let mut counts = [0u32; 6];
        for j in score_judgments {
            let idx = ScoreConstants::kind_to_idx(j.kind);
            counts[idx] += 1;
        }
        counts
    }
    fn count_judgments_until_event_idx(
        score_events: &[ComboEvent],
        score_judgments: &[ScoreJudgmentEvent],
        last_idx: i32,
    ) -> [u32; 6] {
        if last_idx < 0 {
            return [0u32; 6];
        }
        let mut counts = [0u32; 6];
        for event in score_events.iter().take(last_idx as usize + 1) {
            let Some(judgment_idx) = event.score_judgment_idx else {
                continue;
            };
            let Some(judgment) = score_judgments.get(judgment_idx) else {
                continue;
            };
            counts[ScoreConstants::kind_to_idx(judgment.kind)] += 1;
        }
        counts
    }
    pub fn compute_hud_state(
        &self,
        time: i32,
        score_events: &[ComboEvent],
        score_judgments: &[ScoreJudgmentEvent],
        score_scale: f64,
        frame_idx: usize,
        total_frames: usize,
        score_state_end_time_ms: Option<i32>,
        health_timeline: Option<&HealthTimeline>,
        hit_windows: Option<&Windows>,
    ) -> HudFrameState {
        let fail_started_at = health_timeline
            .and_then(|timeline| timeline.fail_time_ms)
            .filter(|fail_time| time >= *fail_time);
        let visual_query_time = fail_started_at.unwrap_or(time);
        let score_query_time = match fail_started_at {
            Some(fail_time) => score_state_end_time_ms
                .filter(|score_end_time| *score_end_time > fail_time)
                .map(|score_end_time| time.min(score_end_time))
                .unwrap_or(fail_time),
            None => time,
        };
        let life = health_timeline
            .map(|timeline| timeline.life_at_time(visual_query_time))
            .unwrap_or(1.0);
        let progress = self.compute_progress(frame_idx, total_frames);
        let hit_error_windows = hit_windows.map(|windows| HitErrorWindows {
            max: windows.max,
            hit300: windows.hit300,
            hit200: windows.hit200,
            hit100: windows.hit100,
            hit50: windows.hit50,
        });
        if score_events.is_empty() {
            return HudFrameState {
                hud_visible: true,
                progress,
                song_elapsed_ms: score_query_time.max(0),
                song_duration_ms: self.hud_beatmap_metadata.duration_ms.max(0),
                beatmap: self.hud_beatmap_metadata.clone(),
                life,
                hit_error_windows,
                is_break_time: false,
                has_failed: fail_started_at.is_some(),
                fail_started_at,
                ..Default::default()
            };
        }
        let last_idx = Self::find_last_event_idx(score_events, score_query_time);
        let score = if last_idx >= 0 {
            Self::compute_animated_score(
                score_events,
                last_idx as usize,
                score_query_time,
                score_scale,
            )
        } else {
            0
        };
        let (combo, accuracy) = if last_idx >= 0 {
            let ev = &score_events[last_idx as usize];
            let acc = if ev.acc_max_hits > 0 {
                ev.acc_hits as f64 / ev.acc_max_hits as f64
            } else {
                1.0
            };
            (ev.combo_after, acc)
        } else {
            (0, 1.0)
        };
        let judgment_counts =
            Self::count_judgments_until_event_idx(score_events, score_judgments, last_idx);
        let last_judgment =
            Self::find_last_judgment(score_events, score_judgments, last_idx, score_query_time);
        let hit_error_judgments = Self::find_recent_hit_error_judgments(
            score_events,
            score_judgments,
            last_idx,
            score_query_time,
        );
        let hit_error_moving_avg_ms = if last_idx >= 0 {
            score_events
                .get(last_idx as usize)
                .and_then(|event| event.hit_error_moving_avg_ms)
        } else {
            None
        };
        let combo_break_anim =
            Self::find_combo_break_anim(score_events, last_idx, score_query_time);
        let combo_inc_anim = Self::find_combo_inc_anim(score_events, last_idx, score_query_time);
        let combo_burst_anim =
            Self::find_combo_burst_anim(score_events, last_idx, score_query_time);
        HudFrameState {
            hud_visible: true,
            score,
            accuracy,
            combo,
            judgment_counts,
            progress,
            song_elapsed_ms: score_query_time.max(0),
            song_duration_ms: self.hud_beatmap_metadata.duration_ms.max(0),
            beatmap: self.hud_beatmap_metadata.clone(),
            life,
            is_break_time: false,
            has_failed: fail_started_at.is_some(),
            fail_started_at,
            last_judgment,
            hit_error_judgments,
            hit_error_moving_avg_ms,
            hit_error_windows,
            combo_break_anim,
            combo_inc_anim,
            combo_burst_anim,
            pp_available: false,
            ..Default::default()
        }
    }
    #[inline]
    fn find_last_event_idx(events: &[ComboEvent], time: i32) -> i32 {
        if events.is_empty() {
            return -1;
        }
        let mut lo = 0i32;
        let mut hi = events.len() as i32 - 1;
        let mut result = -1i32;
        while lo <= hi {
            let mid = (lo + hi) >> 1;
            if events[mid as usize].time <= time {
                result = mid;
                lo = mid + 1;
            } else {
                hi = mid - 1;
            }
        }
        result
    }
    pub fn set_hud_pp_timeline(&mut self, timeline: Vec<(i32, f32)>, final_pp: Option<f32>) {
        self.hud_pp_timeline = timeline;
        self.hud_pp_timeline.sort_by_key(|(time, _)| *time);
        self.hud_pp_final = final_pp.or_else(|| self.hud_pp_timeline.last().map(|(_, pp)| *pp));
        self.hud_pp_warning = if self.hud_pp_final.is_none() {
            Some("PP counter unavailable for this render".to_string())
        } else {
            None
        };
    }
    pub fn set_hud_unstable_rate(&mut self, unstable_rate: Option<f32>) {
        self.hud_unstable_rate = unstable_rate.filter(|value| value.is_finite());
    }
    pub fn set_hud_beatmap_metadata(&mut self, metadata: HudBeatmapMetadataState) {
        self.hud_beatmap_metadata = metadata;
    }
    fn hud_kps_graph_timing(&self) -> (i32, i32) {
        fn scan(nodes: &[crate::hud::HudLayerConfig], interval: &mut i32, window: &mut i32) {
            for node in nodes {
                // Graph timing is data-driven so custom HUDs can request denser or longer KPS samples.
                if node.layer_type == "graph.sparkline" {
                    if let Some(value) = node
                        .props
                        .get("sampleIntervalMs")
                        .and_then(serde_json::Value::as_i64)
                    {
                        *interval = (*interval).min((value as i32).clamp(40, 1000));
                    }
                    if let Some(value) = node
                        .props
                        .get("sampleWindowMs")
                        .and_then(serde_json::Value::as_i64)
                    {
                        *window = (*window).max((value as i32).clamp(1000, 30_000));
                    }
                }
                if !node.children.is_empty() {
                    scan(&node.children, interval, window);
                }
            }
        }
        let mut interval = 120;
        let mut window = 5000;
        if let Some(config) = self
            .hud_config
            .as_ref()
            .filter(|config| config.version == Some(4))
        {
            scan(&config.nodes, &mut interval, &mut window);
            scan(&config.layers, &mut interval, &mut window);
        }
        (interval, window)
    }
    pub(super) fn enrich_hud_state(
        &mut self,
        hud_state: &mut HudFrameState,
        time: i32,
        key_mask: u32,
    ) {
        let pressed = key_mask & !self.hud_key_last_mask;
        let released = self.hud_key_last_mask & !key_mask;
        for index in 0..32 {
            let bit = 1_u32 << index;
            if pressed & bit != 0 {
                self.hud_key_press_times.push_back((time, index as u8));
                self.hud_key_down_since[index] = Some(time);
            }
            if released & bit != 0 {
                if let Some(start) = self.hud_key_down_since[index] {
                    self.hud_key_tail_releases
                        .push_back(super::render::HudKeyTailRelease {
                            key_index: index,
                            released_at_ms: time,
                            duration_ms: (time - start).max(0),
                        });
                }
                self.hud_key_down_since[index] = None;
            }
        }
        self.hud_key_last_mask = key_mask;
        while self
            .hud_key_press_times
            .front()
            .is_some_and(|(press_time, _)| *press_time < time - 1000)
        {
            self.hud_key_press_times.pop_front();
        }
        while self
            .hud_key_tail_releases
            .front()
            .is_some_and(|release| release.released_at_ms < time - 5000)
        {
            self.hud_key_tail_releases.pop_front();
        }
        hud_state.key_down_mask = key_mask;
        hud_state.key_kps = [0.0; 32];
        hud_state.key_press_duration_ms = [0; 32];
        for (_, key_index) in self.hud_key_press_times.iter() {
            let index = *key_index as usize;
            if index < hud_state.key_kps.len() {
                hud_state.key_kps[index] += 1.0;
            }
        }
        for index in 0..32 {
            if key_mask & (1_u32 << index) != 0 {
                if let Some(start) = self.hud_key_down_since[index] {
                    hud_state.key_press_duration_ms[index] = (time - start).max(0);
                }
            }
        }
        hud_state.total_kps = self.hud_key_press_times.len() as f32;
        let (sample_interval_ms, sample_window_ms) = self.hud_kps_graph_timing();
        let should_sample_kps = self
            .hud_last_kps_sample_time
            .map(|last_sample_time| time - last_sample_time >= sample_interval_ms)
            .unwrap_or(true);
        if should_sample_kps {
            // KPS samples are retained separately from raw key presses so graph windows can be wider than one second.
            self.hud_kps_samples.push_back((time, hud_state.total_kps));
            self.hud_last_kps_sample_time = Some(time);
        }
        while self
            .hud_kps_samples
            .front()
            .is_some_and(|(sample_time, _)| *sample_time < time - sample_window_ms)
        {
            self.hud_kps_samples.pop_front();
        }
        let pp_idx = self
            .hud_pp_timeline
            .partition_point(|(sample_time, _)| *sample_time <= time);
        hud_state.pp_current = pp_idx
            .checked_sub(1)
            .and_then(|idx| self.hud_pp_timeline.get(idx).map(|(_, pp)| *pp));
        hud_state.pp_final = self.hud_pp_final;
        hud_state.pp_available = hud_state.pp_current.is_some() || hud_state.pp_final.is_some();
        hud_state.unstable_rate = self.hud_unstable_rate;
    }
    fn compute_animated_score(
        events: &[ComboEvent],
        last_idx: usize,
        time: i32,
        score_scale: f64,
    ) -> u32 {
        let settle_time = time - anim::SCORE_ANIM_MS;
        // Older score deltas are fully settled; recent deltas are blended in for the rolling score animation.
        let mut lo = 0i32;
        let mut hi = last_idx as i32;
        let mut last_settled = -1i32;
        while lo <= hi {
            let mid = (lo + hi) >> 1;
            if events[mid as usize].time <= settle_time {
                last_settled = mid;
                lo = mid + 1;
            } else {
                hi = mid - 1;
            }
        }
        let settled_score = if last_settled >= 0 {
            events[last_settled as usize].cumulative_score
        } else {
            0.0
        };
        let mut partial = 0.0f64;
        let start = (last_settled + 1) as usize;
        for i in start..=last_idx {
            let ev = &events[i];
            let f = ((time - ev.time) as f64 / anim::SCORE_ANIM_MS as f64).clamp(0.0, 1.0);
            partial += ev.score_delta * f;
        }
        let raw = (settled_score + partial) * score_scale;
        Self::score_from_float(raw)
    }
    fn find_last_judgment(
        events: &[ComboEvent],
        score_judgments: &[ScoreJudgmentEvent],
        last_idx: i32,
        time: i32,
    ) -> Option<LastJudgment> {
        if last_idx < 0 || score_judgments.is_empty() {
            return None;
        }
        for k in (0..=last_idx as usize).rev() {
            let ev = &events[k];
            if ev.event_type != ComboEventType::Judgment {
                continue;
            }
            let age = time - ev.time;
            if age > anim::LAST_JUDGMENT_AGE_MS {
                break;
            }
            if let Some(j_idx) = ev.score_judgment_idx {
                if let Some(j) = score_judgments.get(j_idx) {
                    return Some(LastJudgment {
                        kind: j.kind,
                        age_ms: age,
                        column: j.column,
                        hit_offset_ms: j.hit_error_offset_ms,
                    });
                }
            }
            // Older cached events may lack an index; matching by time preserves the visible judgment popup.
            if let Some(j) = score_judgments.iter().find(|jj| jj.event_time == ev.time) {
                return Some(LastJudgment {
                    kind: j.kind,
                    age_ms: age,
                    column: j.column,
                    hit_offset_ms: j.hit_error_offset_ms,
                });
            }
        }
        None
    }
    fn find_recent_hit_error_judgments(
        events: &[ComboEvent],
        score_judgments: &[ScoreJudgmentEvent],
        last_idx: i32,
        time: i32,
    ) -> Vec<HitErrorJudgment> {
        if last_idx < 0 {
            return Vec::new();
        }
        const HIT_ERROR_FADE_MS: i32 = 5000;
        const HIT_ERROR_MAX_LINES: usize = 50;
        let mut judgments = Vec::new();
        for ev in (0..=last_idx as usize).rev().map(|idx| &events[idx]) {
            let age = time - ev.time;
            if age > HIT_ERROR_FADE_MS {
                break;
            }
            if age < 0 || ev.event_type != ComboEventType::Judgment {
                continue;
            }
            let Some(offset_ms) = ev.hit_error_offset_ms else {
                continue;
            };
            let Some(judgment_idx) = ev.score_judgment_idx else {
                continue;
            };
            let Some(score_judgment) = score_judgments.get(judgment_idx) else {
                continue;
            };
            judgments.push(HitErrorJudgment {
                kind: score_judgment.kind,
                offset_ms,
                age_ms: age,
            });
            if judgments.len() >= HIT_ERROR_MAX_LINES {
                break;
            }
        }
        judgments.reverse();
        judgments
    }
    fn find_combo_break_anim(
        events: &[ComboEvent],
        last_idx: i32,
        time: i32,
    ) -> Option<ComboBreakAnimation> {
        if last_idx < 0 {
            return None;
        }
        for k in (0..=last_idx as usize).rev() {
            let ev = &events[k];
            if let Some(start_combo) = ev.combo_break_start {
                if start_combo > 0 {
                    let elapsed = time - ev.time;
                    if (0..anim::COMBO_BREAK_DURATION_MS).contains(&elapsed) {
                        return Some(ComboBreakAnimation {
                            start_combo,
                            break_time: ev.time,
                            age_ms: elapsed,
                            column: 0,
                        });
                    } else if elapsed >= anim::COMBO_BREAK_DURATION_MS {
                        break;
                    }
                }
            }
        }
        None
    }
    fn find_combo_inc_anim(
        events: &[ComboEvent],
        last_idx: i32,
        time: i32,
    ) -> Option<ComboIncAnimation> {
        if last_idx < 0 {
            return None;
        }
        for k in (0..=last_idx as usize).rev() {
            let ev = &events[k];
            if ev.combo_after > 0 && ev.combo_break_start.is_none() {
                let elapsed = time - ev.time;
                if (0..anim::COMBO_INC_ANIM_MS).contains(&elapsed) {
                    return Some(ComboIncAnimation {
                        time: ev.time,
                        age_ms: elapsed,
                    });
                } else if elapsed >= anim::COMBO_INC_ANIM_MS {
                    break;
                }
            }
        }
        None
    }
    fn find_combo_burst_anim(
        events: &[ComboEvent],
        last_idx: i32,
        time: i32,
    ) -> Option<ComboBurstAnimation> {
        if last_idx < 0 {
            return None;
        }
        for k in (0..=last_idx as usize).rev() {
            let ev = &events[k];
            if ev.combo_after == 0 || !ev.combo_after.is_multiple_of(100) {
                continue;
            }
            let elapsed = time - ev.time;
            if (0..anim::COMBO_BURST_ANIM_MS).contains(&elapsed) {
                return Some(ComboBurstAnimation {
                    combo: ev.combo_after,
                    time: ev.time,
                    age_ms: elapsed,
                });
            }
            if elapsed >= anim::COMBO_BURST_ANIM_MS {
                break;
            }
        }
        None
    }
    #[inline]
    fn compute_progress(&self, frame_idx: usize, total_frames: usize) -> f32 {
        if total_frames <= 1 {
            return 1.0;
        }
        (frame_idx as f32 / (total_frames - 1) as f32).clamp(0.0, 1.0)
    }
    pub fn get_frame_range(
        &self,
        plan: &RenderPlan,
        start_seconds: Option<f64>,
        end_seconds: Option<f64>,
    ) -> (usize, usize) {
        let mut i_start = 0usize;
        let mut i_end = plan.total_frames;
        if let Some(ss) = start_seconds {
            let start_ms = (ss * 1000.0) as i32;
            i_start = ((start_ms - plan.timeline_start) as f64 / plan.frame_time)
                .floor()
                .max(0.0) as usize;
        }
        if let Some(es) = end_seconds {
            let end_ms = (es * 1000.0) as i32;
            let calc = ((end_ms - plan.timeline_start) as f64 / plan.frame_time).ceil() as usize;
            i_end = calc.min(plan.total_frames).max(i_start);
        }
        (i_start, i_end)
    }
    pub fn compute_effective_ends(notes: &[crate::types::HitObject]) -> Vec<i32> {
        notes
            .iter()
            .map(|ho| {
                if let Some(end) = ho.end_time {
                    // Tiny end-time offsets are treated as normal notes to avoid keeping short taps alive.
                    if end > ho.time + 2 {
                        return end;
                    }
                }
                ho.time
            })
            .collect()
    }
    pub fn prepare_sorted_notes(notes: &[crate::types::HitObject], num_columns: u8) -> Vec<usize> {
        let mut indices: Vec<usize> = notes
            .iter()
            .enumerate()
            .filter(|(_, ho)| ho.column < num_columns)
            .map(|(i, _)| i)
            .collect();
        indices.sort_by_key(|&i| notes[i].time);
        indices
    }
    pub fn compute_plan_from_notes(
        &self,
        notes: &[crate::types::HitObject],
        replay_end_ms: i32,
        lead_in_ms: i32,
    ) -> Result<RenderPlan, String> {
        let first_note = notes.iter().map(|n| n.time).min().unwrap_or(0);
        let last_note = notes
            .iter()
            .map(|n| n.end_time.unwrap_or(n.time))
            .max()
            .unwrap_or(0);
        let pps = 800.0;
        let travel_ms = self.cfg.height as f64 / pps * 1000.0;
        let timeline_start = i64::from(first_note) - i64::from(lead_in_ms) - travel_ms as i64;
        let timeline_end = i64::from(last_note.max(replay_end_ms)) + 500_i64;
        let frame_time = 1000.0 / self.cfg.fps.max(1) as f64;
        let total_frames = ((timeline_end - timeline_start) as f64 / frame_time).ceil() as usize;
        Ok(RenderPlan {
            timeline_start: i32::try_from(timeline_start).map_err(|_| {
                format!("render timeline start is out of i32 range: {timeline_start} ms")
            })?,
            timeline_end: i32::try_from(timeline_end).map_err(|_| {
                format!("render timeline end is out of i32 range: {timeline_end} ms")
            })?,
            frame_time,
            total_frames,
            travel_ms,
        })
    }
}
