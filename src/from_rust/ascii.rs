use crate::from_rust::{memchr::contains_zero_byte, num::repeat_u8};

// Adapted to search for '.' character
// https://github.com/rust-lang/rust/blob/1.95.0/library/core/src/slice/ascii.rs#L428-L539
#[inline]
#[allow(clippy::ptr_as_ptr, clippy::cast_ptr_alignment)]
pub(crate) fn is_dot(s: &[u8]) -> bool {
    /// Returns `true` if any byte in the word `v` is a dot ('.').
    const fn contains_dot(v: usize) -> bool {
        const DOT_MASK: usize = repeat_u8(b'.');
        (v ^ DOT_MASK) == 0 || contains_zero_byte(v ^ DOT_MASK)
    }

    const USIZE_SIZE: usize = size_of::<usize>();

    let len = s.len();
    let align_offset = s.as_ptr().align_offset(USIZE_SIZE);

    // If we wouldn't gain anything from the word-at-a-time implementation, fall
    // back to a scalar loop.
    //
    // We also do this for architectures where `size_of::<usize>()` isn't
    // sufficient alignment for `usize`, because it's a weird edge case.
    if len < USIZE_SIZE || len < align_offset || USIZE_SIZE < align_of::<usize>() {
        return s.contains(&b'.');
    }

    // We always read the first word unaligned, which means `align_offset` is
    // 0, we'd read the same value again for the aligned read.
    let offset_to_aligned = if align_offset == 0 {
        USIZE_SIZE
    } else {
        align_offset
    };

    let start = s.as_ptr();

    // SAFETY: We verify `len < USIZE_SIZE` above.
    let first_word = unsafe { (start as *const usize).read_unaligned() };

    if contains_dot(first_word) {
        return true;
    }

    // We checked this above, somewhat implicitly. Note that `offset_to_aligned`
    // is either `align_offset` or `USIZE_SIZE`, both of are explicitly checked
    // above.
    debug_assert!(offset_to_aligned <= len);

    // SAFETY: word_ptr is the (properly aligned) usize ptr we use to read the
    // middle chunk of the slice.
    let mut word_ptr = unsafe { start.add(offset_to_aligned) as *const usize };

    // `byte_pos` is the byte index of `word_ptr`, used for loop end checks.
    let mut byte_pos = offset_to_aligned;

    // Paranoia check about alignment, since we're about to do a bunch of
    // unaligned loads. In practice this should be impossible barring a bug in
    // `align_offset` though.
    // While this method is allowed to spuriously fail in CTFE, if it doesn't
    // have alignment information it should have given a `usize::MAX` for
    // `align_offset` earlier, sending things through the scalar path instead of
    // this one, so this check should pass if it's reachable.
    debug_assert!(word_ptr.is_aligned_to(align_of::<usize>()));

    // Read subsequent words until the last aligned word, excluding the last
    // aligned word by itself to be done in tail check later, to ensure that
    // tail is always one `usize` at most to extra branch `byte_pos == len`.
    while byte_pos < len - USIZE_SIZE {
        // Sanity check that the read is in bounds
        debug_assert!(byte_pos + USIZE_SIZE <= len);
        // And that our assumptions about `byte_pos` hold.
        debug_assert!(word_ptr.cast::<u8>() == start.wrapping_add(byte_pos));

        // SAFETY: We know `word_ptr` is properly aligned (because of
        // `align_offset`), and we know that we have enough bytes between `word_ptr` and the end
        let word = unsafe { word_ptr.read() };
        if contains_dot(word) {
            return true;
        }

        byte_pos += USIZE_SIZE;

        // SAFETY: We know that `byte_pos <= len - USIZE_SIZE`, which means that
        // after this `add`, `word_ptr` will be at most one-past-the-end.
        word_ptr = unsafe { word_ptr.add(1) };
    }

    // Sanity check to ensure there really is only one `usize` left. This should
    // be guaranteed by our loop condition.
    debug_assert!(byte_pos <= len && len - byte_pos <= USIZE_SIZE);

    // SAFETY: This relies on `len >= USIZE_SIZE`, which we check at the start.
    let last_word = unsafe { (start.add(len - USIZE_SIZE) as *const usize).read_unaligned() };

    contains_dot(last_word)
}
