//! Integers and floats.

use crate::Data;

/// Why a number token was refused: a message and a hint.
pub(crate) type Refusal = (&'static str, &'static str);

const UNDERSCORE: Refusal = (
    "This number has a misplaced underscore or a character that is not a digit.",
    "Underscores may only stand between two digits, as in 1_000.",
);

/// Reports whether `s` is digits of `radix` with single underscores between them.
fn digits_ok(s: &str, radix: u32) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b[0] != b'_'
        && b[b.len() - 1] != b'_'
        && !s.contains("__")
        && s.chars().all(|c| c == '_' || c.is_digit(radix))
}

/// Removes underscores, borrowing when there are none.
fn plain(s: &str) -> std::borrow::Cow<'_, str> {
    if s.contains('_') { s.replace('_', "").into() } else { s.into() }
}

/// Parses a number token (integer, float, `inf` or `nan`, with sign).
pub(crate) fn parse(token: &str) -> Result<Data, Refusal> {
    match token {
        "inf" | "+inf" => return Ok(Data::Float(f64::INFINITY)),
        "-inf" => return Ok(Data::Float(f64::NEG_INFINITY)),
        "nan" | "+nan" | "-nan" => return Ok(Data::Float(f64::NAN)),
        _ => {}
    }
    for (prefix, radix) in [("0x", 16), ("0o", 8), ("0b", 2)] {
        if let Some(rest) = token.strip_prefix(prefix) {
            return radix_integer(rest, radix);
        }
    }
    if token.contains(['.', 'e', 'E']) { float(token) } else { decimal(token) }
}

fn radix_integer(rest: &str, radix: u32) -> Result<Data, Refusal> {
    if !digits_ok(rest, radix) {
        return Err(UNDERSCORE);
    }
    match i64::from_str_radix(&plain(rest), radix) {
        Ok(n) => Ok(Data::Integer(n)),
        Err(_) => Err(TOO_LARGE),
    }
}

const TOO_LARGE: Refusal = (
    "This integer does not fit in 64 bits.",
    "Integers range from -9223372036854775808 to 9223372036854775807; quote larger ones.",
);

fn unsigned_part(token: &str) -> &str {
    token.strip_prefix(['+', '-']).unwrap_or(token)
}

fn check_leading_zero(digits: &str) -> Result<(), Refusal> {
    if digits.len() > 1 && digits.starts_with('0') {
        return Err((
            "A decimal number may not start with a zero.",
            "Remove the leading zeros, or write 0o for octal or 0x for hexadecimal.",
        ));
    }
    Ok(())
}

fn decimal(token: &str) -> Result<Data, Refusal> {
    let digits = unsigned_part(token);
    if !digits_ok(digits, 10) {
        return Err((
            "This is not a number, a string, true or false, an array or a table.",
            "Put text in quotes, as in name = \"value\".",
        ));
    }
    check_leading_zero(digits)?;
    match plain(token).parse::<i64>() {
        Ok(n) => Ok(Data::Integer(n)),
        Err(_) => Err(TOO_LARGE),
    }
}

fn float(token: &str) -> Result<Data, Refusal> {
    const SHAPE: Refusal = (
        "This float is malformed.",
        "Write digits on both sides of the point, as in 3.14, and an exponent as in 1e6.",
    );
    let body = unsigned_part(token);
    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(i) => (&body[..i], Some(&body[i + 1..])),
        None => (body, None),
    };
    let (whole, fraction) = match mantissa.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (mantissa, None),
    };
    if !digits_ok(whole, 10) || fraction.is_some_and(|f| !digits_ok(f, 10)) {
        return Err(SHAPE);
    }
    check_leading_zero(whole)?;
    if let Some(e) = exponent
        && !digits_ok(unsigned_part(e), 10)
    {
        return Err(SHAPE);
    }
    match plain(token).parse::<f64>() {
        Ok(f) => Ok(Data::Float(f)),
        Err(_) => Err(SHAPE),
    }
}
