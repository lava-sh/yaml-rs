use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

#[inline]
fn current_digit_to_value(byte: u8, radix: u32) -> Option<u64> {
    let value = match byte {
        b'0'..=b'9' => u64::from(byte - b'0'),
        b'a'..=b'f' => u64::from(byte - b'a' + 10),
        b'A'..=b'F' => u64::from(byte - b'A' + 10),
        _ => return None,
    };
    if value < radix as u64 { Some(value) } else { None }
}

#[inline]
fn current_parse_i64(bytes: &[u8], radix: u32, neg: bool) -> Option<i64> {
    let mut acc = 0u64;
    let mut has_digit = false;
    let limit = if neg { i64::MIN.unsigned_abs() } else { i64::MAX as u64 };

    for &b in bytes {
        if b == b'_' { continue; }
        let d = current_digit_to_value(b, radix)?;
        has_digit = true;
        acc = acc.checked_mul(radix as u64)?;
        acc = acc.checked_add(d)?;
        if acc > limit { return None; }
    }

    if !has_digit { return None; }

    if neg {
        if acc == i64::MIN.unsigned_abs() { Some(i64::MIN) }
        else { Some(-(acc as i64)) }
    } else {
        Some(acc as i64)
    }
}

static LUT: [u8; 256] = {
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
    let limit = if neg { i64::MIN.unsigned_abs() } else { i64::MAX as u64 };

    for &b in bytes {
        if b == b'_' { continue; }
        let d = LUT[b as usize];
        if d == 0xFF || d >= radix as u8 { return None; }
        has_digit = true;
        acc = acc.checked_mul(radix as u64)?.checked_add(d as u64)?;
        if acc > limit { return None; }
    }

    if !has_digit { return None; }

    if neg {
        if acc == i64::MIN.unsigned_abs() { Some(i64::MIN) }
        else { Some(-(acc as i64)) }
    } else {
        Some(acc as i64)
    }
}

#[inline]
fn fast_decimal(bytes: &[u8], neg: bool) -> Option<i64> {
    let mut acc = 0u64;
    let mut has_digit = false;
    let limit = if neg { i64::MIN.unsigned_abs() } else { i64::MAX as u64 };

    for &b in bytes {
        if b == b'_' { continue; }
        if !b.is_ascii_digit() { return None; }
        let d = (b - b'0') as u64;
        has_digit = true;
        if acc > (limit - d) / 10 { return None; }
        acc = acc * 10 + d;
    }

    if !has_digit { return None; }

    if neg {
        if acc == i64::MIN.unsigned_abs() { Some(i64::MIN) }
        else { Some(-(acc as i64)) }
    } else {
        Some(acc as i64)
    }
}

#[inline]
fn fast_decimal_unrolled(bytes: &[u8], neg: bool) -> Option<i64> {
    let mut acc = 0u64;
    let mut has_digit = false;
    let limit = if neg { i64::MIN.unsigned_abs() } else { i64::MAX as u64 };

    let mut i = 0;

    while i + 2 <= bytes.len() {
        let b0 = bytes[i];
        let b1 = bytes[i + 1];

        if b0 == b'_' { i += 1; continue; }

        if b1 == b'_' {
            if !b0.is_ascii_digit() { return None; }
            let d = (b0 - b'0') as u64;
            if acc > (limit - d) / 10 { return None; }
            acc = acc * 10 + d;
            has_digit = true;
            i += 2;
            continue;
        }

        if !b0.is_ascii_digit() || !b1.is_ascii_digit() { break; }

        let d0 = (b0 - b'0') as u64;
        let d1 = (b1 - b'0') as u64;

        if acc > (limit - d0) / 10 { return None; }
        acc = acc * 10 + d0;

        if acc > (limit - d1) / 10 { return None; }
        acc = acc * 10 + d1;

        has_digit = true;
        i += 2;
    }

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'_' { i += 1; continue; }
        if !b.is_ascii_digit() { return None; }
        let d = (b - b'0') as u64;
        if acc > (limit - d) / 10 { return None; }
        acc = acc * 10 + d;
        has_digit = true;
        i += 1;
    }

    if !has_digit { return None; }

    if neg {
        if acc == i64::MIN.unsigned_abs() { Some(i64::MIN) }
        else { Some(-(acc as i64)) }
    } else {
        Some(acc as i64)
    }
}

