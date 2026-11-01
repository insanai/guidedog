//! Fixed series keep SVG geometry independent of the host's libm implementation.

pub(crate) fn log2(value: f64) -> f64 {
    // Inputs are positive normal ratios of bounded musical durations.
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32 - 1023;
    let mantissa = f64::from_bits((bits & ((1u64 << 52) - 1)) | (1023u64 << 52));
    let z = (mantissa - 1.0) / (mantissa + 1.0);
    let square = z * z;
    let mut power = z;
    let mut sum = 0.0;
    for denominator in (1..=31).step_by(2) {
        sum += power / f64::from(denominator);
        power *= square;
    }
    f64::from(exponent) + 2.0 * sum * std::f64::consts::LOG2_E
}

pub(crate) fn atan(value: f64) -> f64 {
    let sign = if value < 0.0 { -1.0 } else { 1.0 };
    let mut x = value.abs();
    let reciprocal = x > 1.0;
    if reciprocal {
        x = 1.0 / x;
    }
    let shifted = x > 0.41421356237309503;
    if shifted {
        x = (x - 1.0) / (x + 1.0);
    }
    let square = x * x;
    let mut power = x;
    let mut sum = 0.0;
    for term in 0..24 {
        sum += power / f64::from(2 * term + 1);
        power *= -square;
    }
    if shifted {
        sum += std::f64::consts::FRAC_PI_4;
    }
    if reciprocal {
        sum = std::f64::consts::FRAC_PI_2 - sum;
    }
    sign * sum
}
