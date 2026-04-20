use std::borrow::Cow;

use pyo3::PyResult;

use crate::{
    YAMLEncodeError,
    from_rust::memchr::{contains_zero_byte, repeat_u8},
};

pub(crate) fn is_nan_numeric_payload<const N: usize>(
    bytes: &[u8],
    start: usize,
    prefix: [u8; N],
) -> bool {
    if bytes.len() <= start || start != N {
        return false;
    }

    let ptr = bytes.as_ptr();
    let end = unsafe { ptr.add(bytes.len()) };

    let mut p = ptr;

    let mut i = 0usize;
    while i < N {
        // SAFETY: `start == N` and `bytes.len() > start`, so indices `0..N` are in bounds.
        let b = unsafe { *p } | 0x20;
        if b != prefix[i] {
            return false;
        }
        p = unsafe { p.add(1) };
        i += 1;
    }

    let mut p = unsafe { ptr.add(start) };

    while p < end {
        // SAFETY: `p < end` guarantees valid read.
        let byte = unsafe { *p };
        let d = byte.wrapping_sub(b'0');
        if d > 9 {
            return false;
        }
        p = unsafe { p.add(1) };
    }

    true
}

macro_rules! check_byte {
    ($chunk:expr, $offset:expr, $base:expr) => {{
        let b = unsafe {
            // SAFETY: caller guarantees base + offset < len
            *$chunk.add($offset)
        };
        if b == b'e' || b == b'E' {
            return Some($base + $offset);
        }
    }};
}

#[inline]
pub(crate) fn find_exp_index(bytes: &[u8]) -> Option<usize> {
    let ptr = bytes.as_ptr();
    let len = bytes.len();
    let mut i = 0usize;

    while i + 8 <= len {
        let chunk = unsafe {
            // SAFETY: i + 8 <= len ensures ptr.add(i..i+7) are in bounds
            ptr.add(i)
        };
        check_byte!(chunk, 0, i);
        check_byte!(chunk, 1, i);
        check_byte!(chunk, 2, i);
        check_byte!(chunk, 3, i);
        check_byte!(chunk, 4, i);
        check_byte!(chunk, 5, i);
        check_byte!(chunk, 6, i);
        check_byte!(chunk, 7, i);

        i += 8;
    }

    while i < len {
        let byte = unsafe {
            // SAFETY: i < len guaranteed by loop condition
            *ptr.add(i)
        };

        if byte == b'e' || byte == b'E' {
            return Some(i);
        }

        i += 1;
    }

    None
}
#[inline]
pub(crate) unsafe fn contains_dot(bytes: &[u8]) -> bool {
    const DOT_REPEATED: usize = repeat_u8(b'.');

    let len = bytes.len();

    if len < size_of::<usize>() {
        return bytes.contains(&b'.');
    }

    let ptr = bytes.as_ptr();
    let mut offset = ptr.align_offset(size_of::<usize>());

    if offset > 0 {
        offset = offset.min(len);
        if bytes[..offset].contains(&b'.') {
            return true;
        }
    }

    while offset <= len - size_of::<usize>() {
        // SAFETY: the loop condition guarantees the full word lies within
        // `bytes`, so reading an unaligned `usize` from this position is safe
        let word = unsafe { ptr.add(offset).cast::<usize>().read_unaligned() };

        if contains_zero_byte(word ^ DOT_REPEATED) {
            return bytes[offset..offset + size_of::<usize>()].contains(&b'.');
        }
        offset += size_of::<usize>();
    }

    bytes[offset..].contains(&b'.')
}

pub(crate) fn normalize_float(py_float: &str) -> String {
    let bytes = py_float.as_bytes();

    match bytes {
        b"inf" => return String::from(".inf"),
        b"-inf" => return String::from("-.inf"),
        b"nan" => return String::from(".nan"),
        _ => {}
    }

    let Some(exp_index) = find_exp_index(bytes) else {
        return py_float.to_owned();
    };

    let mantissa = &bytes[..exp_index];
    let exp = &bytes[exp_index + 1..];
    let has_dot = unsafe {
        // SAFETY: `mantissa` is a subslice of `repr.as_bytes()` and contains only valid reads.
        contains_dot(mantissa)
    };

    let mut out = vec![0; bytes.len() + if has_dot { 0 } else { 2 }];
    let ptr = out.as_mut_ptr();

    // SAFETY: source and destination pointers are valid and don't overlap.
    unsafe {
        std::ptr::copy_nonoverlapping(mantissa.as_ptr(), ptr, mantissa.len());
    }

    let mut cursor = mantissa.len();

    // SAFETY: `ptr.add(cursor)` is within bounds of out allocation.
    unsafe {
        if has_dot {
            *ptr.add(cursor) = b'e';
            cursor += 1;
        } else {
            *ptr.add(cursor) = b'.';
            *ptr.add(cursor + 1) = b'0';
            *ptr.add(cursor + 2) = b'e';
            cursor += 3;
        }
    }

    // SAFETY: source and destination pointers are valid and don't overlap.
    unsafe {
        std::ptr::copy_nonoverlapping(exp.as_ptr(), ptr.add(cursor), exp.len());
    }

    // SAFETY: All bytes are ASCII from valid UTF-8 input + '.', '0', and 'e'.
    unsafe { String::from_utf8_unchecked(out) }
}

