const LO_USIZE: usize = repeat_u8(0x01);
const HI_USIZE: usize = repeat_u8(0x80);

// https://github.com/rust-lang/rust/blob/1.95.0/library/core/src/num/mod.rs#L1431-L1434
#[inline]
const fn repeat_u8(x: u8) -> usize {
    usize::from_ne_bytes([x; size_of::<usize>()])
}

// https://github.com/rust-lang/rust/blob/1.95.0/library/core/src/slice/memchr.rs#L17-L20
#[inline]
const fn contains_zero_byte(x: usize) -> bool {
    x.wrapping_sub(LO_USIZE) & !x & HI_USIZE != 0
}

pub fn normalize_float_repr(repr: &str) -> String {
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
        // SAFETY: `mantissa` is a valid slice derived from `repr`, helper
        // only reads within slice bounds and handles the unaligned prefix
        // before performing aligned word loads
        contains_dot(mantissa)
    };

    let mut out = vec![0; bytes.len() + if has_dot { 0 } else { 2 }];

    // SAFETY: `out` has length `total_len`, so all writes below stay within
    // the allocated buffer. Source slices come from `repr`, and copied byte
    // counts exactly match the destination regions we calculated
    unsafe {
        let ptr = out.as_mut_ptr();

        std::ptr::copy_nonoverlapping(mantissa.as_ptr(), ptr, mantissa.len());

        let mut cursor = mantissa.len();

        if has_dot {
            *ptr.add(cursor) = b'e';
            cursor += 1;
        } else {
            *ptr.add(cursor) = b'.';
            *ptr.add(cursor + 1) = b'0';
            *ptr.add(cursor + 2) = b'e';
            cursor += 3;
        }

        std::ptr::copy_nonoverlapping(exp.as_ptr(), ptr.add(cursor), exp.len());

        // SAFETY: all bytes written above are ASCII bytes copied from `repr`
        // plus `.`, `0`, and `e`, so the final buffer is valid UTF-8
        String::from_utf8_unchecked(out)
    }
}

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
