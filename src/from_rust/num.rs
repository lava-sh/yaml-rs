// https://github.com/rust-lang/rust/blob/1.95.0/library/core/src/num/mod.rs#L1431-L1434
#[inline]
pub(crate) const fn repeat_u8(x: u8) -> usize {
    usize::from_ne_bytes([x; size_of::<usize>()])
}
