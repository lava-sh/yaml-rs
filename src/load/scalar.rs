use std::borrow::Cow;

use crate::{from_rust::dec2flt::is_8digits, load::value::Value};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum FloatingPointCategory {
    Infinite,
    NotANumber,
}

#[inline]
pub(crate) fn is_null(str: &str) -> bool {
    // https://yaml.org/spec/1.2.2/#1031-tags
    // Regular expression: null | Null | NULL | ~
    matches!(str, "null" | "Null" | "NULL" | "~")
}

#[inline]
pub(crate) fn is_bool(str: &str) -> Option<bool> {
    // https://yaml.org/spec/1.2.2/#1031-tags
    // Regular expression: true | True | TRUE | false | False | FALSE
    match str {
        "true" | "True" | "TRUE" => Some(true),
        "false" | "False" | "FALSE" => Some(false),
        _ => None,
    }
}

#[inline]
pub(crate) fn is_datetime(bytes: &[u8]) -> bool {
    if bytes.len() < 10 {
        return false;
    }

    if bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }

    let digits = u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[5], bytes[6], bytes[8], bytes[9],
    ]);

    if !is_8digits(digits) {
        return false;
    }

    if bytes.len() == 10 {
        return true;
    }

    matches!(bytes[10], b'T' | b't' | b' ')
}

#[inline]
pub(crate) fn normalize_num(str: &str) -> Cow<'_, str> {
    let bytes = str.as_bytes();

    if memchr::memchr(b'_', bytes).is_none() {
        return Cow::Borrowed(str);
    }

    let mut vec = Vec::with_capacity(bytes.len());
    for &b in bytes {
        if b != b'_' {
            vec.push(b);
        }
    }
    // SAFETY: Input is valid UTF-8 and only ASCII underscores are removed.
    Cow::Owned(unsafe { String::from_utf8_unchecked(vec) })
}

#[inline]
pub(crate) fn is_inf_nan(bytes: &[u8]) -> Option<(FloatingPointCategory, bool)> {
    if bytes.len() < 3 || bytes.len() > 5 {
        return None;
    }

    let mut i = 0usize;
    let neg = match bytes[0] {
        b'+' => {
            i = 1;
            false
        }
        b'-' => {
            i = 1;
            true
        }
        _ => false,
    };

    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
    }

    if bytes.len() - i != 3 {
        return None;
    }

    // SAFETY: We checked that `bytes.len() - i == 3`, so `i..i+3` is in-bounds.
    let (a, b, c) = unsafe {
        (
            *bytes.get_unchecked(i) | 0x20,
            *bytes.get_unchecked(i + 1) | 0x20,
            *bytes.get_unchecked(i + 2) | 0x20,
        )
    };

    match (a, b, c) {
        (b'i', b'n', b'f') => Some((FloatingPointCategory::Infinite, neg)),
        (b'n', b'a', b'n') => Some((FloatingPointCategory::NotANumber, neg)),
        _ => None,
    }
}

#[inline]
pub(crate) fn is_float(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }

    if is_inf_nan(bytes).is_some() {
        return true;
    }

    let mut i = 0;
    let len = bytes.len();

    if matches!(bytes[0], b'+' | b'-') {
        i = 1;
        if i >= len {
            return false;
        }
    }

    let mut has_digit = false;
    let mut has_dot = false;
    let mut has_exp = false;

    while i < len {
        // SAFETY: The loop invariant ensures `i < len` for each access.
        let byte = unsafe { *bytes.get_unchecked(i) };
        match byte {
            b'0'..=b'9' => has_digit = true,
            b'_' => {}
            b'.' if !has_dot && !has_exp => {
                has_dot = true;
                i += 1;

                while i < len && matches!(bytes[i], b'0'..=b'9' | b'_') {
                    if bytes[i] != b'_' {
                        has_digit = true;
                    }
                    i += 1;
                }

                if i < len && matches!(bytes[i], b'e' | b'E') {
                    has_exp = true;
                    i += 1;
                    break;
                }
                return i >= len && has_digit;
            }
            b'e' | b'E' if has_digit && !has_exp => {
                has_exp = true;
                i += 1;
                break;
            }
            _ => return false,
        }
        i += 1;
    }

    if has_exp {
        if i < len && matches!(bytes[i], b'+' | b'-') {
            i += 1;
        }

        let mut exp_has_digit = false;
        while i < len {
            match bytes[i] {
                b'0'..=b'9' => exp_has_digit = true,
                b'_' => {}
                _ => return false,
            }
            i += 1;
        }

        return exp_has_digit;
    }

    has_digit && (has_dot || has_exp)
}

