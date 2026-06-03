use crate::from_rust::num::repeat_u8;

// https://github.com/rust-lang/rust/blob/1.96.0/library/core/src/slice/memchr.rs#L6
const LO_USIZE: usize = repeat_u8(0x01);
// https://github.com/rust-lang/rust/blob/1.96.0/library/core/src/slice/memchr.rs#L7
const HI_USIZE: usize = repeat_u8(0x80);

// https://github.com/rust-lang/rust/blob/1.96.0/library/core/src/slice/memchr.rs#L10-L20
#[inline]
pub const fn contains_zero_byte(x: usize) -> bool {
    x.wrapping_sub(LO_USIZE) & !x & HI_USIZE != 0
}
