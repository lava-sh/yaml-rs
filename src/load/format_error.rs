use saphyr_parser::ScanError;

#[inline]
unsafe fn push(buf: &mut String, byte: u8, count: usize) {
    // SAFETY: Only ASCII bytes are pushed, so UTF-8 stays valid.
    let bytes = unsafe { buf.as_mut_vec() };
    bytes.reserve(count);
    for _ in 0..count {
        bytes.push(byte);
    }
}

pub(crate) fn format_error(source: &str, error: &ScanError) -> String {
    let marker = error.marker();

    let line = marker.line();
    let col = marker.col() + 1;

    let mut line_buf = itoa::Buffer::new();
    let line_str = line_buf.format(line);

    let mut col_buf = itoa::Buffer::new();
    let col_str = col_buf.format(col);

    let gutter = line_str.len();
    let error_line = source.lines().nth(line - 1);
    let mut err = String::new();

    err.push_str("YAML parse error at line ");
    err.push_str(line_str);
    err.push_str(", column ");
    err.push_str(col_str);
    err.push('\n');

    if let Some(error_line) = error_line {
        unsafe {
            // SAFETY: We append only ASCII spaces and punctuation.
            push(&mut err, b' ', gutter);
            let bytes = err.as_mut_vec();
            bytes.extend_from_slice(b" |\n");
        }
        err.push_str(line_str);
        err.push_str(" | ");
        err.push_str(error_line);
        err.push('\n');

        unsafe {
            // SAFETY: We append only ASCII spaces and punctuation.
            push(&mut err, b' ', gutter);
            let bytes = err.as_mut_vec();
            bytes.extend_from_slice(b" |");
            push(&mut err, b' ', marker.col());
            let bytes = err.as_mut_vec();
            bytes.extend_from_slice(b" ^\n");
        }
    }

    err.push_str(error.info());
    err
}