pub(crate) fn parse_float(str: &str) -> Option<f64> {
    if str.is_empty() {
        return None;
    }

    if let Some((kind, neg)) = is_inf_nan(str.as_bytes()) {
        return Some(match kind {
            FloatingPointCategory::NotANumber => f64::NAN,
            FloatingPointCategory::Infinite => {
                if neg {
                    f64::NEG_INFINITY
                } else {
                    f64::INFINITY
                }
            }
        });
    }

    lexical_core::parse::<f64>(normalize_num(str).as_bytes()).ok()
}

#[inline]
pub(crate) fn is_int(bytes: &[u8]) -> bool {
    let bytes = if let [b'+' | b'-', rest @ ..] = bytes {
        rest
    } else {
        bytes
    };

    if bytes.is_empty() {
        return false;
    }

    if let [
        b'0',
        pref @ (b'x' | b'X' | b'o' | b'O' | b'b' | b'B'),
        rest @ ..,
    ] = bytes
    {
        if rest.is_empty() {
            return false;
        }
        let mut has_digit = false;

        for &byte in rest {
            if byte == b'_' {
                continue;
            }
            let valid = match pref {
                b'x' | b'X' => byte.is_ascii_hexdigit(),
                b'o' | b'O' => matches!(byte, b'0'..=b'7'),
                _ => matches!(byte, b'0' | b'1'),
            };
            if !valid {
                return false;
            }
            has_digit = true;
        }
        return has_digit;
    }

    let mut has_digit = false;

    for &byte in bytes {
        if byte == b'_' {
            continue;
        }
        if byte.is_ascii_digit() {
            has_digit = true;
            continue;
        }
        return false;
    }
    has_digit
}

static CHAR_TO_DIGIT: [u8; 256] = {
    let mut t = [0xFF; 256];
    let mut i = 0;
    while i < 256 {
        t[i] = if i >= b'0' as usize && i <= b'9' as usize {
            i as u8 - b'0'
        } else if i >= b'a' as usize && i <= b'f' as usize {
            i as u8 - b'a' + 10
        } else if i >= b'A' as usize && i <= b'F' as usize {
            i as u8 - b'A' + 10
        } else {
            0xFF
        };
        i += 1;
    }
    t
};

#[inline]
fn parse_i64(bytes: &[u8], radix: u32, neg: bool) -> Option<i64> {
    let mut acc = 0u64;
    let mut has_digit = false;

    let lim = if neg {
        i64::MIN.unsigned_abs()
    } else {
        i64::MAX as u64
    };

    for &byte in bytes {
        if byte == b'_' {
            continue;
        }

        let digit = CHAR_TO_DIGIT[byte as usize];
        if digit == 0xFF || digit >= radix as u8 {
            return None;
        }

        has_digit = true;
        acc = acc
            .checked_mul(u64::from(radix))?
            .checked_add(u64::from(digit))?;

        if acc > lim {
            return None;
        }
    }

    if !has_digit {
        return None;
    }

    if neg {
        if acc == i64::MIN.unsigned_abs() {
            Some(i64::MIN)
        } else {
            Some(-i64::try_from(acc).ok()?)
        }
    } else {
        i64::try_from(acc).ok()
    }
}

pub(crate) fn parse_int<'a>(str: &str) -> Option<Value<'a>> {
    if str.is_empty() {
        return None;
    }

    let bytes = str.as_bytes();
    let (neg, sign_offset, mut index) = match bytes[0] {
        b'+' => (false, 1usize, 1usize),
        b'-' => (true, 1usize, 1usize),
        _ => (false, 0usize, 0usize),
    };

    if index >= bytes.len() {
        return None;
    }

    let mut radix = 10u32;
    let mut prefixed = false;
    if bytes[index] == b'0' && index + 1 < bytes.len() {
        match bytes[index + 1] {
            b'x' | b'X' => {
                radix = 16;
                index += 2;
                prefixed = true;
            }
            b'o' | b'O' => {
                radix = 8;
                index += 2;
                prefixed = true;
            }
            b'b' | b'B' => {
                radix = 2;
                index += 2;
                prefixed = true;
            }
            _ => {}
        }
    }

    let digits = &bytes[index..];

    if digits.is_empty() {
        return None;
    }

    if let Some(i_64) = parse_i64(digits, radix, neg) {
        return Some(Value::Integer64(i_64));
    }

    let norm = normalize_num(&str[sign_offset..]);
    let normalized = norm.as_ref();
    let digits = if prefixed {
        if normalized.len() < 2 {
            return None;
        }
        &normalized[2..]
    } else {
        normalized
    };
    if digits.is_empty() {
        return None;
    }

    num_bigint::BigInt::parse_bytes(digits.as_bytes(), radix).map(|big_int| {
        if neg {
            Value::BigInteger(-big_int)
        } else {
            Value::BigInteger(big_int)
        }
    })
}
