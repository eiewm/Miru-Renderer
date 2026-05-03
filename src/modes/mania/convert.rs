use crate::types::{Beatmap, HitObject, HitSample};
use anyhow::{anyhow, Result};
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::Path;
const HIT_SOUND_WHISTLE: u8 = 1 << 1;
const HIT_SOUND_FINISH: u8 = 1 << 2;
const HIT_SOUND_CLAP: u8 = 1 << 3;
const MAX_STAGE_KEYS: i32 = 9;
const MAX_NOTES_FOR_DENSITY: usize = 7;
const PATTERN_FORCE_STACK: u16 = 1;
const PATTERN_FORCE_NOT_STACK: u16 = 1 << 1;
const PATTERN_KEEP_SINGLE: u16 = 1 << 2;
const PATTERN_LOW_PROBABILITY: u16 = 1 << 3;
const PATTERN_GATHERED: u16 = 1 << 7;
const PATTERN_MIRROR: u16 = 1 << 8;
const PATTERN_REVERSE: u16 = 1 << 9;
const PATTERN_CYCLE: u16 = 1 << 10;
const PATTERN_STAIR: u16 = 1 << 11;
const PATTERN_REVERSE_STAIR: u16 = 1 << 12;
#[derive(Debug, Clone, Copy, Default)]
struct SampleFlags {
    whistle: bool,
    finish: bool,
    clap: bool,
}
impl SampleFlags {
    fn from_hit_sound(hit_sound: u8) -> Self {
        Self {
            whistle: hit_sound & HIT_SOUND_WHISTLE != 0,
            finish: hit_sound & HIT_SOUND_FINISH != 0,
            clap: hit_sound & HIT_SOUND_CLAP != 0,
        }
    }
    fn to_hit_sound(self) -> u8 {
        (if self.whistle { HIT_SOUND_WHISTLE } else { 0 })
            | (if self.finish { HIT_SOUND_FINISH } else { 0 })
            | (if self.clap { HIT_SOUND_CLAP } else { 0 })
    }
    fn has_accent(self) -> bool {
        self.whistle || self.finish || self.clap
    }
    fn has_double(self) -> bool {
        self.clap || self.finish
    }
}
#[derive(Debug, Clone)]
enum StandardHitObject {
    Circle {
        x: f32,
        y: f32,
        time: i32,
        samples: SampleFlags,
    },
    Slider {
        x: f32,
        y: f32,
        time: i32,
        span_count: i32,
        distance: f64,
        samples: SampleFlags,
        node_samples: Vec<SampleFlags>,
    },
    Spinner {
        time: i32,
        end_time: i32,
        samples: SampleFlags,
    },
}
#[derive(Debug, Clone, Copy)]
struct LegacyRandom {
    x: u32,
    y: u32,
    z: u32,
    w: u32,
}
impl LegacyRandom {
    fn new(seed: i32) -> Self {
        Self {
            x: seed as u32,
            y: 842_502_087,
            z: 3_579_807_591,
            w: 273_326_509,
        }
    }
    fn next_u32(&mut self) -> u32 {
        let t = self.x ^ (self.x << 11);
        self.x = self.y;
        self.y = self.z;
        self.z = self.w;
        self.w ^= self.w >> 19;
        self.w ^= t;
        self.w ^= t >> 8;
        self.w
    }
    fn next_double(&mut self) -> f64 {
        const INT_TO_REAL: f64 = 1.0 / (i32::MAX as f64 + 1.0);
        ((self.next_u32() & 0x7fff_ffff) as f64) * INT_TO_REAL
    }
    fn next_i32(&mut self, lower: i32, upper: i32) -> i32 {
        if upper <= lower {
            return lower;
        }
        (lower as f64 + self.next_double() * (upper - lower) as f64) as i32
    }
}
#[derive(Debug, Clone, Default)]
struct Pattern {
    hit_objects: Vec<HitObject>,
    occupied_columns: HashSet<i32>,
}
impl Pattern {
    fn add_object(&mut self, object: HitObject) {
        self.occupied_columns.insert(object.column as i32);
        self.hit_objects.push(object);
    }
    fn add_pattern(&mut self, other: &Pattern) {
        for object in &other.hit_objects {
            self.occupied_columns.insert(object.column as i32);
            self.hit_objects.push(object.clone());
        }
    }
    fn clear(&mut self) {
        self.hit_objects.clear();
        self.occupied_columns.clear();
    }
    fn column_has_object(&self, column: i32) -> bool {
        self.occupied_columns.contains(&column)
    }
    fn column_with_objects(&self) -> i32 {
        self.occupied_columns.len() as i32
    }
}
pub fn convert_standard_beatmap_to_mania(
    beatmap: &Beatmap,
    beatmap_path: &Path,
) -> Result<Beatmap> {
    if beatmap.metadata.mode != 0 {
        return Err(anyhow!(
            "unsupported lazer mania source beatmap mode: {}",
            beatmap.metadata.mode
        ));
    }
    let content = fs::read_to_string(beatmap_path)?;
    let source_objects = parse_standard_hit_objects(&content)?;
    if source_objects.is_empty() {
        return Err(anyhow!("standard beatmap has no hit objects to convert"));
    }
    let end_time_object_count = source_objects
        .iter()
        .filter(|object| !matches!(object, StandardHitObject::Circle { .. }))
        .count();
    let target_columns = get_column_count(
        beatmap.metadata.mode,
        beatmap.difficulty.cs,
        beatmap.difficulty.od,
        source_objects.len(),
        end_time_object_count,
    )
    .clamp(1, MAX_STAGE_KEYS) as u8;
    let seed = ((beatmap.difficulty.hp + beatmap.difficulty.cs).round() as i32 * 20)
        + (beatmap.difficulty.od as f64 * 41.2) as i32
        + beatmap.difficulty.ar.round() as i32;
    let mut converter = StandardToManiaConverter::new(beatmap, target_columns, seed);
    let mut hit_objects = converter.convert(&source_objects);
    hit_objects.sort_by_key(|object| {
        (
            object.time,
            object.end_time.unwrap_or(object.time),
            object.column,
            object.obj_type,
        )
    });
    let mut converted = beatmap.clone();
    converted.metadata.mode = 3;
    converted.difficulty.cs = target_columns as f32;
    converted.hit_objects = hit_objects;
    Ok(converted)
}
fn get_column_count(
    source_mode: u8,
    circle_size: f32,
    overall_difficulty: f32,
    total_object_count: usize,
    end_time_object_count: usize,
) -> i32 {
    let rounded_circle_size = circle_size.round() as i32;
    if source_mode == 3 {
        return rounded_circle_size.max(1);
    }
    let rounded_overall_difficulty = overall_difficulty.round() as i32;
    if total_object_count > 0 && end_time_object_count > 0 {
        let percent_special_objects = end_time_object_count as f64 / total_object_count as f64;
        if percent_special_objects < 0.2 {
            return 7;
        }
        if percent_special_objects < 0.3 || rounded_circle_size >= 5 {
            return if rounded_overall_difficulty > 5 { 7 } else { 6 };
        }
        if percent_special_objects > 0.6 {
            return if rounded_overall_difficulty > 4 { 5 } else { 4 };
        }
    }
    (rounded_overall_difficulty + 1).clamp(4, 7)
}
fn parse_standard_hit_objects(content: &str) -> Result<Vec<StandardHitObject>> {
    let mut objects = Vec::new();
    let mut in_hit_objects = false;
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_hit_objects = line == "[HitObjects]";
            continue;
        }
        if !in_hit_objects {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 5 {
            continue;
        }
        let x: f32 = parts[0].trim().parse()?;
        let y: f32 = parts[1].trim().parse()?;
        let time: i32 = parts[2].trim().parse()?;
        let object_type: i32 = parts[3].trim().parse()?;
        let hit_sound: u8 = parts[4].trim().parse().unwrap_or(0);
        let samples = SampleFlags::from_hit_sound(hit_sound);
        if object_type & 1 != 0 {
            objects.push(StandardHitObject::Circle {
                x,
                y,
                time,
                samples,
            });
            continue;
        }
        if object_type & 2 != 0 {
            let span_count = parts
                .get(6)
                .and_then(|value| value.trim().parse::<i32>().ok())
                .unwrap_or(1)
                .max(1);
            let distance = parts
                .get(7)
                .and_then(|value| value.trim().parse::<f64>().ok())
                .unwrap_or(0.0);
            let node_samples = parse_edge_sounds(parts.get(8).copied(), span_count, samples);
            objects.push(StandardHitObject::Slider {
                x,
                y,
                time,
                span_count,
                distance,
                samples,
                node_samples,
            });
            continue;
        }
        if object_type & 8 != 0 {
            let end_time = parts
                .get(5)
                .and_then(|value| value.trim().parse::<i32>().ok())
                .unwrap_or(time);
            objects.push(StandardHitObject::Spinner {
                time,
                end_time,
                samples,
            });
        }
    }
    Ok(objects)
}
fn parse_edge_sounds(
    raw: Option<&str>,
    span_count: i32,
    default_samples: SampleFlags,
) -> Vec<SampleFlags> {
    let mut parsed = raw
        .map(|value| {
            value
                .split('|')
                .map(|entry| {
                    entry
                        .trim()
                        .parse::<u8>()
                        .map(SampleFlags::from_hit_sound)
                        .unwrap_or(default_samples)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let wanted = span_count.saturating_add(1) as usize;
    while parsed.len() < wanted {
        parsed.push(*parsed.last().unwrap_or(&default_samples));
    }
    parsed.truncate(wanted);
    parsed
}
struct StandardToManiaConverter<'a> {
    beatmap: &'a Beatmap,
    total_columns: i32,
    random_start: i32,
    random: LegacyRandom,
    previous_pattern: Pattern,
    previous_note_times: VecDeque<f64>,
    density: f64,
    last_time: f64,
    last_position: (f32, f32),
    last_stair: u16,
}
impl<'a> StandardToManiaConverter<'a> {
    fn new(beatmap: &'a Beatmap, total_columns: u8, seed: i32) -> Self {
        let total_columns = total_columns as i32;
        Self {
            beatmap,
            total_columns,
            random_start: if total_columns == 8 { 1 } else { 0 },
            random: LegacyRandom::new(seed),
            previous_pattern: Pattern::default(),
            previous_note_times: VecDeque::with_capacity(MAX_NOTES_FOR_DENSITY),
            density: i32::MAX as f64,
            last_time: 0.0,
            last_position: (0.0, 0.0),
            last_stair: PATTERN_STAIR,
        }
    }
    fn convert(&mut self, source_objects: &[StandardHitObject]) -> Vec<HitObject> {
        let mut out = Vec::new();
        for object in source_objects {
            match object {
                StandardHitObject::Circle {
                    x,
                    y,
                    time,
                    samples,
                } => {
                    self.compute_density(*time as f64);
                    let (patterns, stair_type) = self.convert_circle(*x, *y, *time, *samples);
                    self.record_note(*time as f64, *x, *y);
                    self.last_stair = stair_type;
                    for pattern in patterns {
                        out.extend(pattern.hit_objects.iter().cloned());
                        self.previous_pattern = pattern;
                    }
                }
                StandardHitObject::Slider {
                    x,
                    y,
                    time,
                    span_count,
                    distance,
                    samples,
                    node_samples,
                } => {
                    let patterns = self.convert_slider(
                        *x,
                        *y,
                        *time,
                        *span_count,
                        *distance,
                        *samples,
                        node_samples,
                    );
                    let segment_duration =
                        self.slider_segment_duration(*time, *span_count, *distance);
                    for index in 0..=*span_count {
                        let row_time = *time + segment_duration * index;
                        self.record_note(row_time as f64, *x, *y);
                        self.compute_density(row_time as f64);
                    }
                    for pattern in patterns {
                        out.extend(pattern.hit_objects.iter().cloned());
                        self.previous_pattern = pattern;
                    }
                }
                StandardHitObject::Spinner {
                    time,
                    end_time,
                    samples,
                } => {
                    let patterns = self.convert_spinner(*time, *end_time, *samples);
                    self.record_note(*end_time as f64, 256.0, 192.0);
                    self.compute_density(*end_time as f64);
                    for pattern in patterns {
                        out.extend(pattern.hit_objects.iter().cloned());
                    }
                }
            }
        }
        out
    }
    fn compute_density(&mut self, new_note_time: f64) {
        if self.previous_note_times.len() == MAX_NOTES_FOR_DENSITY {
            self.previous_note_times.pop_front();
        }
        self.previous_note_times.push_back(new_note_time);
        if self.previous_note_times.len() >= 2 {
            let first = *self.previous_note_times.front().unwrap_or(&new_note_time);
            let last = *self.previous_note_times.back().unwrap_or(&new_note_time);
            self.density = (last - first) / self.previous_note_times.len() as f64;
        }
    }
    fn record_note(&mut self, time: f64, x: f32, y: f32) {
        self.last_time = time;
        self.last_position = (x, y);
    }
    fn conversion_difficulty(&self) -> f64 {
        let first_time = self
            .beatmap
            .hit_objects
            .first()
            .map(|object| object.time)
            .unwrap_or(0);
        let last_time = self
            .beatmap
            .hit_objects
            .last()
            .map(|object| object.time)
            .unwrap_or(first_time);
        let total_break_time: i32 = self
            .beatmap
            .events
            .breaks
            .iter()
            .map(|period| (period.end - period.start).max(0))
            .sum();
        let mut drain_time = ((last_time - first_time - total_break_time) / 1000).max(0);
        if drain_time == 0 {
            drain_time = 10_000;
        }
        let difficulty = (((self.beatmap.difficulty.hp as f64
            + self.beatmap.difficulty.ar.clamp(4.0, 7.0) as f64)
            / 1.5)
            + self.beatmap.hit_objects.len() as f64 / drain_time as f64 * 9.0)
            / 38.0
            * 5.0
            / 1.15;
        difficulty.min(12.0)
    }
    fn timing_beat_length_at(&self, time: i32) -> f64 {
        self.beatmap
            .timing_points
            .iter()
            .filter(|point| point.uninherited && point.time <= time as f64)
            .max_by(|left, right| left.time.partial_cmp(&right.time).unwrap())
            .map(|point| point.beat_length)
            .unwrap_or(500.0)
    }
    fn precision_adjusted_beat_length_at(&self, time: i32) -> f64 {
        let timing_beat_length = self.timing_beat_length_at(time);
        let slider_velocity_beat_length = self
            .beatmap
            .timing_points
            .iter()
            .filter(|point| !point.uninherited && point.time <= time as f64)
            .max_by(|left, right| left.time.partial_cmp(&right.time).unwrap())
            .map(|point| point.beat_length)
            .unwrap_or(-100.0);
        let bpm_multiplier = if slider_velocity_beat_length < 0.0 {
            slider_velocity_beat_length.abs().clamp(10.0, 10_000.0) / 100.0
        } else {
            1.0
        };
        timing_beat_length * bpm_multiplier
    }
    fn is_kiai_at(&self, time: i32) -> bool {
        self.beatmap
            .timing_points
            .iter()
            .filter(|point| point.time <= time as f64)
            .max_by(|left, right| left.time.partial_cmp(&right.time).unwrap())
            .map(|point| point.effects & 1 != 0)
            .unwrap_or(false)
    }
    fn get_column(&self, position: f32, allow_special: bool) -> i32 {
        if allow_special && self.total_columns == 8 {
            let divisor = 512.0 / 7.0;
            return ((position / divisor).floor() as i32).clamp(0, 6) + 1;
        }
        let divisor = 512.0 / self.total_columns as f32;
        ((position / divisor).floor() as i32).clamp(0, self.total_columns - 1)
    }
    fn get_random_column(&mut self, lower_bound: Option<i32>, upper_bound: Option<i32>) -> i32 {
        self.random.next_i32(
            lower_bound.unwrap_or(self.random_start),
            upper_bound.unwrap_or(self.total_columns),
        )
    }
    fn find_available_column<F, V>(
        &mut self,
        mut initial_column: i32,
        lower_bound: Option<i32>,
        upper_bound: Option<i32>,
        mut next_column: F,
        validation: V,
        patterns: &[&Pattern],
    ) -> i32
    where
        F: FnMut(&mut Self, i32) -> i32,
        V: Fn(i32) -> bool,
    {
        let lower_bound = lower_bound.unwrap_or(self.random_start);
        let upper_bound = upper_bound.unwrap_or(self.total_columns);
        let is_valid = |column: i32| {
            validation(column)
                && !patterns
                    .iter()
                    .any(|pattern| pattern.column_has_object(column))
        };
        if is_valid(initial_column) {
            return initial_column;
        }
        let has_valid_columns = (lower_bound..upper_bound).any(is_valid);
        if !has_valid_columns {
            return initial_column.clamp(lower_bound, upper_bound.saturating_sub(1));
        }
        loop {
            initial_column = next_column(self, initial_column);
            if is_valid(initial_column) {
                return initial_column;
            }
        }
    }
    fn make_note(&self, column: i32, time: i32, samples: SampleFlags) -> HitObject {
        let clamped_column = column.clamp(0, self.total_columns - 1) as u8;
        HitObject {
            x: (((clamped_column as f32 + 0.5) / self.total_columns as f32) * 512.0).round() as i32,
            y: 192,
            time,
            obj_type: 1,
            hit_sound: samples.to_hit_sound(),
            end_time: None,
            column: clamped_column,
            hit_sample: HitSample::default(),
        }
    }
    fn make_hold(
        &self,
        column: i32,
        start_time: i32,
        end_time: i32,
        samples: SampleFlags,
    ) -> HitObject {
        let clamped_column = column.clamp(0, self.total_columns - 1) as u8;
        HitObject {
            x: (((clamped_column as f32 + 0.5) / self.total_columns as f32) * 512.0).round() as i32,
            y: 192,
            time: start_time,
            obj_type: 128,
            hit_sound: samples.to_hit_sound(),
            end_time: Some(end_time.max(start_time)),
            column: clamped_column,
            hit_sample: HitSample::default(),
        }
    }
    fn raw_random_note_count(&mut self, p2: f64, p3: f64, p4: f64, p5: f64, p6: f64) -> i32 {
        let value = self.random.next_double();
        if value >= 1.0 - p6 {
            return 6;
        }
        if value >= 1.0 - p5 {
            return 5;
        }
        if value >= 1.0 - p4 {
            return 4;
        }
        if value >= 1.0 - p3 {
            return 3;
        }
        if value >= 1.0 - p2 {
            2
        } else {
            1
        }
    }
    fn convert_circle(
        &mut self,
        x: f32,
        y: f32,
        time: i32,
        samples: SampleFlags,
    ) -> (Vec<Pattern>, u16) {
        let beat_length = self.timing_beat_length_at(time);
        let position_separation =
            ((x - self.last_position.0).powi(2) + (y - self.last_position.1).powi(2)).sqrt();
        let time_separation = time as f64 - self.last_time;
        let mut convert_type = 0;
        if time_separation <= 80.0 {
            convert_type |= PATTERN_FORCE_NOT_STACK | PATTERN_KEEP_SINGLE;
        } else if time_separation <= 95.0 {
            convert_type |= PATTERN_FORCE_NOT_STACK | PATTERN_KEEP_SINGLE | self.last_stair;
        } else if time_separation <= 105.0 {
            convert_type |= PATTERN_FORCE_NOT_STACK | PATTERN_LOW_PROBABILITY;
        } else if time_separation <= 125.0 {
            convert_type |= PATTERN_FORCE_NOT_STACK;
        } else if time_separation <= 135.0 && position_separation < 20.0 {
            convert_type |= PATTERN_CYCLE | PATTERN_KEEP_SINGLE;
        } else if time_separation <= 150.0 && position_separation < 20.0 {
            convert_type |= PATTERN_FORCE_STACK | PATTERN_LOW_PROBABILITY;
        } else if position_separation < 20.0 && self.density >= beat_length / 2.5 {
            convert_type |= PATTERN_REVERSE | PATTERN_LOW_PROBABILITY;
        } else if self.density >= beat_length / 2.5 && !self.is_kiai_at(time) {
            convert_type |= PATTERN_LOW_PROBABILITY;
        }
        if convert_type & PATTERN_KEEP_SINGLE == 0 {
            if samples.finish && self.total_columns != 8 {
                convert_type |= PATTERN_MIRROR;
            } else if samples.clap {
                convert_type |= PATTERN_GATHERED;
            }
        }
        let mut stair_type = self.last_stair;
        let pattern = self.generate_circle_pattern(x, time, samples, convert_type, &mut stair_type);
        (vec![pattern], stair_type)
    }
    fn generate_circle_pattern(
        &mut self,
        x: f32,
        time: i32,
        samples: SampleFlags,
        convert_type: u16,
        stair_type: &mut u16,
    ) -> Pattern {
        let pattern = if self.total_columns == 1 {
            let mut pattern = Pattern::default();
            pattern.add_object(self.make_note(0, time, samples));
            pattern
        } else {
            let last_column = self
                .previous_pattern
                .hit_objects
                .first()
                .map(|object| object.column as i32)
                .unwrap_or(0);
            if convert_type & PATTERN_REVERSE != 0 && !self.previous_pattern.hit_objects.is_empty()
            {
                let mut pattern = Pattern::default();
                for column in self.random_start..self.total_columns {
                    if self.previous_pattern.column_has_object(column) {
                        pattern.add_object(self.make_note(
                            self.random_start + self.total_columns - column - 1,
                            time,
                            samples,
                        ));
                    }
                }
                pattern
            } else if convert_type & PATTERN_CYCLE != 0
                && self.previous_pattern.hit_objects.len() == 1
                && (self.total_columns != 8 || last_column != 0)
                && (self.total_columns % 2 == 0 || last_column != self.total_columns / 2)
            {
                let mut pattern = Pattern::default();
                let column = self.random_start + self.total_columns - last_column - 1;
                pattern.add_object(self.make_note(column, time, samples));
                pattern
            } else if convert_type & PATTERN_FORCE_STACK != 0
                && !self.previous_pattern.hit_objects.is_empty()
            {
                let mut pattern = Pattern::default();
                for column in self.random_start..self.total_columns {
                    if self.previous_pattern.column_has_object(column) {
                        pattern.add_object(self.make_note(column, time, samples));
                    }
                }
                pattern
            } else if self.previous_pattern.hit_objects.len() == 1
                && convert_type & PATTERN_STAIR != 0
            {
                let mut pattern = Pattern::default();
                let mut target_column = last_column + 1;
                if target_column == self.total_columns {
                    target_column = self.random_start;
                }
                pattern.add_object(self.make_note(target_column, time, samples));
                pattern
            } else if self.previous_pattern.hit_objects.len() == 1
                && convert_type & PATTERN_REVERSE_STAIR != 0
            {
                let mut pattern = Pattern::default();
                let mut target_column = last_column - 1;
                if target_column == self.random_start - 1 {
                    target_column = self.total_columns - 1;
                }
                pattern.add_object(self.make_note(target_column, time, samples));
                pattern
            } else if convert_type & PATTERN_KEEP_SINGLE != 0 {
                self.circle_generate_random_notes(x, time, samples, convert_type, 1)
            } else if convert_type & PATTERN_MIRROR != 0 {
                let difficulty = self.conversion_difficulty();
                if difficulty > 6.5 {
                    self.circle_generate_random_pattern_with_mirrored(
                        time,
                        samples,
                        convert_type,
                        0.12,
                        0.38,
                        0.12,
                    )
                } else if difficulty > 4.0 {
                    self.circle_generate_random_pattern_with_mirrored(
                        time,
                        samples,
                        convert_type,
                        0.12,
                        0.17,
                        0.0,
                    )
                } else {
                    self.circle_generate_random_pattern_with_mirrored(
                        time,
                        samples,
                        convert_type,
                        0.12,
                        0.0,
                        0.0,
                    )
                }
            } else {
                let difficulty = self.conversion_difficulty();
                if difficulty > 6.5 {
                    if convert_type & PATTERN_LOW_PROBABILITY != 0 {
                        self.circle_generate_random_pattern(
                            time,
                            samples,
                            convert_type,
                            x,
                            0.78,
                            0.42,
                            0.0,
                            0.0,
                        )
                    } else {
                        self.circle_generate_random_pattern(
                            time,
                            samples,
                            convert_type,
                            x,
                            1.0,
                            0.62,
                            0.0,
                            0.0,
                        )
                    }
                } else if difficulty > 4.0 {
                    if convert_type & PATTERN_LOW_PROBABILITY != 0 {
                        self.circle_generate_random_pattern(
                            time,
                            samples,
                            convert_type,
                            x,
                            0.35,
                            0.08,
                            0.0,
                            0.0,
                        )
                    } else {
                        self.circle_generate_random_pattern(
                            time,
                            samples,
                            convert_type,
                            x,
                            0.52,
                            0.15,
                            0.0,
                            0.0,
                        )
                    }
                } else if difficulty > 2.0 {
                    if convert_type & PATTERN_LOW_PROBABILITY != 0 {
                        self.circle_generate_random_pattern(
                            time,
                            samples,
                            convert_type,
                            x,
                            0.18,
                            0.0,
                            0.0,
                            0.0,
                        )
                    } else {
                        self.circle_generate_random_pattern(
                            time,
                            samples,
                            convert_type,
                            x,
                            0.45,
                            0.0,
                            0.0,
                            0.0,
                        )
                    }
                } else {
                    self.circle_generate_random_pattern(
                        time,
                        samples,
                        convert_type,
                        x,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                    )
                }
            }
        };
        for object in &pattern.hit_objects {
            if convert_type & PATTERN_STAIR != 0 && object.column as i32 == self.total_columns - 1 {
                *stair_type = PATTERN_REVERSE_STAIR;
            }
            if convert_type & PATTERN_REVERSE_STAIR != 0
                && object.column as i32 == self.random_start
            {
                *stair_type = PATTERN_STAIR;
            }
        }
        pattern
    }
    fn circle_generate_random_notes(
        &mut self,
        x: f32,
        time: i32,
        samples: SampleFlags,
        convert_type: u16,
        note_count: i32,
    ) -> Pattern {
        let mut pattern = Pattern::default();
        let allow_stacking = convert_type & PATTERN_FORCE_NOT_STACK == 0;
        let max_notes = if allow_stacking {
            note_count
        } else {
            note_count.min(
                self.total_columns
                    - self.random_start
                    - self.previous_pattern.column_with_objects(),
            )
        };
        let mut next_column = self.get_column(x, true);
        for _ in 0..max_notes.max(0) {
            next_column = if allow_stacking {
                self.find_available_column(
                    next_column,
                    None,
                    None,
                    |this, last| {
                        if convert_type & PATTERN_GATHERED != 0 {
                            let mut column = last + 1;
                            if column == this.total_columns {
                                column = this.random_start;
                            }
                            column
                        } else {
                            this.get_random_column(None, None)
                        }
                    },
                    |_| true,
                    &[&pattern],
                )
            } else {
                let previous_pattern = self.previous_pattern.clone();
                self.find_available_column(
                    next_column,
                    None,
                    None,
                    |this, last| {
                        if convert_type & PATTERN_GATHERED != 0 {
                            let mut column = last + 1;
                            if column == this.total_columns {
                                column = this.random_start;
                            }
                            column
                        } else {
                            this.get_random_column(None, None)
                        }
                    },
                    |_| true,
                    &[&pattern, &previous_pattern],
                )
            };
            pattern.add_object(self.make_note(next_column, time, samples));
        }
        pattern
    }
    fn circle_has_special_column(&self, samples: SampleFlags) -> bool {
        samples.clap && samples.finish
    }
    fn circle_generate_random_pattern(
        &mut self,
        time: i32,
        samples: SampleFlags,
        convert_type: u16,
        x: f32,
        p2: f64,
        p3: f64,
        p4: f64,
        p5: f64,
    ) -> Pattern {
        let note_count = self.circle_get_random_note_count(samples, p2, p3, p4, p5);
        let mut pattern =
            self.circle_generate_random_notes(x, time, samples, convert_type, note_count);
        if self.random_start > 0 && self.circle_has_special_column(samples) {
            pattern.add_object(self.make_note(0, time, samples));
        }
        pattern
    }
    fn circle_generate_random_pattern_with_mirrored(
        &mut self,
        time: i32,
        samples: SampleFlags,
        convert_type: u16,
        centre_probability: f64,
        p2: f64,
        p3: f64,
    ) -> Pattern {
        if convert_type & PATTERN_FORCE_NOT_STACK != 0 {
            return self.circle_generate_random_pattern(
                time,
                samples,
                convert_type,
                256.0,
                0.5 + p2 / 2.0,
                p2,
                (p2 + p3) / 2.0,
                p3,
            );
        }
        let mut pattern = Pattern::default();
        let (note_count, add_to_centre) =
            self.circle_get_random_note_count_mirrored(centre_probability, p2, p3);
        let column_limit = if self.total_columns % 2 == 0 {
            self.total_columns
        } else {
            self.total_columns - 1
        } / 2;
        let mut next_column = self.get_random_column(None, Some(column_limit));
        for _ in 0..note_count {
            next_column = self.find_available_column(
                next_column,
                None,
                Some(column_limit),
                |this, _| this.get_random_column(None, Some(column_limit)),
                |_| true,
                &[&pattern],
            );
            pattern.add_object(self.make_note(next_column, time, samples));
            pattern.add_object(self.make_note(
                self.random_start + self.total_columns - next_column - 1,
                time,
                samples,
            ));
        }
        if add_to_centre {
            pattern.add_object(self.make_note(self.total_columns / 2, time, samples));
        }
        if self.random_start > 0 && self.circle_has_special_column(samples) {
            pattern.add_object(self.make_note(0, time, samples));
        }
        pattern
    }
    fn circle_get_random_note_count(
        &mut self,
        samples: SampleFlags,
        mut p2: f64,
        mut p3: f64,
        mut p4: f64,
        mut p5: f64,
    ) -> i32 {
        match self.total_columns {
            2 => {
                p2 = 0.0;
                p3 = 0.0;
                p4 = 0.0;
                p5 = 0.0;
            }
            3 => {
                p2 = p2.min(0.10);
                p3 = 0.0;
                p4 = 0.0;
                p5 = 0.0;
            }
            4 => {
                p2 = p2.min(0.23);
                p3 = p3.min(0.04);
                p4 = 0.0;
                p5 = 0.0;
            }
            5 => {
                p3 = p3.min(0.15);
                p4 = p4.min(0.03);
                p5 = 0.0;
            }
            _ => {}
        }
        if samples.clap {
            p2 = 1.0;
        }
        self.raw_random_note_count(p2, p3, p4, p5, 0.0)
    }
    fn circle_get_random_note_count_mirrored(
        &mut self,
        mut centre_probability: f64,
        mut p2: f64,
        mut p3: f64,
    ) -> (i32, bool) {
        match self.total_columns {
            2 => {
                centre_probability = 0.0;
                p2 = 0.0;
                p3 = 0.0;
            }
            3 => {
                centre_probability = centre_probability.min(0.03);
                p2 = 0.0;
                p3 = 0.0;
            }
            4 => {
                centre_probability = 0.0;
                p2 = 1.0 - ((1.0 - p2) * 2.0).max(0.8);
                p3 = 0.0;
            }
            5 => {
                centre_probability = centre_probability.min(0.03);
                p3 = 0.0;
            }
            6 => {
                centre_probability = 0.0;
                p2 = 1.0 - ((1.0 - p2) * 2.0).max(0.5);
                p3 = 1.0 - ((1.0 - p3) * 2.0).max(0.85);
            }
            _ => {}
        }
        p2 = p2.clamp(0.0, 1.0);
        p3 = p3.clamp(0.0, 1.0);
        let centre_value = self.random.next_double();
        let note_count = self.raw_random_note_count(p2, p3, 0.0, 0.0, 0.0);
        let add_to_centre = self.total_columns % 2 != 0
            && note_count != 3
            && centre_value > 1.0 - centre_probability;
        (note_count, add_to_centre)
    }
    fn slider_segment_duration(&self, time: i32, span_count: i32, distance: f64) -> i32 {
        let span_count = span_count.max(1);
        let beat_length = self.precision_adjusted_beat_length_at(time);
        let slider_multiplier = self.beatmap.difficulty.slider_multiplier.max(0.01) as f64;
        let end_time = (time as f64
            + distance * beat_length * span_count as f64 * 0.01 / slider_multiplier)
            .floor() as i32;
        (end_time - time) / span_count
    }
    fn convert_slider(
        &mut self,
        x: f32,
        _y: f32,
        time: i32,
        span_count: i32,
        distance: f64,
        samples: SampleFlags,
        node_samples: &[SampleFlags],
    ) -> Vec<Pattern> {
        let mut convert_type = 0;
        if !self.is_kiai_at(time) {
            convert_type |= PATTERN_LOW_PROBABILITY;
        }
        let beat_length = self.precision_adjusted_beat_length_at(time);
        let slider_multiplier = self.beatmap.difficulty.slider_multiplier.max(0.01) as f64;
        let span_count = span_count.max(1);
        let start_time = time;
        let end_time = (start_time as f64
            + distance * beat_length * span_count as f64 * 0.01 / slider_multiplier)
            .floor() as i32;
        let segment_duration = (end_time - start_time) / span_count;
        let original_pattern = self.generate_slider_pattern(
            x,
            start_time,
            end_time,
            segment_duration,
            span_count,
            samples,
            node_samples,
            convert_type,
        );
        if original_pattern.hit_objects.len() == 1 {
            return vec![original_pattern];
        }
        let mut intermediate_pattern = Pattern::default();
        let mut end_time_pattern = Pattern::default();
        for object in original_pattern.hit_objects {
            if object.end_time.unwrap_or(object.time) == end_time {
                end_time_pattern.add_object(object);
            } else {
                intermediate_pattern.add_object(object);
            }
        }
        vec![intermediate_pattern, end_time_pattern]
    }
    fn generate_slider_pattern(
        &mut self,
        x: f32,
        start_time: i32,
        end_time: i32,
        segment_duration: i32,
        span_count: i32,
        samples: SampleFlags,
        node_samples: &[SampleFlags],
        mut convert_type: u16,
    ) -> Pattern {
        if self.total_columns == 1 {
            let mut pattern = Pattern::default();
            pattern.add_object(self.make_hold(0, start_time, end_time, samples));
            return pattern;
        }
        if span_count > 1 {
            if segment_duration <= 90 {
                return self.slider_generate_random_hold_notes(start_time, end_time, 1, samples);
            }
            if segment_duration <= 120 {
                convert_type |= PATTERN_FORCE_NOT_STACK;
                return self.slider_generate_random_notes(
                    x,
                    start_time,
                    segment_duration,
                    span_count + 1,
                    samples,
                    node_samples,
                    convert_type,
                );
            }
            if segment_duration <= 160 {
                return self.slider_generate_stair(
                    x,
                    start_time,
                    segment_duration,
                    span_count,
                    samples,
                    node_samples,
                );
            }
            if segment_duration <= 200 && self.conversion_difficulty() > 3.0 {
                return self.slider_generate_random_multiple_notes(
                    x,
                    start_time,
                    segment_duration,
                    span_count,
                    samples,
                    node_samples,
                );
            }
            if end_time - start_time >= 4000 {
                return self.slider_generate_n_random_notes(
                    start_time,
                    end_time,
                    0.23,
                    0.0,
                    0.0,
                    samples,
                    node_samples,
                    convert_type,
                );
            }
            if segment_duration > 400 && span_count < self.total_columns - 1 - self.random_start {
                return self.slider_generate_tiled_hold_notes(
                    x,
                    start_time,
                    segment_duration,
                    span_count,
                    samples,
                    convert_type,
                );
            }
            return self.slider_generate_hold_and_normal_notes(
                x,
                start_time,
                end_time,
                segment_duration,
                span_count,
                samples,
                node_samples,
                convert_type,
            );
        }
        if segment_duration <= 110 {
            if self.previous_pattern.column_with_objects() < self.total_columns {
                convert_type |= PATTERN_FORCE_NOT_STACK;
            } else {
                convert_type &= !PATTERN_FORCE_NOT_STACK;
            }
            return self.slider_generate_random_notes(
                x,
                start_time,
                segment_duration,
                if segment_duration < 80 { 1 } else { 2 },
                samples,
                node_samples,
                convert_type,
            );
        }
        let difficulty = self.conversion_difficulty();
        if difficulty > 6.5 {
            if convert_type & PATTERN_LOW_PROBABILITY != 0 {
                return self.slider_generate_n_random_notes(
                    start_time,
                    end_time,
                    0.78,
                    0.3,
                    0.0,
                    samples,
                    node_samples,
                    convert_type,
                );
            }
            return self.slider_generate_n_random_notes(
                start_time,
                end_time,
                0.85,
                0.36,
                0.03,
                samples,
                node_samples,
                convert_type,
            );
        }
        if difficulty > 4.0 {
            if convert_type & PATTERN_LOW_PROBABILITY != 0 {
                return self.slider_generate_n_random_notes(
                    start_time,
                    end_time,
                    0.43,
                    0.08,
                    0.0,
                    samples,
                    node_samples,
                    convert_type,
                );
            }
            return self.slider_generate_n_random_notes(
                start_time,
                end_time,
                0.56,
                0.18,
                0.0,
                samples,
                node_samples,
                convert_type,
            );
        }
        if difficulty > 2.5 {
            if convert_type & PATTERN_LOW_PROBABILITY != 0 {
                return self.slider_generate_n_random_notes(
                    start_time,
                    end_time,
                    0.3,
                    0.0,
                    0.0,
                    samples,
                    node_samples,
                    convert_type,
                );
            }
            return self.slider_generate_n_random_notes(
                start_time,
                end_time,
                0.37,
                0.08,
                0.0,
                samples,
                node_samples,
                convert_type,
            );
        }
        if convert_type & PATTERN_LOW_PROBABILITY != 0 {
            return self.slider_generate_n_random_notes(
                start_time,
                end_time,
                0.17,
                0.0,
                0.0,
                samples,
                node_samples,
                convert_type,
            );
        }
        self.slider_generate_n_random_notes(
            start_time,
            end_time,
            0.27,
            0.0,
            0.0,
            samples,
            node_samples,
            convert_type,
        )
    }
    fn slider_sample_at(
        &self,
        time: i32,
        start_time: i32,
        segment_duration: i32,
        default_samples: SampleFlags,
        node_samples: &[SampleFlags],
    ) -> SampleFlags {
        if node_samples.is_empty() {
            return default_samples;
        }
        let index = if segment_duration == 0 {
            0
        } else {
            ((time - start_time) / segment_duration).max(0) as usize
        };
        node_samples.get(index).copied().unwrap_or(default_samples)
    }
    fn slider_generate_random_hold_notes(
        &mut self,
        start_time: i32,
        end_time: i32,
        note_count: i32,
        samples: SampleFlags,
    ) -> Pattern {
        let mut pattern = Pattern::default();
        let usable_columns =
            self.total_columns - self.random_start - self.previous_pattern.column_with_objects();
        let mut next_column = self.get_random_column(None, None);
        let first_batch = note_count.min(usable_columns).max(0);
        let previous_pattern = self.previous_pattern.clone();
        for _ in 0..first_batch {
            next_column = self.find_available_column(
                next_column,
                None,
                None,
                |this, _| this.get_random_column(None, None),
                |_| true,
                &[&pattern, &previous_pattern],
            );
            pattern.add_object(self.make_hold(next_column, start_time, end_time, samples));
        }
        for _ in 0..(note_count - first_batch).max(0) {
            next_column = self.find_available_column(
                next_column,
                None,
                None,
                |this, _| this.get_random_column(None, None),
                |_| true,
                &[&pattern],
            );
            pattern.add_object(self.make_hold(next_column, start_time, end_time, samples));
        }
        pattern
    }
    fn slider_generate_random_notes(
        &mut self,
        x: f32,
        mut start_time: i32,
        segment_duration: i32,
        note_count: i32,
        samples: SampleFlags,
        node_samples: &[SampleFlags],
        convert_type: u16,
    ) -> Pattern {
        let mut pattern = Pattern::default();
        let mut next_column = self.get_column(x, true);
        let anchor_time = start_time;
        if convert_type & PATTERN_FORCE_NOT_STACK != 0
            && self.previous_pattern.column_with_objects() < self.total_columns
        {
            let previous_pattern = self.previous_pattern.clone();
            next_column = self.find_available_column(
                next_column,
                None,
                None,
                |this, _| this.get_random_column(None, None),
                |_| true,
                &[&previous_pattern],
            );
        }
        let mut last_column = next_column;
        for _ in 0..note_count.max(0) {
            let row_samples = self.slider_sample_at(
                start_time,
                anchor_time,
                segment_duration,
                samples,
                node_samples,
            );
            pattern.add_object(self.make_note(next_column, start_time, row_samples));
            next_column = self.find_available_column(
                next_column,
                None,
                None,
                |this, _| this.get_random_column(None, None),
                |column| column != last_column,
                &[],
            );
            last_column = next_column;
            start_time += segment_duration;
        }
        pattern
    }
    fn slider_generate_stair(
        &mut self,
        x: f32,
        mut start_time: i32,
        segment_duration: i32,
        span_count: i32,
        samples: SampleFlags,
        node_samples: &[SampleFlags],
    ) -> Pattern {
        let mut pattern = Pattern::default();
        let mut column = self.get_column(x, true);
        let mut increasing = self.random.next_double() > 0.5;
        let anchor_time = start_time;
        for _ in 0..=span_count.max(0) {
            let row_samples = self.slider_sample_at(
                start_time,
                anchor_time,
                segment_duration,
                samples,
                node_samples,
            );
            pattern.add_object(self.make_note(column, start_time, row_samples));
            start_time += segment_duration;
            if increasing {
                if column >= self.total_columns - 1 {
                    increasing = false;
                    column -= 1;
                } else {
                    column += 1;
                }
            } else if column <= self.random_start {
                increasing = true;
                column += 1;
            } else {
                column -= 1;
            }
        }
        pattern
    }
    fn slider_generate_random_multiple_notes(
        &mut self,
        x: f32,
        mut start_time: i32,
        segment_duration: i32,
        span_count: i32,
        samples: SampleFlags,
        node_samples: &[SampleFlags],
    ) -> Pattern {
        let mut pattern = Pattern::default();
        let legacy = (4..=8).contains(&self.total_columns);
        let interval = self.get_random_column(
            Some(1),
            Some(self.total_columns - if legacy { 1 } else { 0 }),
        );
        let mut next_column = self.get_column(x, true);
        let anchor_time = start_time;
        for _ in 0..=span_count.max(0) {
            let row_samples = self.slider_sample_at(
                start_time,
                anchor_time,
                segment_duration,
                samples,
                node_samples,
            );
            pattern.add_object(self.make_note(next_column, start_time, row_samples));
            next_column += interval;
            if next_column >= self.total_columns - self.random_start {
                next_column = next_column - self.total_columns - self.random_start
                    + if legacy { 1 } else { 0 };
            }
            next_column += self.random_start;
            if self.total_columns > 2 {
                pattern.add_object(self.make_note(next_column, start_time, row_samples));
            }
            next_column = self.get_random_column(None, None);
            start_time += segment_duration;
        }
        pattern
    }
    fn slider_generate_n_random_notes(
        &mut self,
        start_time: i32,
        end_time: i32,
        mut p2: f64,
        mut p3: f64,
        mut p4: f64,
        samples: SampleFlags,
        node_samples: &[SampleFlags],
        convert_type: u16,
    ) -> Pattern {
        match self.total_columns {
            2 => {
                p2 = 0.0;
                p3 = 0.0;
                p4 = 0.0;
            }
            3 => {
                p2 = p2.min(0.10);
                p3 = 0.0;
                p4 = 0.0;
            }
            4 => {
                p2 = p2.min(0.30);
                p3 = p3.min(0.04);
                p4 = 0.0;
            }
            5 => {
                p2 = p2.min(0.34);
                p3 = p3.min(0.10);
                p4 = p4.min(0.03);
            }
            _ => {}
        }
        let can_generate_two_notes = convert_type & PATTERN_LOW_PROBABILITY == 0
            && (samples.has_double()
                || self
                    .slider_sample_at(start_time, start_time, 1, samples, node_samples)
                    .has_double());
        if can_generate_two_notes {
            p2 = 1.0;
        }
        let note_count = self.raw_random_note_count(p2, p3, p4, 0.0, 0.0);
        self.slider_generate_random_hold_notes(start_time, end_time, note_count, samples)
    }
    fn slider_generate_tiled_hold_notes(
        &mut self,
        x: f32,
        mut start_time: i32,
        segment_duration: i32,
        span_count: i32,
        samples: SampleFlags,
        convert_type: u16,
    ) -> Pattern {
        let mut pattern = Pattern::default();
        let column_repeat = span_count.min(self.total_columns).max(0);
        let end_time = start_time + segment_duration * span_count.max(0);
        let mut next_column = self.get_column(x, true);
        if convert_type & PATTERN_FORCE_NOT_STACK != 0
            && self.previous_pattern.column_with_objects() < self.total_columns
        {
            let previous_pattern = self.previous_pattern.clone();
            next_column = self.find_available_column(
                next_column,
                None,
                None,
                |this, _| this.get_random_column(None, None),
                |_| true,
                &[&previous_pattern],
            );
        }
        for _ in 0..column_repeat {
            next_column = self.find_available_column(
                next_column,
                None,
                None,
                |this, _| this.get_random_column(None, None),
                |_| true,
                &[&pattern],
            );
            pattern.add_object(self.make_hold(next_column, start_time, end_time, samples));
            start_time += segment_duration;
        }
        pattern
    }
    fn slider_generate_hold_and_normal_notes(
        &mut self,
        x: f32,
        mut start_time: i32,
        end_time: i32,
        segment_duration: i32,
        span_count: i32,
        samples: SampleFlags,
        node_samples: &[SampleFlags],
        convert_type: u16,
    ) -> Pattern {
        let mut pattern = Pattern::default();
        let anchor_time = start_time;
        let mut hold_column = self.get_column(x, true);
        if convert_type & PATTERN_FORCE_NOT_STACK != 0
            && self.previous_pattern.column_with_objects() < self.total_columns
        {
            let previous_pattern = self.previous_pattern.clone();
            hold_column = self.find_available_column(
                hold_column,
                None,
                None,
                |this, _| this.get_random_column(None, None),
                |_| true,
                &[&previous_pattern],
            );
        }
        pattern.add_object(self.make_hold(hold_column, start_time, end_time, samples));
        let mut next_column = self.get_random_column(None, None);
        let difficulty = self.conversion_difficulty();
        let mut note_count = if difficulty > 6.5 {
            self.raw_random_note_count(0.63, 0.0, 0.0, 0.0, 0.0)
        } else if difficulty > 4.0 {
            self.raw_random_note_count(
                if self.total_columns < 6 { 0.12 } else { 0.45 },
                0.0,
                0.0,
                0.0,
                0.0,
            )
        } else if difficulty > 2.5 {
            self.raw_random_note_count(
                if self.total_columns < 6 { 0.0 } else { 0.24 },
                0.0,
                0.0,
                0.0,
                0.0,
            )
        } else {
            1
        } - 1;
        note_count = note_count.clamp(0, self.total_columns - 1);
        let ignore_head = !self
            .slider_sample_at(
                start_time,
                anchor_time,
                segment_duration,
                samples,
                node_samples,
            )
            .has_accent();
        let mut row_pattern = Pattern::default();
        for _ in 0..=span_count.max(0) {
            if !(ignore_head && start_time == anchor_time) {
                for _ in 0..note_count {
                    next_column = self.find_available_column(
                        next_column,
                        None,
                        None,
                        |this, _| this.get_random_column(None, None),
                        |column| column != hold_column,
                        &[&row_pattern],
                    );
                    let row_samples = self.slider_sample_at(
                        start_time,
                        anchor_time,
                        segment_duration,
                        samples,
                        node_samples,
                    );
                    row_pattern.add_object(self.make_note(next_column, start_time, row_samples));
                }
            }
            pattern.add_pattern(&row_pattern);
            row_pattern.clear();
            start_time += segment_duration;
        }
        pattern
    }
    fn convert_spinner(&mut self, time: i32, end_time: i32, samples: SampleFlags) -> Vec<Pattern> {
        let mut pattern = Pattern::default();
        let force_not_stack = self.previous_pattern.column_with_objects() != self.total_columns;
        let generate_hold = end_time - time >= 100;
        let column = if self.total_columns == 8 && samples.finish && end_time - time < 1000 {
            0
        } else if self.total_columns == 8 {
            self.spinner_random_column(force_not_stack, None)
        } else {
            self.spinner_random_column(force_not_stack, Some(0))
        };
        if generate_hold {
            pattern.add_object(self.make_hold(column, time, end_time, samples));
        } else {
            pattern.add_object(self.make_note(column, time, samples));
        }
        vec![pattern]
    }
    fn spinner_random_column(&mut self, force_not_stack: bool, lower_bound: Option<i32>) -> i32 {
        if force_not_stack {
            let initial_column = self.get_random_column(lower_bound, None);
            let previous_pattern = self.previous_pattern.clone();
            self.find_available_column(
                initial_column,
                lower_bound,
                None,
                |this, _| this.get_random_column(lower_bound, None),
                |_| true,
                &[&previous_pattern],
            )
        } else {
            let initial_column = self.get_random_column(lower_bound, None);
            self.find_available_column(
                initial_column,
                lower_bound,
                None,
                |this, _| this.get_random_column(lower_bound, None),
                |_| true,
                &[],
            )
        }
    }
}
