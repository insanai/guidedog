/// A point in staff spaces during layout and in points in the public display list.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    /// Horizontal coordinate, increasing to the right.
    pub x: f64,
    /// Vertical coordinate, increasing downwards.
    pub y: f64,
}

/// An axis-aligned rectangle in the same units as its associated marks.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    /// Minimum horizontal coordinate.
    pub left: f64,
    /// Minimum vertical coordinate.
    pub top: f64,
    /// Maximum horizontal coordinate.
    pub right: f64,
    /// Maximum vertical coordinate.
    pub bottom: f64,
}

impl Rect {
    /// Returns a translated rectangle, borrowing no state and performing no allocation.
    pub fn translated(self, x: f64, y: f64) -> Self {
        Self {
            left: self.left + x,
            right: self.right + x,
            top: self.top + y,
            bottom: self.bottom + y,
        }
    }

    /// Returns the smallest rectangle enclosing both inputs.
    pub fn union(self, other: Self) -> Self {
        Self {
            left: self.left.min(other.left),
            right: self.right.max(other.right),
            top: self.top.min(other.top),
            bottom: self.bottom.max(other.bottom),
        }
    }

    /// Returns whether the interiors overlap beyond a nonnegative tolerance.
    pub fn overlaps(self, other: Self, tolerance: f64) -> bool {
        self.right.min(other.right) - self.left.max(other.left) > tolerance
            && self.bottom.min(other.bottom) - self.top.max(other.top) > tolerance
    }
}

/// A cubic Bezier curve with its two control points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bezier {
    /// Start.
    pub from: Point,
    /// First control point.
    pub first: Point,
    /// Second control point.
    pub second: Point,
    /// End.
    pub to: Point,
}

impl Bezier {
    /// Evaluates the polynomial at t; callers use t in the closed interval 0..=1.
    pub fn point(self, t: f64) -> Point {
        let u = 1.0 - t;
        Point {
            x: u * u * u * self.from.x
                + 3.0 * u * u * t * self.first.x
                + 3.0 * u * t * t * self.second.x
                + t * t * t * self.to.x,
            y: u * u * u * self.from.y
                + 3.0 * u * u * t * self.first.y
                + 3.0 * u * t * t * self.second.y
                + t * t * t * self.to.y,
        }
    }
}
