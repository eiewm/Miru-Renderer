use super::layout::ManiaLayout;
use crate::types::TimingPoint;
use std::collections::HashMap;
pub const SPEED_CALIB_A: f64 = 0.05448;
pub const SPEED_CALIB_B: f64 = 0.42740;
const TIMING_BEAT_LENGTH_DEFAULT: f64 = 1000.0;
const TIMING_BEAT_LENGTH_MIN: f64 = 6.0;
const TIMING_BEAT_LENGTH_MAX: f64 = 60000.0;
const LEGACY_GREEN_BEAT_LENGTH_MIN: f64 = 10.0;
const LEGACY_GREEN_BEAT_LENGTH_MAX: f64 = 10000.0;
const EFFECT_SCROLL_MIN: f64 = 0.01;
const EFFECT_SCROLL_MAX: f64 = 10.0;
const TIME_EPSILON: f64 = 1e-6;
#[derive(Debug, Clone, Copy, Default)]
pub struct ScrollSpeedPoint {
    pub time: f64,
    pub multiplier: f64,
}
#[derive(Debug, Clone, Copy, Default)]
struct StableTimingState {
    time: f64,
    raw_red_beat_length: f64,
    red_beat_length: f64,
    green_factor: f64,
    timing_change: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StableScrollBehavior {
    use_raw_positive_red_beat_length: bool,
}
impl StableScrollBehavior {
    pub const fn legacy_clamped_red() -> Self {
        Self {
            use_raw_positive_red_beat_length: false,
        }
    }
    pub const fn stable_reference_unclamped_red() -> Self {
        Self {
            use_raw_positive_red_beat_length: true,
        }
    }
    pub const fn uses_raw_positive_red_beat_length(self) -> bool {
        self.use_raw_positive_red_beat_length
    }
}
#[derive(Debug, Clone)]
pub struct ScrollSpeedTimeline {
    points: Vec<ScrollSpeedPoint>,
    prefix_dist: Vec<f64>,
    pps_base: f64,
}
impl ScrollSpeedTimeline {
    pub fn new(mut points: Vec<ScrollSpeedPoint>, pps_base: f64) -> Self {
        points.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        let mut optimized: Vec<ScrollSpeedPoint> = Vec::with_capacity(points.len());
        let mut last_mult: Option<f64> = None;
        for p in points {
            // Later points at the same time override earlier ones, matching timing-point ordering.
            if let Some(prev) = optimized.last() {
                if (p.time - prev.time).abs() < 1e-6 {
                    optimized.pop();
                }
            }
            let is_redundant = last_mult
                .map(|m| (p.multiplier - m).abs() < 1e-6)
                .unwrap_or(false);
            if !is_redundant {
                last_mult = Some(p.multiplier);
                optimized.push(p);
            }
        }
        if optimized.is_empty() {
            optimized.push(ScrollSpeedPoint {
                time: 0.0,
                multiplier: 1.0,
            });
        } else if optimized[0].time.abs() > 1e-6 {
            let mult = optimized[0].multiplier;
            optimized.insert(
                0,
                ScrollSpeedPoint {
                    time: 0.0,
                    multiplier: mult,
                },
            );
        }
        let mut prefix_dist = Vec::with_capacity(optimized.len());
        let mut dist = 0.0f64;
        let mut prev_time = 0.0f64;
        let mut prev_mult = optimized[0].multiplier;
        for pt in &optimized {
            // Prefix distances make arbitrary time-to-time distance queries logarithmic.
            let dt = (pt.time - prev_time).max(0.0);
            if dt > 0.0 && pps_base.is_finite() {
                dist += (pps_base * prev_mult * dt) / 1000.0;
            }
            prefix_dist.push(dist);
            prev_time = pt.time;
            prev_mult = pt.multiplier;
        }
        Self {
            points: optimized,
            prefix_dist,
            pps_base,
        }
    }
    pub fn distance_px(&self, from_time: i32, to_time: i32) -> f32 {
        let t0 = from_time as f64;
        let t1 = to_time as f64;
        let dist = self.prefix_distance(t1) - self.prefix_distance(t0);
        dist as f32
    }
    pub fn distance_at_ms(&self, time_ms: i32) -> f64 {
        self.prefix_distance(time_ms as f64)
    }
    pub fn time_at_distance_ms(&self, from_ms: i32, distance_px: f64) -> i32 {
        if distance_px <= 0.0 || !distance_px.is_finite() {
            return from_ms;
        }
        if self.points.is_empty() || !self.pps_base.is_finite() || self.pps_base <= 0.0 {
            return from_ms;
        }
        let from_time = from_ms as f64;
        let start_dist = self.prefix_distance(from_time);
        let target_dist = start_dist + distance_px;
        if !target_dist.is_finite() {
            return from_ms;
        }
        let mut idx = self.prefix_dist.partition_point(|d| *d <= target_dist);
        idx = idx.saturating_sub(1);
        if idx >= self.points.len() {
            idx = self.points.len().saturating_sub(1);
        }
        let mut base_dist = self.prefix_dist.get(idx).copied().unwrap_or(0.0);
        let mut base_time = self.points[idx].time;
        let mut mult = self.points[idx].multiplier;
        const EPS: f64 = 1e-6;
        while mult.abs() <= EPS {
            // Zero-speed segments cannot be inverted; move to the next usable segment.
            idx += 1;
            if idx >= self.points.len() {
                return from_ms;
            }
            base_dist = self.prefix_dist[idx];
            base_time = self.points[idx].time;
            mult = self.points[idx].multiplier;
        }
        let denom = self.pps_base * mult;
        if denom.abs() <= EPS {
            return from_ms;
        }
        let dt = ((target_dist - base_dist) * 1000.0 / denom).max(0.0);
        let t = base_time + dt;
        if !t.is_finite() {
            return from_ms;
        }
        let t = t.ceil();
        let clamped = if t > i32::MAX as f64 {
            i32::MAX
        } else if t < i32::MIN as f64 {
            i32::MIN
        } else {
            t as i32
        };
        clamped.max(from_ms)
    }
    pub fn multiplier_at_ms(&self, time_ms: i32) -> f64 {
        self.multiplier_at(time_ms as f64)
    }
    fn multiplier_at(&self, time: f64) -> f64 {
        if self.points.is_empty() {
            return 1.0;
        }
        let idx = self.points.partition_point(|p| p.time <= time);
        if idx == 0 {
            return self.points[0].multiplier;
        }
        self.points[idx - 1].multiplier
    }
    fn prefix_distance(&self, time: f64) -> f64 {
        if self.points.is_empty() {
            return (self.pps_base * time) / 1000.0;
        }
        if time <= self.points[0].time {
            return (self.pps_base * self.points[0].multiplier * (time - self.points[0].time))
                / 1000.0;
        }
        let idx = self.points.partition_point(|p| p.time <= time);
        let idx = idx.saturating_sub(1);
        let base_dist = self.prefix_dist.get(idx).copied().unwrap_or(0.0);
        let pt = &self.points[idx];
        let extra = (time - pt.time).max(0.0);
        base_dist + (self.pps_base * pt.multiplier * extra) / 1000.0
    }
}
#[derive(Debug, Clone)]
pub struct StableScrollModel {
    object: ScrollSpeedTimeline,
    grid: ScrollSpeedTimeline,
}
impl StableScrollModel {
    pub fn new(object: ScrollSpeedTimeline, grid: ScrollSpeedTimeline) -> Self {
        Self { object, grid }
    }
    pub fn object_distance_px(&self, from_time: i32, to_time: i32) -> f32 {
        self.object.distance_px(from_time, to_time)
    }
    pub fn grid_distance_px(&self, from_time: i32, to_time: i32) -> f32 {
        self.grid.distance_px(from_time, to_time)
    }
    pub fn object_distance_at_ms(&self, time_ms: i32) -> f64 {
        self.object.distance_at_ms(time_ms)
    }
    pub fn object_multiplier_at_ms(&self, time_ms: i32) -> f64 {
        self.object.multiplier_at_ms(time_ms)
    }
    pub fn grid_multiplier_at_ms(&self, time_ms: i32) -> f64 {
        self.grid.multiplier_at_ms(time_ms)
    }
    pub fn object_timeline(&self) -> &ScrollSpeedTimeline {
        &self.object
    }
    pub fn grid_timeline(&self) -> &ScrollSpeedTimeline {
        &self.grid
    }
}
pub fn travel_time_sec(ss: f64, sv: f64) -> f64 {
    let denom = (sv * (SPEED_CALIB_A * ss + SPEED_CALIB_B)).max(1e-9);
    1.0 / denom
}
pub fn get_visible_travel_distance_px(layout: &ManiaLayout, spawn_offset: i32) -> f64 {
    let spawn_y = layout.stage.top_y + spawn_offset;
    (layout.stage.hit_y - spawn_y).max(1) as f64
}
pub fn pixels_per_ms_from_ss(ss: f64, sv: f64, layout: &ManiaLayout, spawn_offset: i32) -> f64 {
    let dist = get_visible_travel_distance_px(layout, spawn_offset);
    let t_sec = travel_time_sec(ss, sv);
    (dist / t_sec) / 1000.0
}
pub fn ss_to_pixels_per_second(ss: f64, layout: &ManiaLayout) -> f64 {
    pixels_per_ms_from_ss(ss, 1.0, layout, 0) * 1000.0
}
pub fn compute_travel_ms(layout: &ManiaLayout, pps: f64, note_head_h: i32, margin: i32) -> i32 {
    let spawn_y = layout.stage.top_y - note_head_h - margin;
    let travel_px = (layout.stage.hit_y - spawn_y) as f64;
    (travel_px / (pps / 1000.0)).ceil() as i32
}
pub fn compute_timeline_start_ms(
    first_obj_ms: i32,
    travel_ms: i32,
    audio_lead_in: i32,
    user_lead_in: i32,
    safety_ms: i32,
) -> i32 {
    first_obj_ms - travel_ms - audio_lead_in - user_lead_in - safety_ms
}
pub fn get_inherited_points_sorted(timing_points: &[TimingPoint]) -> Vec<TimingPoint> {
    let mut inherited: Vec<TimingPoint> = timing_points
        .iter()
        .filter(|p| !p.uninherited)
        .copied()
        .collect();
    let all_sorted: Vec<_> = {
        let mut v: Vec<_> = timing_points.to_vec();
        v.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        v
    };
    for tp in &all_sorted {
        if inherited.iter().any(|p| (p.time - tp.time).abs() < 0.1) {
            continue;
        }
        let is_uninherited = tp.uninherited;
        if !is_uninherited {
            continue;
        }
        let has_prev_sv = inherited.iter().any(|p| p.time < tp.time);
        if !has_prev_sv {
            continue;
        }
        // Insert neutral green points at red timing changes so inherited SV carries across BPM changes.
        inherited.push(TimingPoint {
            time: tp.time,
            beat_length: -100.0,
            meter: tp.meter,
            sample_set: tp.sample_set,
            sample_index: tp.sample_index,
            volume: tp.volume,
            uninherited: false,
            effects: tp.effects,
        });
    }
    inherited.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    optimize_inherited_points(&inherited)
}
fn optimize_inherited_points(points: &[TimingPoint]) -> Vec<TimingPoint> {
    if points.len() <= 1 {
        return points.to_vec();
    }
    let mut result = Vec::with_capacity(points.len());
    let mut prev_sv = -1.0f64;
    let mut prev_time = f64::NEG_INFINITY;
    const EPSILON: f64 = 0.001;
    for (i, pt) in points.iter().enumerate() {
        let bl = pt.beat_length;
        if bl > 100000.0 {
            // Extremely large inherited beat lengths act as scroll freezes.
            result.push(*pt);
            prev_sv = 0.0001;
            prev_time = pt.time;
            continue;
        }
        let sv = 100.0 / bl.abs().max(1e-9);
        if i == 0 {
            result.push(*pt);
            prev_sv = sv;
            prev_time = pt.time;
            continue;
        }
        if i == points.len() - 1 {
            if (sv - prev_sv).abs() > EPSILON || (pt.time - prev_time) > 5000.0 {
                result.push(*pt);
            }
            continue;
        }
        let diff = (sv - prev_sv).abs();
        let ratio = diff / prev_sv.abs().max(0.001);
        if diff > EPSILON || ratio > 0.001 || (pt.time - prev_time) > 5000.0 {
            result.push(*pt);
            prev_sv = sv;
            prev_time = pt.time;
        }
    }
    result
}
pub fn get_scroll_velocity_at(t: f64, inherited: &[TimingPoint]) -> f64 {
    if inherited.is_empty() {
        return 1.0;
    }
    let idx = inherited.partition_point(|p| p.time <= t);
    if idx == 0 {
        return 1.0;
    }
    let p = &inherited[idx - 1];
    let bl = p.beat_length;
    if bl > 100000.0 {
        return 0.0001;
    }
    100.0 / bl.abs().max(1e-9)
}
pub fn integrate_scroll_px(t0: f64, t1: f64, pps_base: f64, inherited: &[TimingPoint]) -> f64 {
    if t1 <= t0 {
        return 0.0;
    }
    if inherited.is_empty() {
        return pps_base * (t1 - t0) / 1000.0;
    }
    let mut cur = t0;
    let mut cur_idx = inherited.partition_point(|p| p.time <= t0);
    cur_idx = cur_idx.saturating_sub(1);
    let mut px = 0.0;
    let sv_of = |idx: usize| -> f64 {
        if idx >= inherited.len() {
            return 1.0;
        }
        let bl = inherited[idx].beat_length;
        if bl > 100000.0 {
            return 0.0001;
        }
        100.0 / bl.abs().max(1e-9)
    };
    let is_freeze = |idx: usize| -> bool {
        if idx >= inherited.len() {
            return false;
        }
        inherited[idx].beat_length > 100000.0
    };
    let mut cur_sv = if cur_idx > 0 || (cur_idx == 0 && inherited[0].time <= t0) {
        sv_of(cur_idx)
    } else {
        1.0
    };
    while cur < t1 {
        let next_time = if cur_idx + 1 < inherited.len() {
            inherited[cur_idx + 1].time
        } else {
            t1
        };
        let seg_end = t1.min(next_time);
        let dt = (seg_end - cur).max(0.0);
        if dt > 0.0 {
            if !is_freeze(cur_idx) {
                let sv_eff = cur_sv.max(0.0001);
                px += (pps_base * sv_eff * dt) / 1000.0;
            }
            cur = seg_end;
        }
        if cur >= t1 {
            break;
        }
        cur_idx += 1;
        cur_sv = sv_of(cur_idx);
    }
    px
}
pub fn logical_spacing_px(t0: f64, t1: f64, pps_base: f64) -> f64 {
    if t1 <= t0 {
        return 0.0;
    }
    pps_base * (t1 - t0) / 1000.0
}
pub fn compute_barlines(timing_points: &[TimingPoint], start: f64, end: f64) -> Vec<f64> {
    let mut bars = Vec::new();
    const MAX_BARLINES: usize = 10000;
    let mut uninherited: Vec<_> = timing_points.iter().filter(|p| p.uninherited).collect();
    uninherited.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    if uninherited.is_empty() {
        return bars;
    }
    for i in 0..uninherited.len() {
        let tp = uninherited[i];
        let seg_end = uninherited.get(i + 1).map(|p| p.time).unwrap_or(end);
        if seg_end < start {
            continue;
        }
        let beat = tp.beat_length;
        let meter = tp.meter.max(1) as f64;
        let bar_dur = beat * meter;
        if !bar_dur.is_finite() || bar_dur <= 0.0 || bar_dur > 1_000_000.0 {
            continue;
        }
        let mut bar_time = tp.time;
        if bar_time < start {
            let k = ((start - bar_time) / bar_dur).ceil();
            bar_time += k * bar_dur;
        }
        while bar_time <= seg_end && bar_time <= end && bars.len() < MAX_BARLINES {
            if bar_time >= start {
                bars.push(bar_time);
            }
            bar_time += bar_dur;
        }
        if bars.len() >= MAX_BARLINES {
            break;
        }
    }
    bars
}
fn normalize_timing_beat_length(beat_length: f64) -> f64 {
    if !beat_length.is_finite() {
        return TIMING_BEAT_LENGTH_DEFAULT;
    }
    beat_length.clamp(TIMING_BEAT_LENGTH_MIN, TIMING_BEAT_LENGTH_MAX)
}
fn effect_scroll_speed_from_beat_length(beat_length: f64) -> f64 {
    if !beat_length.is_finite() {
        return 1.0;
    }
    let speed = if beat_length < 0.0 {
        100.0 / (-beat_length).max(1e-9)
    } else {
        1.0
    };
    speed.clamp(EFFECT_SCROLL_MIN, EFFECT_SCROLL_MAX)
}
fn beat_length_to_bpm(beat_length: f64) -> f64 {
    let safe_beat_length = normalize_timing_beat_length(beat_length);
    if safe_beat_length.abs() < 1e-9 {
        return 0.0;
    }
    60000.0 / safe_beat_length
}
fn sv_multiplier_from_beat_length(beat_length: f64) -> f64 {
    effect_scroll_speed_from_beat_length(beat_length)
}
fn legacy_green_factor_from_beat_length(beat_length: f64) -> f64 {
    if !beat_length.is_finite() {
        return 1.0;
    }
    if beat_length < 0.0 {
        (-beat_length).clamp(LEGACY_GREEN_BEAT_LENGTH_MIN, LEGACY_GREEN_BEAT_LENGTH_MAX) / 100.0
    } else {
        1.0
    }
}
fn build_stable_timing_states(timing_points: &[TimingPoint]) -> Vec<StableTimingState> {
    let mut points: Vec<(usize, TimingPoint)> = timing_points
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, p)| p.time.is_finite())
        .collect();
    points.sort_by(|a, b| {
        let tcmp = a.1.time.total_cmp(&b.1.time);
        if tcmp == std::cmp::Ordering::Equal {
            a.0.cmp(&b.0)
        } else {
            tcmp
        }
    });
    if points.is_empty() {
        return vec![StableTimingState {
            time: 0.0,
            raw_red_beat_length: TIMING_BEAT_LENGTH_DEFAULT,
            red_beat_length: TIMING_BEAT_LENGTH_DEFAULT,
            green_factor: 1.0,
            timing_change: false,
        }];
    }
    let mut states = Vec::with_capacity(points.len());
    let mut active_raw_red = TIMING_BEAT_LENGTH_DEFAULT;
    let mut active_red = TIMING_BEAT_LENGTH_DEFAULT;
    let mut active_green_factor = 1.0;
    let mut i = 0usize;
    while i < points.len() {
        let group_time = points[i].1.time;
        let mut j = i;
        let mut first_red: Option<TimingPoint> = None;
        let mut last_green: Option<TimingPoint> = None;
        // Same-time points are evaluated as one stable timing state: first red, last green.
        while j < points.len() && (points[j].1.time - group_time).abs() <= TIME_EPSILON {
            let point = points[j].1;
            if point.uninherited {
                if first_red.is_none() {
                    first_red = Some(point);
                }
            } else {
                last_green = Some(point);
            }
            j += 1;
        }
        let mut timing_change = false;
        if let Some(red) = first_red {
            active_raw_red = red.beat_length;
            active_red = normalize_timing_beat_length(red.beat_length);
            active_green_factor = 1.0;
            timing_change = true;
        }
        if let Some(green) = last_green {
            active_green_factor = legacy_green_factor_from_beat_length(green.beat_length);
        }
        states.push(StableTimingState {
            time: group_time,
            raw_red_beat_length: active_raw_red,
            red_beat_length: active_red,
            green_factor: active_green_factor,
            timing_change,
        });
        i = j;
    }
    states
}
pub fn most_common_beat_length(
    timing_points: &[TimingPoint],
    last_object_time_ms: Option<i32>,
) -> f64 {
    let states = build_stable_timing_states(timing_points);
    let timing_changes: Vec<(f64, f64)> = states
        .iter()
        .filter(|s| s.timing_change)
        .map(|s| (s.time, s.red_beat_length))
        .collect();
    if timing_changes.is_empty() {
        return TIMING_BEAT_LENGTH_DEFAULT;
    }
    let last_time = last_object_time_ms
        .map(f64::from)
        .or_else(|| timing_changes.last().map(|(t, _)| *t))
        .unwrap_or(0.0);
    let mut durations_by_key: HashMap<i64, f64> = HashMap::new();
    let mut key_order: Vec<i64> = Vec::new();
    for i in 0..timing_changes.len() {
        let (time, beat_length) = timing_changes[i];
        let mut current_time = time;
        if i == 0 && current_time > 0.0 {
            current_time = 0.0;
        }
        // Stable's mode BPM is effectively the red beat length with the most covered duration.
        let next_time = timing_changes
            .get(i + 1)
            .map(|(t, _)| *t)
            .unwrap_or(last_time);
        let duration = if current_time > last_time {
            0.0
        } else {
            (next_time - current_time).max(0.0)
        };
        let rounded_key = (beat_length * 1000.0).round() as i64;
        let entry = durations_by_key.entry(rounded_key).or_insert_with(|| {
            key_order.push(rounded_key);
            0.0
        });
        *entry += duration;
    }
    let mut best_key = key_order
        .first()
        .copied()
        .unwrap_or((TIMING_BEAT_LENGTH_DEFAULT * 1000.0).round() as i64);
    let mut best_duration = f64::NEG_INFINITY;
    for key in key_order {
        let duration = durations_by_key.get(&key).copied().unwrap_or(0.0);
        if duration > best_duration {
            best_duration = duration;
            best_key = key;
        }
    }
    let selected = best_key as f64 / 1000.0;
    let min_bl = timing_changes
        .iter()
        .map(|(_, bl)| *bl)
        .fold(f64::INFINITY, f64::min);
    let max_bl = timing_changes
        .iter()
        .map(|(_, bl)| *bl)
        .fold(f64::NEG_INFINITY, f64::max);
    selected.clamp(min_bl, max_bl)
}
pub fn bpm_at(t: f64, bpm_points_sorted: &[TimingPoint], default_bpm: f64) -> f64 {
    if bpm_points_sorted.is_empty() {
        return default_bpm;
    }
    let idx = bpm_points_sorted.partition_point(|p| p.time <= t);
    let p = if idx == 0 {
        &bpm_points_sorted[0]
    } else {
        &bpm_points_sorted[idx - 1]
    };
    let bpm = beat_length_to_bpm(p.beat_length);
    if bpm.is_finite() && bpm > 0.0 {
        bpm
    } else {
        default_bpm
    }
}
pub fn sv_at(t: f64, sv_points_sorted: &[TimingPoint]) -> f64 {
    if sv_points_sorted.is_empty() {
        return 1.0;
    }
    let idx = sv_points_sorted.partition_point(|p| p.time <= t);
    if idx == 0 {
        return 1.0;
    }
    let p = &sv_points_sorted[idx - 1];
    sv_multiplier_from_beat_length(p.beat_length)
}
pub fn effective_multiplier_at(
    t: f64,
    bpm_points_sorted: &[TimingPoint],
    sv_points_sorted: &[TimingPoint],
    mode_bpm: f64,
) -> f64 {
    let safe_mode = if mode_bpm.is_finite() && mode_bpm > 0.0 {
        mode_bpm
    } else {
        1.0
    };
    let mut combined = Vec::with_capacity(bpm_points_sorted.len() + sv_points_sorted.len());
    combined.extend(bpm_points_sorted.iter().copied());
    combined.extend(sv_points_sorted.iter().copied());
    combined.sort_by(|a, b| {
        let tcmp = a.time.total_cmp(&b.time);
        if tcmp == std::cmp::Ordering::Equal {
            // Red points win same-time ordering before build_stable_timing_states groups them.
            b.uninherited.cmp(&a.uninherited)
        } else {
            tcmp
        }
    });
    let mode_beat_length = normalize_timing_beat_length(60000.0 / safe_mode.max(1e-9));
    let states = build_stable_timing_states(&combined);
    let idx = states.partition_point(|state| state.time <= t);
    let state = if idx == 0 { states[0] } else { states[idx - 1] };
    lane_multiplier_for_state(
        state,
        mode_beat_length,
        ScrollLane::Object,
        production_scroll_behavior(),
    )
}
#[derive(Debug, Clone, Copy)]
enum ScrollLane {
    Object,
    Grid,
}
fn production_scroll_behavior() -> StableScrollBehavior {
    StableScrollBehavior::stable_reference_unclamped_red()
}
fn effective_red_beat_length(state: StableTimingState, behavior: StableScrollBehavior) -> f64 {
    if behavior.uses_raw_positive_red_beat_length()
        && state.raw_red_beat_length.is_finite()
        && state.raw_red_beat_length > 0.0
    {
        state.raw_red_beat_length
    } else {
        state.red_beat_length
    }
}
fn lane_multiplier_for_state(
    state: StableTimingState,
    mode_beat_length: f64,
    lane: ScrollLane,
    behavior: StableScrollBehavior,
) -> f64 {
    let safe_mode = normalize_timing_beat_length(mode_beat_length);
    let effective_red = effective_red_beat_length(state, behavior);
    let effective_beat_length = match lane {
        // Object spacing applies inherited green SV; bar/grid spacing follows only red timing.
        ScrollLane::Object => effective_red * state.green_factor,
        ScrollLane::Grid => effective_red,
    };
    let mut multiplier = safe_mode / effective_beat_length.max(1e-9);
    if !multiplier.is_finite() || multiplier < 0.0 {
        multiplier = 1.0;
    }
    multiplier
}
fn build_lane_timeline(
    states: &[StableTimingState],
    mode_beat_length: f64,
    pps_base: f64,
    lane: ScrollLane,
    behavior: StableScrollBehavior,
) -> ScrollSpeedTimeline {
    let points: Vec<ScrollSpeedPoint> = states
        .iter()
        .copied()
        .map(|state| ScrollSpeedPoint {
            time: state.time,
            multiplier: lane_multiplier_for_state(state, mode_beat_length, lane, behavior),
        })
        .collect();
    ScrollSpeedTimeline::new(points, pps_base)
}
pub fn build_stable_scroll_model_with_behavior(
    timing_points: &[TimingPoint],
    last_object_time_ms: Option<i32>,
    pps_base: f64,
    behavior: StableScrollBehavior,
) -> StableScrollModel {
    let states = build_stable_timing_states(timing_points);
    let mode_beat_length = most_common_beat_length(timing_points, last_object_time_ms);
    let object = build_lane_timeline(
        &states,
        mode_beat_length,
        pps_base,
        ScrollLane::Object,
        behavior,
    );
    let grid = build_lane_timeline(
        &states,
        mode_beat_length,
        pps_base,
        ScrollLane::Grid,
        behavior,
    );
    StableScrollModel::new(object, grid)
}
pub fn build_stable_scroll_model(
    timing_points: &[TimingPoint],
    last_object_time_ms: Option<i32>,
    pps_base: f64,
) -> StableScrollModel {
    build_stable_scroll_model_with_behavior(
        timing_points,
        last_object_time_ms,
        pps_base,
        production_scroll_behavior(),
    )
}
pub fn build_scroll_speed_timeline(
    timing_points: &[TimingPoint],
    last_object_time_ms: Option<i32>,
    pps_base: f64,
) -> ScrollSpeedTimeline {
    build_stable_scroll_model(timing_points, last_object_time_ms, pps_base)
        .object_timeline()
        .clone()
}
