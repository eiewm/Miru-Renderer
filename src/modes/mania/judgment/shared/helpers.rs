use crate::types::{Beatmap, JudgmentKind, KeyAction, Windows};
#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    pub time: i32,
    pub pressed: bool,
}
pub fn build_events_by_col(key_actions: &[KeyAction], key_count: u8) -> Vec<Vec<KeyEvent>> {
    let mut by_col: Vec<Vec<KeyEvent>> = (0..key_count).map(|_| Vec::new()).collect();
    for action in key_actions {
        let col = action.column as usize;
        if col < by_col.len() {
            by_col[col].push(KeyEvent {
                time: action.time,
                pressed: action.pressed,
            });
        }
    }
    for col_events in &mut by_col {
        col_events.sort_by_key(|e| e.time);
    }
    by_col
}
pub fn effective_key_count(map: &Beatmap) -> u8 {
    let mut key_count = map.key_count();
    if key_count == 0 {
        if let Some(max_col) = map.hit_objects.iter().map(|h| h.column).max() {
            key_count = max_col.saturating_add(1);
        }
    }
    key_count.max(1)
}
pub fn is_key_down_at(events: &[KeyEvent], time: i32) -> bool {
    let mut down = false;
    for e in events {
        if e.time > time {
            break;
        }
        down = e.pressed;
    }
    down
}
#[inline]
pub fn calc_hit_kind(dt: i32, w: &Windows) -> JudgmentKind {
    if dt <= w.max {
        JudgmentKind::Max
    } else if dt <= w.hit300 {
        JudgmentKind::Hit300
    } else if dt <= w.hit200 {
        JudgmentKind::Hit200
    } else if dt <= w.hit100 {
        JudgmentKind::Hit100
    } else if dt <= w.hit50 {
        JudgmentKind::Hit50
    } else {
        JudgmentKind::Miss
    }
}
#[inline]
pub fn calc_note_kind(dt: i32, w: &Windows) -> JudgmentKind {
    calc_hit_kind(dt, w)
}
pub fn find_first_press_from(events: &[KeyEvent], from_time: i32) -> Option<usize> {
    events.iter().position(|e| e.pressed && e.time >= from_time)
}
