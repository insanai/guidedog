//! A chord's dots share a right column and choose the nearest free staff spaces.

use crate::prepare::EventInfo;

pub(crate) fn arrange(info: &mut EventInfo) {
    let mut used = Vec::<f64>::new();
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
        for (index, dot) in head.dots.iter_mut().enumerate() {
            dot.y = y;
            dot.x = info.bounds.right - info.voice_shift + 0.5 + index as f64 * 0.6;
        }
    }
}
