//! Fixed-point text of Python `Decimal` values and byte sizes, rounded half to even.

use crate::cost_report::constants::{BYTE_SCALE, BYTE_UNITS};
use crate::cost_report::models::DecimalSortKey;

/// The digits and power of ten of a finite decimal text, with its sign.
struct DecimalParts {
    negative: bool,
    digits: String,
    exponent: i64,
}

fn parts(text: &str) -> Option<DecimalParts> {
    let trimmed: &str = text.trim();
    let (negative, unsigned) = match trimmed.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };
    let (mantissa, exponent) = match unsigned.find(['e', 'E']) {
        Some(position) => {
            let Ok(exponent) = unsigned[position + 1..].parse::<i64>() else {
                return None;
            };
            (&unsigned[..position], exponent)
        }
        None => (unsigned, 0),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits: String = format!("{whole}{fraction}");
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let Ok(places) = i64::try_from(fraction.len()) else {
        return None;
    };
    Some(DecimalParts {
        negative,
        exponent: exponent - places,
        digits,
    })
}

/// Whether the text is a finite decimal Python's `Decimal` reads.
pub(crate) fn is_decimal(text: &str) -> bool {
    parts(text).is_some()
}

/// Exact finite-decimal ordering without rounding or expanding powers of ten.
pub(crate) fn sort_key(text: &str) -> Option<DecimalSortKey> {
    let value = parts(text)?;
    let digits = value.digits.trim_start_matches('0');
    if digits.is_empty() {
        return Some(DecimalSortKey::Zero);
    }
    let magnitude = (
        i128::from(value.exponent) + digits.len() as i128,
        digits.trim_end_matches('0').to_owned(),
    );
    Some(if value.negative {
        DecimalSortKey::Negative(std::cmp::Reverse(magnitude))
    } else {
        DecimalSortKey::Positive(magnitude)
    })
}

/// Round the digit string half to even after removing its last `dropped` digits.
fn rounded(digits: &str, dropped: usize) -> String {
    if dropped == 0 {
        return digits.to_owned();
    }
    let padded: String = if dropped > digits.len() {
        format!("{}{digits}", "0".repeat(dropped - digits.len()))
    } else {
        digits.to_owned()
    };
    let (kept, tail) = padded.split_at(padded.len() - dropped);
    let first: u8 = tail.as_bytes()[0];
    let rest_zero: bool = tail[1..].bytes().all(|byte| byte == b'0');
    let last_odd: bool = kept
        .bytes()
        .last()
        .is_some_and(|byte| (byte - b'0') % 2 == 1);
    let up: bool = first > b'5' || (first == b'5' && (!rest_zero || last_odd));
    let mut result: Vec<u8> = format!("0{kept}").into_bytes();
    if up {
        for byte in result.iter_mut().rev() {
            if *byte == b'9' {
                *byte = b'0';
            } else {
                *byte += 1;
                break;
            }
        }
    }
    String::from_utf8(result).unwrap_or_default()
}

/// `format(Decimal(text), f".{places}f")`; non-finite values keep Python's spelling.
pub(crate) fn fixed(text: &str, places: usize) -> String {
    let Some(parts) = parts(text) else {
        return text.trim().to_owned();
    };
    let scale: i64 = parts.exponent + i64::try_from(places).unwrap_or(i64::MAX);
    let scaled: String = match usize::try_from(scale) {
        Ok(zeros) => format!("{}{}", parts.digits, "0".repeat(zeros)),
        Err(_) => rounded(&parts.digits, usize::try_from(-scale).unwrap_or(usize::MAX)),
    };
    let padded: String = format!("{scaled:0>width$}", width = places + 1);
    let (whole, fraction) = padded.split_at(padded.len() - places);
    let whole: &str = whole.trim_start_matches('0');
    let whole: &str = if whole.is_empty() { "0" } else { whole };
    let sign: &str = if parts.negative { "-" } else { "" };
    if places == 0 {
        return format!("{sign}{whole}");
    }
    format!("{sign}{whole}.{fraction}")
}

/// `_format_bytes`: whole bytes below a KiB, else one decimal of the largest fitting unit.
pub(crate) fn byte_size(value: i64) -> String {
    let Ok(bytes) = u128::try_from(value) else {
        return format!("{value} {}", BYTE_UNITS[0]);
    };
    if bytes < BYTE_SCALE {
        return format!("{value} {}", BYTE_UNITS[0]);
    }
    let mut divisor: u128 = BYTE_SCALE;
    for (index, unit) in BYTE_UNITS.iter().enumerate().skip(1) {
        if bytes < divisor * BYTE_SCALE || index == BYTE_UNITS.len() - 1 {
            let numerator: u128 = bytes * 10;
            let quotient: u128 = numerator / divisor;
            let remainder: u128 = numerator % divisor;
            let twice: u128 = remainder * 2;
            let tenths: u128 = if twice > divisor || (twice == divisor && quotient % 2 == 1) {
                quotient + 1
            } else {
                quotient
            };
            return format!("{}.{} {unit}", tenths / 10, tenths % 10);
        }
        divisor *= BYTE_SCALE;
    }
    format!("{value} {}", BYTE_UNITS[0])
}