pub(crate) fn normalize_decimal(repr: &str) -> PyResult<Cow<'_, str>> {
    let bytes = repr.as_bytes();
    let mut start = 0usize;
    let mut end = bytes.len();

    // SAFETY: bounds checked by loop condition
    while start < end && unsafe { bytes.get_unchecked(start) }.is_ascii_whitespace() {
        start += 1;
    }

    // SAFETY: bounds checked by loop condition
    while start < end && unsafe { bytes.get_unchecked(end - 1) }.is_ascii_whitespace() {
        end -= 1;
    }

    // SAFETY: start..end is within original string
    let trimmed = unsafe { repr.get_unchecked(start..end) };
    let bytes = trimmed.as_bytes();

    let mut offset = 0usize;
    let mut neg = false;

    if !bytes.is_empty() {
        // SAFETY: bytes is non-empty
        match unsafe { *bytes.get_unchecked(0) } {
            b'-' => {
                neg = true;
                offset = 1;
            }
            b'+' => {
                offset = 1;
            }
            _ => {}
        }
    }

    // SAFETY: offset <= bytes.len()
    let rest = unsafe { bytes.get_unchecked(offset..) };
    let len = rest.len();

    if len == 3 {
        // SAFETY: `len == 3`, so indices `0..3` are valid.
        let a = unsafe { *rest.get_unchecked(0) } | 0x20;
        let b = unsafe { *rest.get_unchecked(1) } | 0x20;
        let c = unsafe { *rest.get_unchecked(2) } | 0x20;

        if (a, b, c) == (b'n', b'a', b'n') {
            return Ok(Cow::Borrowed(".nan"));
        }
        if (a, b, c) == (b'i', b'n', b'f') {
            return Ok(if neg {
                Cow::Borrowed("-.inf")
            } else {
                Cow::Borrowed(".inf")
            });
        }
    }

    if len == 4 {
        // SAFETY: `len == 4`, so indices `0..4` are valid.
        let a = unsafe { *rest.get_unchecked(0) } | 0x20;
        let b = unsafe { *rest.get_unchecked(1) } | 0x20;
        let c = unsafe { *rest.get_unchecked(2) } | 0x20;
        let d = unsafe { *rest.get_unchecked(3) } | 0x20;

        if (a, b, c, d) == (b's', b'n', b'a', b'n') {
            return Ok(Cow::Borrowed(".nan"));
        }
    }

    if is_nan_numeric_payload(rest, 3, *b"nan") || is_nan_numeric_payload(rest, 4, *b"snan") {
        return Err(YAMLEncodeError::new_err(format!(
            "Cannot serialize invalid decimal.Decimal('{trimmed}') to YAML"
        )));
    }

    if len == 8 {
        let inf = b"infinity";
        let mut matches = true;
        let mut i = 0usize;

        while i < 8 {
            // SAFETY: `len == 8`, so `i < 8` keeps both reads in bounds.
            if (unsafe { *rest.get_unchecked(i) } | 0x20) != unsafe { *inf.get_unchecked(i) } {
                matches = false;
                break;
            }
            i += 1;
        }

        if matches {
            return Ok(if neg {
                Cow::Borrowed("-.inf")
            } else {
                Cow::Borrowed(".inf")
            });
        }
    }

    let mut has_dot = false;
    let mut has_exp = false;
    let mut i = 0usize;

    while i < bytes.len() {
        // SAFETY: loop guarantees i < bytes.len()
        match unsafe { *bytes.get_unchecked(i) } {
            b'.' => has_dot = true,
            b'e' | b'E' => has_exp = true,
            _ => {}
        }
        i += 1;
    }

    if !has_dot && !has_exp {
        let mut normalized = String::with_capacity(trimmed.len() + 2);
        normalized.push_str(trimmed);
        normalized.push_str(".0");
        return Ok(Cow::Owned(normalized));
    }

    Ok(Cow::Borrowed(trimmed))
}
