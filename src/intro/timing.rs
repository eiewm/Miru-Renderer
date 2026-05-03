use crate::types::{Beatmap, HitObject, TimingPoint};
pub fn get_bpm_at_time(timing_points: &[TimingPoint], time_ms: f64) -> f32 {
    let mut best: Option<&TimingPoint> = None;
    for tp in timing_points {
        // Inherited timing points affect slider velocity, not the BPM used for the intro pulse.
        if !tp.uninherited {
            continue;
        }
        if tp.beat_length < 10.0 || tp.beat_length > 100000.0 {
            continue;
        }
        if tp.time <= time_ms {
            match best {
                Some(prev) if tp.time > prev.time => best = Some(tp),
                None => best = Some(tp),
                _ => {}
            }
        }
    }
    if let Some(tp) = best {
        let bpm = 60000.0 / tp.beat_length;
        // Clamp outlier timing data so the logo pulse stays visually usable.
        return bpm.clamp(60.0, 300.0) as f32;
    }
    for tp in timing_points {
        if tp.uninherited && tp.beat_length >= 10.0 && tp.beat_length < 100000.0 {
            let bpm = 60000.0 / tp.beat_length;
            return bpm.clamp(60.0, 300.0) as f32;
        }
    }
    120.0
}
pub fn find_preview_point(
    beatmap_preview: i32,
    hit_objects: &[HitObject],
    audio_duration_ms: Option<i32>,
) -> i32 {
    if beatmap_preview > 0 {
        return beatmap_preview;
    }
    if !hit_objects.is_empty() {
        const WINDOW_MS: i32 = 5000;
        let mut max_density = 0;
        let mut best_time = hit_objects[0].time;
        // Without a beatmap preview point, choose the densest nearby section instead of the first note.
        for (i, obj) in hit_objects.iter().enumerate() {
            let window_end = obj.time + WINDOW_MS;
            let count = hit_objects[i..]
                .iter()
                .take_while(|o| o.time < window_end)
                .count();
            if count > max_density {
                max_density = count;
                best_time = obj.time + WINDOW_MS / 2;
            }
        }
        return best_time;
    }
    if let Some(dur) = audio_duration_ms {
        if dur > 0 {
            return (dur as f32 * 0.3) as i32;
        }
    }
    30000
}
pub fn get_preview_bpm(beatmap: &Beatmap) -> f32 {
    let preview = if beatmap.metadata.preview_time > 0 {
        beatmap.metadata.preview_time
    } else if !beatmap.hit_objects.is_empty() {
        beatmap.hit_objects[0].time
    } else {
        0
    };
    get_bpm_at_time(&beatmap.timing_points, preview as f64)
}
pub const INTRO_DURATION_MS: u32 = 3500;
#[inline]
pub fn beat_interval_ms(bpm: f32) -> f32 {
    60000.0 / bpm.max(1.0)
}
#[inline]
pub fn pulse_intensity(time_ms: f32, bpm: f32) -> f32 {
    let interval = beat_interval_ms(bpm);
    let phase = (time_ms % interval) / interval;
    // The pulse snaps on the beat and decays exponentially before the next beat.
    (-phase * 4.0).exp()
}
#[inline]
pub fn pulse_scale_from_intensity(intensity: f32) -> f32 {
    1.0 + 0.015 * intensity
}
#[inline]
pub fn pulse_scale(time_ms: f32, bpm: f32) -> f32 {
    pulse_scale_from_intensity(pulse_intensity(time_ms, bpm))
}
#[inline]
pub fn fade_opacity(time_ms: f32, duration_ms: u32) -> f32 {
    let fade_start = duration_ms as f32 - 500.0;
    if time_ms >= fade_start {
        (1.0 - (time_ms - fade_start) / 500.0).clamp(0.0, 1.0)
    } else {
        1.0
    }
}
