//! Segment endpoints are shared by collision scoring, quantization and final drawing.

use gd_score::Error;

use crate::beams::Stem;
use crate::limits::{charge, failure};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Segment {
    pub(crate) left: f64,
    pub(crate) right: f64,
    pub(crate) level: u8,
}

pub(crate) fn collect(
    stems: &[Stem],
    space: f64,
    work: &mut usize,
) -> Result<Vec<Segment>, Error> {
    let mut result = Vec::new();
    let levels = stems.iter().map(|s| s.flags).max().unwrap_or(0);
    for level in 0..levels {
        for (index, stem) in stems.iter().enumerate() {
            charge(work, 1)?;
            if stem.flags <= level {
                continue;
            }
            let next = stems.get(index + 1).filter(|next| next.flags > level);
            let previous =
                index.checked_sub(1).and_then(|i| stems.get(i)).filter(|prev| prev.flags > level);
            let (left, right) = if let Some(next) = next {
                (stem.x, next.x)
            } else if previous.is_some() {
                continue;
            } else {
                let right = stems.get(index + 1).map(|s| s.x - stem.x);
                let left = index.checked_sub(1).map(|i| stem.x - stems[i].x);
                let right_beat = stems.get(index + 1).is_some_and(|s| s.beat == stem.beat);
                let left_beat = index.checked_sub(1).is_some_and(|i| stems[i].beat == stem.beat);
                if right.is_some() && (left.is_none() || (right_beat && !left_beat)) {
                    (stem.x, stem.x + right.unwrap_or(space).min(1.25 * space))
                } else {
                    (stem.x - left.unwrap_or(space).min(1.25 * space), stem.x)
                }
            };
            result.try_reserve(1).map_err(|_| failure("Cannot reserve beam segments."))?;
            result.push(Segment { left, right, level });
        }
    }
    Ok(result)
}
