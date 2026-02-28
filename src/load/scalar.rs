use std::borrow::Cow;

use num_bigint::BigInt;

use crate::load::value::Value;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpecialFloat {
    Inf,
    Nan,
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

    if !(bytes[4] == b'-' && bytes[7] == b'-') {
        return false;
    }

    if !bytes[0].is_ascii_digit()
        || !bytes[1].is_ascii_digit()
        || !bytes[2].is_ascii_digit()
        || !bytes[3].is_ascii_digit()
        || !bytes[5].is_ascii_digit()
        || !bytes[6].is_ascii_digit()
        || !bytes[8].is_ascii_digit()
        || !bytes[9].is_ascii_digit()
    {
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

    let mut vec = bytes.to_vec();
    vec.retain(|&byte| byte != b'_');
    // SAFETY: Input is valid UTF-8 and only ASCII underscores are removed.
    Cow::Owned(unsafe { String::from_utf8_unchecked(vec) })
}

#[inline]
pub(crate) fn is_inf_nan(bytes: &[u8]) -> Option<(SpecialFloat, bool)> {
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
        (b'i', b'n', b'f') => Some((SpecialFloat::Inf, neg)),
        (b'n', b'a', b'n') => Some((SpecialFloat::Nan, neg)),
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

    let len = bytes.len();
    let mut i = 0usize;

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
            b'.' if !has_dot && !has_exp => has_dot = true,
            b'e' | b'E' if has_digit && !has_exp => {
                has_exp = true;
                has_digit = false;
                if i + 1 < len {
                    // SAFETY: Guarded by `i + 1 < len`.
                    let next = unsafe { *bytes.get_unchecked(i + 1) };
                    if matches!(next, b'+' | b'-') {
                        i += 1;
                    }
                }
            }
            _ => return false,
        }
        i += 1;
    }

    has_digit && (has_dot || has_exp)
}

pub(crate) fn parse_float(str: &str) -> Option<f64> {
    if str.is_empty() {
        return None;
    }

    if let Some((kind, neg)) = is_inf_nan(str.as_bytes()) {
        return Some(match kind {
            SpecialFloat::Nan => f64::NAN,
            SpecialFloat::Inf => {
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

#[inline]
fn digit_to_value(byte: u8, radix: u32) -> Option<u64> {
    let value = match byte {
        b'0'..=b'9' => u64::from(byte - b'0'),
        b'a'..=b'f' => u64::from(byte - b'a' + 10),
        b'A'..=b'F' => u64::from(byte - b'A' + 10),
        _ => return None,
    };

    if value < u64::from(radix) {
        Some(value)
    } else {
        None
    }
}

#[inline]
fn parse_i64(bytes: &[u8], radix: u32, neg: bool) -> Option<i64> {
    let mut acc = 0u64;
    let mut has_digit = false;
    let limit = if neg {
        i64::MIN.unsigned_abs()
    } else {
        i64::MAX as u64
    };

    for &byte in bytes {
        if byte == b'_' {
            continue;
        }

        let digit = digit_to_value(byte, radix)?;
        has_digit = true;
        acc = acc.checked_mul(u64::from(radix))?;
        acc = acc.checked_add(digit)?;
        if acc > limit {
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
        return Some(Value::IntegerI64(i_64));
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

    BigInt::parse_bytes(digits.as_bytes(), radix).map(|big_int| {
        if neg {
            Value::IntegerBig(-big_int)
        } else {
            Value::IntegerBig(big_int)
        }
    })
}
