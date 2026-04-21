use crate::from_rust::num::repeat_u8;

const LO_USIZE: usize = repeat_u8(0x01);
const HI_USIZE: usize = repeat_u8(0x80);

// https://github.com/rust-lang/rust/blob/1.95.0/library/core/src/slice/memchr.rs#L17-L20
#[inline]
pub(crate) const fn contains_zero_byte(x: usize) -> bool {
    x.wrapping_sub(LO_USIZE) & !x & HI_USIZE != 0
}
