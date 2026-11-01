use std::cmp::Ordering;
use std::fmt;

use crate::{Error, ErrorKind};

/// A reduced exact rational with a strictly positive denominator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ratio {
    num: i64,
    den: i64,
}

impl Ratio {
    /// Exact zero.
    pub const ZERO: Self = Self { num: 0, den: 1 };
    /// Exact one.
    pub const ONE: Self = Self { num: 1, den: 1 };

    /// Reduces a fraction. Fails for a zero denominator or an unrepresentable result.
    pub fn new(num: i64, den: i64) -> Result<Self, Error> {
        Self::reduce(i128::from(num), i128::from(den))
    }

    /// Returns the reduced numerator.
    pub fn numerator(self) -> i64 {
        self.num
    }

    /// Returns the positive reduced denominator.
    pub fn denominator(self) -> i64 {
        self.den
    }

    /// Converts to a finite floating approximation for geometry, not musical time.
    pub fn to_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }

    /// Adds two rationals, failing when the reduced result does not fit.
    pub fn checked_add(self, rhs: Self) -> Result<Self, Error> {
        Self::reduce(
            i128::from(self.num) * i128::from(rhs.den)
                + i128::from(rhs.num) * i128::from(self.den),
            i128::from(self.den) * i128::from(rhs.den),
        )
    }

    /// Subtracts two rationals, failing when the reduced result does not fit.
    pub fn checked_sub(self, rhs: Self) -> Result<Self, Error> {
        Self::reduce(
            i128::from(self.num) * i128::from(rhs.den)
                - i128::from(rhs.num) * i128::from(self.den),
            i128::from(self.den) * i128::from(rhs.den),
        )
    }

    /// Multiplies two rationals, failing when the reduced result does not fit.
    pub fn checked_mul(self, rhs: Self) -> Result<Self, Error> {
        Self::reduce(
            i128::from(self.num) * i128::from(rhs.num),
            i128::from(self.den) * i128::from(rhs.den),
        )
    }

    /// Divides two rationals, failing for zero or an unrepresentable reduced result.
    pub fn checked_div(self, rhs: Self) -> Result<Self, Error> {
        Self::reduce(
            i128::from(self.num) * i128::from(rhs.den),
            i128::from(self.den) * i128::from(rhs.num),
        )
    }

    /// Negates the value, failing only for the smallest integer numerator.
    pub fn checked_neg(self) -> Result<Self, Error> {
        Self::reduce(-i128::from(self.num), i128::from(self.den))
    }

    /// Returns whether this value is a whole integer, without converting to floating point.
    pub fn is_integer(self) -> bool {
        self.den == 1
    }

    fn reduce(mut num: i128, mut den: i128) -> Result<Self, Error> {
        if den == 0 {
            return Err(Error::new(
                ErrorKind::Duration,
                "A fraction or duration multiplier has a zero denominator.",
            )
            .hint("Use a positive denominator such as `*2/3` or `\\tuplet 3/2`."));
        }
        if den < 0 {
            num = -num;
            den = -den;
        }
        let (mut a, mut b) = (num.abs(), den);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        Ok(Self {
            num: i64::try_from(num / a).map_err(|_| overflow())?,
            den: i64::try_from(den / a).map_err(|_| overflow())?,
        })
    }
}

impl Default for Ratio {
    fn default() -> Self {
        Self::ZERO
    }
}

impl Ord for Ratio {
    fn cmp(&self, rhs: &Self) -> Ordering {
        (i128::from(self.num) * i128::from(rhs.den))
            .cmp(&(i128::from(rhs.num) * i128::from(self.den)))
    }
}

impl PartialOrd for Ratio {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

impl fmt::Display for Ratio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.den == 1 {
            write!(f, "{}", self.num)
        } else {
            write!(f, "{}/{}", self.num, self.den)
        }
    }
}

fn overflow() -> Error {
    Error::new(ErrorKind::TimeOverflow, "The exact musical time does not fit an i64 fraction.")
        .hint("Use smaller duration multipliers in the input and fewer nested tuplets.")
}
