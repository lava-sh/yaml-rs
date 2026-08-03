// https://github.com/rust-lang/rust/blob/1.97.1/library/core/src/num/mod.rs#L1478-L1482
#[inline]
pub const fn repeat_u8(x: u8) -> usize {
    usize::from_ne_bytes([x; size_of::<usize>()])
}
