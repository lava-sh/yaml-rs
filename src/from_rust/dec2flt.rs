// https://github.com/rust-lang/rust/blob/1.96.0/library/core/src/num/imp/dec2flt/common.rs#L56-L64
#[inline]
pub fn is_8digits(v: u64) -> bool {
    let a = v.wrapping_add(0x4646_4646_4646_4646);
    let b = v.wrapping_sub(0x3030_3030_3030_3030);
    (a | b) & 0x8080_8080_8080_8080 == 0
}
