//! A chord's dots share a right column and choose the nearest free staff spaces.

use gd_score::EventId;

use crate::prepare::{EventInfo, Plan};

pub(crate) fn arrange(info: &mut EventInfo) {
    let mut used = Vec::<f64>::new();
    place_chord(info, info.bounds.right, &mut used);
}

pub(crate) fn arrange_staff_column(plan: &mut Plan, ids: &[EventId]) {
    let right =
        ids.iter().map(|id| plan.events[id.0 as usize].bounds.right).fold(0.0f64, f64::max);
    let mut ordered = ids.to_vec();
    ordered.sort_by_key(|id| !plan.events[id.0 as usize].up);
    let mut used = Vec::<f64>::new();
    for id in ordered {
        let info = &mut plan.events[id.0 as usize];
        place_chord(info, right, &mut used);
    }
}

fn place_chord(info: &mut EventInfo, column_right: f64, used: &mut Vec<f64>) {
    let mut indices: Vec<_> = (0..info.heads.len()).collect();
    indices.sort_by(|&a, &b| info.heads[a].at.y.total_cmp(&info.heads[b].at.y));
    if info.dot_down {
        indices.reverse();
    }
    for index in indices {
        let head = &mut info.heads[index];
        if head.dots.is_empty() || head.shared {
            continue;
        }
        let origin = head.at.y;
        let y = (-16..=16)
            .map(|step| origin.floor() + f64::from(step) + 0.5)
            .filter(|y| !used.iter().any(|old| (old - y).abs() < 0.4))
            .min_by(|a, b| {
                (a - origin)
                    .abs()
                    .total_cmp(&(b - origin).abs())
                    .then_with(|| if info.dot_down { b.total_cmp(a) } else { a.total_cmp(b) })
            })
            .unwrap_or(origin + 16.5);
        used.push(y);
        for (dot_idx, dot) in head.dots.iter_mut().enumerate() {
            dot.y = y;
            dot.x = column_right - info.voice_shift + 0.5 + dot_idx as f64 * 0.6;
        }
    }
}
