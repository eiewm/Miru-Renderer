use std::f32::consts::PI;
#[derive(Debug, Clone)]
pub struct KeyAction {
    pub time: i32,
    pub column: u8,
    pub pressed: bool,
}
#[derive(Debug, Clone)]
pub struct KeyMaskEvent {
    pub time: i32,
    pub mask: u8,
}
pub fn exp_decay(age_ms: f32, life_ms: f32) -> f32 {
    if age_ms < 0.0 {
        return 0.0;
    }
    let tau = life_ms / 3.0;
    (-age_ms / tau).exp()
}
pub fn linear_decay(age_ms: f32, life_ms: f32) -> f32 {
    if age_ms < 0.0 || age_ms >= life_ms {
        return 0.0;
    }
    1.0 - age_ms / life_ms
}
pub fn pulse_alpha(time_ms: f32, period_ms: f32, min: f32, max: f32) -> f32 {
    let mid = (min + max) / 2.0;
    let amp = (max - min) / 2.0;
    mid + amp * ((time_ms / period_ms) * 2.0 * PI).sin()
}
pub fn build_key_mask_timeline(actions: &[KeyAction]) -> Vec<KeyMaskEvent> {
    if actions.is_empty() {
        return vec![KeyMaskEvent {
            time: i32::MIN,
            mask: 0,
        }];
    }
    let mut sorted: Vec<_> = actions.iter().collect();
    sorted.sort_by_key(|a| a.time);
    let mut events = Vec::with_capacity(sorted.len() + 1);
    // The sentinel makes key lookups before the first action return a stable empty mask.
    events.push(KeyMaskEvent {
        time: i32::MIN,
        mask: 0,
    });
    let mut mask: u8 = 0;
    for a in sorted {
        let bit = 1u8 << a.column;
        if a.pressed {
            mask |= bit;
        } else {
            mask &= !bit;
        }
        events.push(KeyMaskEvent { time: a.time, mask });
    }
    events
}
pub fn get_keys_mask_at(t: i32, events: &[KeyMaskEvent]) -> u8 {
    if events.is_empty() {
        return 0;
    }
    // Timeline events store state after each action, so use the last event at or before t.
    let mut lo = 0usize;
    let mut hi = events.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if events[mid].time <= t {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if lo == 0 {
        0
    } else {
        events[lo - 1].mask
    }
}
