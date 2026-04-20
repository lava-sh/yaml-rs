use crate::from_rust::memchr::{contains_zero_byte, repeat_u8};

#[inline]
fn find_exp_index(bytes: &[u8]) -> Option<usize> {
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'e' || byte == b'E' {
            return Some(index);
        }
        index += 1;
    }
    None
}

#[inline]
unsafe fn contains_dot(bytes: &[u8]) -> bool {
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

pub(crate) fn normalize_float(repr: &str) -> String {
    let bytes = repr.as_bytes();

    match bytes {
        b"inf" => return String::from(".inf"),
        b"-inf" => return String::from("-.inf"),
        b"nan" => return String::from(".nan"),
        _ => {}
    }

    let Some(exp_index) = find_exp_index(bytes) else {
        return repr.to_owned();
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