#[inline]
unsafe fn fast_decimal_unsafe(bytes: &[u8], neg: bool) -> Option<i64> {
    let mut acc = 0u64;
    let mut has_digit = false;
    let limit = if neg { i64::MIN.unsigned_abs() } else { i64::MAX as u64 };
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        let b = *bytes.get_unchecked(i);
        i += 1;

        if b == b'_' { continue; }
        if b < b'0' || b > b'9' { return None; }

        let d = (b - b'0') as u64;
        has_digit = true;

        acc = acc.wrapping_mul(10).wrapping_add(d);
        if acc > limit { return None; }
    }

    if !has_digit { return None; }

    if neg {
        if acc == i64::MIN.unsigned_abs() { Some(i64::MIN) }
        else { Some(-(acc as i64)) }
    } else {
        Some(acc as i64)
    }
}

#[inline]
unsafe fn fast_decimal_ptr(bytes: &[u8], neg: bool) -> Option<i64> {
    let mut acc = 0u64;
    let mut has_digit = false;
    let limit = if neg { i64::MIN.unsigned_abs() } else { i64::MAX as u64 };

    let mut ptr = bytes.as_ptr();
    let end = ptr.add(bytes.len());

    while ptr < end {
        let b = *ptr;
        ptr = ptr.add(1);

        if b == b'_' { continue; }
        if b < b'0' || b > b'9' { return None; }

        let d = (b - b'0') as u64;
        has_digit = true;

        acc = acc.wrapping_mul(10).wrapping_add(d);
        if acc > limit { return None; }
    }

    if !has_digit { return None; }

    if neg {
        if acc == i64::MIN.unsigned_abs() { Some(i64::MIN) }
        else { Some(-(acc as i64)) }
    } else {
        Some(acc as i64)
    }
}

fn bench_parse_i64(c: &mut Criterion) {
    let inputs = [
        (b"0".as_slice(), 10, false),
        (b"1".as_slice(), 10, false),
        (b"9".as_slice(), 10, false),
        (b"10".as_slice(), 10, false),
        (b"123".as_slice(), 10, false),
        (b"9999".as_slice(), 10, false),
        (b"1234567890".as_slice(), 10, false),
        (b"9223372036854775807".as_slice(), 10, false),
        (b"9223372036854775808".as_slice(), 10, true),
        (b"18446744073709551615".as_slice(), 10, false),
        (b"1_000".as_slice(), 10, false),
        (b"9_223_372_036_854_775".as_slice(), 10, false),
        (b"1_2_3_4_5_6".as_slice(), 10, false),
        (b"0_0_0_1".as_slice(), 10, false),
        (b"deadbeef".as_slice(), 16, false),
        (b"dead_beef".as_slice(), 16, false),
        (b"ff".as_slice(), 16, false),
        (b"ff_ff_ff_ff".as_slice(), 16, false),
        (b"7fffffff".as_slice(), 16, false),
        (b"7fff_ffff".as_slice(), 16, true),
        (b"ABCDEF".as_slice(), 16, false),
        (b"abcdef".as_slice(), 16, false),
        (b"101010".as_slice(), 2, false),
        (b"1111_0000".as_slice(), 2, false),
        (b"zz".as_slice(), 36, false),
        (b"1z".as_slice(), 36, false),
        (b"invalid".as_slice(), 10, false),
        (b"123x".as_slice(), 10, false),
        (b"_123".as_slice(), 10, false),
        (b"".as_slice(), 10, false),
    ];

    c.bench_function("current", |b| {
        b.iter(|| {
            for &(bytes, r, n) in &inputs {
                black_box(current_parse_i64(black_box(bytes), r, n));
            }
        })
    });

    c.bench_function("lut", |b| {
        b.iter(|| {
            for &(bytes, r, n) in &inputs {
                black_box(parse_i64(black_box(bytes), r, n));
            }
        })
    });

    c.bench_function("fast_decimal", |b| {
        b.iter(|| {
            for &(bytes, r, n) in &inputs {
                if r == 10 {
                    black_box(fast_decimal(black_box(bytes), n));
                }
            }
        })
    });

    c.bench_function("fast_decimal_unrolled", |b| {
        b.iter(|| {
            for &(bytes, r, n) in &inputs {
                if r == 10 {
                    black_box(fast_decimal_unrolled(black_box(bytes), n));
                }
            }
        })
    });

    c.bench_function("fast_decimal_unsafe", |b| {
        b.iter(|| {
            for &(bytes, r, n) in &inputs {
                if r == 10 {
                    unsafe {
                        black_box(fast_decimal_unsafe(black_box(bytes), n));
                    }
                }
            }
        })
    });

    c.bench_function("fast_decimal_ptr", |b| {
        b.iter(|| {
            for &(bytes, r, n) in &inputs {
                if r == 10 {
                    unsafe {
                        black_box(fast_decimal_ptr(black_box(bytes), n));
                    }
                }
            }
        })
    });
}

criterion_group!(benches, bench_parse_i64);
criterion_main!(benches);