// Determine if all characters in an 8-byte byte string (represented as a `u64`) are all decimal
// digits.
//
// This does not care about the order in which the bytes were loaded.
#[inline]
fn is_8digits(v: u64) -> bool {
    let a = v.wrapping_add(0x4646_4646_4646_4646);
    let b = v.wrapping_sub(0x3030_3030_3030_3030);
    (a | b) & 0x8080_8080_8080_8080 == 0
}

// Parse 8 digits, loaded as bytes in little-endian order.
//
// This uses the trick where every digit is in [0x030, 0x39],
// and therefore can be parsed in 3 multiplications, much
// faster than the normal 8.
//
// This is based off the algorithm described in "Fast numeric string to
// int", available here: https://johnnylee-sde.github.io/Fast-numeric-string-to-int.
#[inline]
fn parse_8digits(mut v: u64) -> u64 {
    const MASK: u64 = 0x0000_00FF_0000_00FF;
    const MUL1: u64 = 0x000F_4240_0000_0064;
    const MUL2: u64 = 0x0000_2710_0000_0001;

    v -= 0x3030_3030_3030_3030;
    v = (v * 10) + (v >> 8); // will not overflow, fits in 63 bits
    let v1 = (v & MASK).wrapping_mul(MUL1);
    let v2 = ((v >> 16) & MASK).wrapping_mul(MUL2);
    u64::from((v1.wrapping_add(v2) >> 32) as u32)
}

unsafe fn parse_digits_unsafe(bytes: &[u8], start: usize, count: usize) -> u32 {
    let mut d = 0u32;
    let mut i = 0;

    while i + 8 <= count {
        // SAFETY: `i + 8 <= count` ensures we have at least 8 bytes available.
        // `start + i` is within bounds since caller guarantees `start + count <= bytes.len()`.
        unsafe {
            let ptr = bytes.as_ptr().add(start + i);
            let mut tmp = [0u8; 8];
            std::ptr::copy_nonoverlapping(ptr, tmp.as_mut_ptr(), 8);
            let v = u64::from_le_bytes(tmp);

            if is_8digits(v) {
                d = d * 100_000_000 + parse_8digits(v) as u32;
                i += 8;
            } else {
                break;
            }
        }
    }

    while i < count {
        // SAFETY: `i < count` and `start + count <= bytes.len()`
        // ensures `start + i` is a valid index.
        let byte = unsafe { *bytes.get_unchecked(start + i) };
        let digit = byte.wrapping_sub(b'0');
        if digit < 10 {
            d = d * 10 + u32::from(digit);
            i += 1;
        } else {
            break;
        }
    }
    d
}

#[inline]
pub(crate) fn parse_digits(bytes: &[u8], start: usize, count: usize) -> u32 {
    debug_assert!(
        start
            .checked_add(count)
            .is_some_and(|end| end <= bytes.len())
    );
    // SAFETY: probably
    unsafe { parse_digits_unsafe(bytes, start, count) }
}
