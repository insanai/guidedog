use gd_score::Error;

use crate::limits::charge;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Segment {
    pub(crate) left: f64,
    pub(crate) right: f64,
    pub(crate) first: f64,
    pub(crate) last: f64,
}

impl Segment {
    fn at(self, x: f64) -> f64 {
        self.first + (self.last - self.first) * (x - self.left) / (self.right - self.left)
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Skyline {
    pub(crate) segments: Vec<Segment>,
}

impl Skyline {
    pub(crate) fn add(
        &mut self,
        segment: Segment,
        upper: bool,
        work: &mut usize,
    ) -> Result<(), Error> {
        if segment.right <= segment.left {
            return Ok(());
        }
        let next = Self { segments: vec![segment] };
        *self = self.merge(&next, upper, work)?;
        Ok(())
    }

    pub(crate) fn merge(
        &self,
        other: &Self,
        upper: bool,
        work: &mut usize,
    ) -> Result<Self, Error> {
        let boundaries = boundaries(&self.segments, &other.segments, work)?;
        let (mut a, mut b) = (0, 0);
        let mut result = Self::default();
        for pair in boundaries.windows(2) {
            charge(work, 1)?;
            let (left, right) = (pair[0], pair[1]);
            if right <= left {
                continue;
            }
            while self.segments.get(a).is_some_and(|s| s.right <= left) {
                a += 1;
            }
            while other.segments.get(b).is_some_and(|s| s.right <= left) {
                b += 1;
            }
            let av = self.segments.get(a).copied().filter(|s| s.left <= left && s.right >= right);
            let bv = other.segments.get(b).copied().filter(|s| s.left <= left && s.right >= right);
            match (av, bv) {
                (Some(a), Some(b)) => result.overlap(a, b, left, right, upper),
                (Some(a), None) | (None, Some(a)) => {
                    result.segment(Segment { left, right, first: a.at(left), last: a.at(right) })
                }
                (None, None) => {}
            }
        }
        Ok(result)
    }

    fn overlap(&mut self, a: Segment, b: Segment, left: f64, right: f64, upper: bool) {
        let da = a.at(left) - b.at(left);
        let db = a.at(right) - b.at(right);
        let choose = |difference: f64| if (difference <= 0.0) == upper { a } else { b };
        if da * db < 0.0 {
            let cross = left + (right - left) * da / (da - db);
            let first = choose(da);
            let last = choose(db);
            self.segment(Segment {
                left,
                right: cross,
                first: first.at(left),
                last: first.at(cross),
            });
            self.segment(Segment {
                left: cross,
                right,
                first: last.at(cross),
                last: last.at(right),
            });
        } else {
            let chosen = choose(if da == 0.0 { db } else { da });
            self.segment(Segment { left, right, first: chosen.at(left), last: chosen.at(right) });
        }
    }

    fn segment(&mut self, segment: Segment) {
        if let Some(last) = self.segments.last_mut()
            && last.right == segment.left
            && last.last == segment.first
            && (last.last - last.first) / (last.right - last.left)
                == (segment.last - segment.first) / (segment.right - segment.left)
        {
            last.right = segment.right;
            last.last = segment.last;
            return;
        }
        self.segments.push(segment);
    }

    /// Maximum difference between facing envelopes over their common x intervals.
    pub(crate) fn distance(&self, upper: &Self, work: &mut usize) -> Result<f64, Error> {
        let (mut a, mut b) = (0, 0);
        let mut distance = f64::NEG_INFINITY;
        while let (Some(low), Some(high)) = (self.segments.get(a), upper.segments.get(b)) {
            charge(work, 1)?;
            let (left, right) = (low.left.max(high.left), low.right.min(high.right));
            if left <= right {
                distance =
                    distance.max(low.at(left) - high.at(left)).max(low.at(right) - high.at(right));
            }
            if low.right <= high.right {
                a += 1;
            } else {
                b += 1;
            }
        }
        Ok(distance)
    }
}

fn boundaries(a: &[Segment], b: &[Segment], work: &mut usize) -> Result<Vec<f64>, Error> {
    let mut result = Vec::new();
    let (mut left, mut right) = (0, 0);
    while left < 2 * a.len() || right < 2 * b.len() {
        charge(work, 1)?;
        let endpoint = |segments: &[Segment], index: usize| {
            segments.get(index / 2).map(|s| if index.is_multiple_of(2) { s.left } else { s.right })
        };
        let value = match (endpoint(a, left), endpoint(b, right)) {
            (Some(x), Some(y)) if x <= y => {
                left += 1;
                x
            }
            (Some(_), Some(y)) | (None, Some(y)) => {
                right += 1;
                y
            }
            (Some(x), None) => {
                left += 1;
                x
            }
            (None, None) => break,
        };
        if result.last() != Some(&value) {
            result.push(value);
        }
    }
    Ok(result)
}
