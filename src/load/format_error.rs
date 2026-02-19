use saphyr_parser::ScanError;

pub(crate) fn format_error(source: &str, error: &ScanError) -> String {
    let marker = error.marker();
    let line = marker.line();
    let col = marker.col() + 1;
    let gutter = line.to_string().len();

    let error_len = error.info().len();
    let base_len = 50;
    let line_len = itoa::Buffer::new().format(line).len();
    let col_len = itoa::Buffer::new().format(col).len();

    let error_line = source.lines().nth(line - 1);

    let total_len = base_len
        + line_len
        + col_len
        + error_len
        + if let Some(error_line) = error_line {
            gutter + 3 + line_len + 3 + error_line.len() + 1 + gutter + 2 + marker.col() + 3 + 1
        } else {
            0
        };

    let mut err = String::with_capacity(total_len);

    err.push_str("YAML parse error at line ");
    err.push_str(itoa::Buffer::new().format(line));
    err.push_str(", column ");
    err.push_str(itoa::Buffer::new().format(col));
    err.push('\n');

    if let Some(error_line) = error_line {
        unsafe {
            // SAFETY: We only push valid ASCII bytes (spaces, '|', '\n') to the Vec<u8>.
            // String's UTF-8 invariant is maintained because all bytes are valid UTF-8.
            let bytes = err.as_mut_vec();
            bytes.reserve(gutter + 3);
            for _ in 0..gutter {
                bytes.push(b' ');
            }
            bytes.push(b' ');
            bytes.push(b'|');
            bytes.push(b'\n');
        }
        err.push_str(itoa::Buffer::new().format(line));
        err.push_str(" | ");
        err.push_str(error_line);
        err.push('\n');
        unsafe {
            // SAFETY: We only push valid ASCII bytes (spaces, '|', '^', '\n') to the Vec<u8>.
            // All ASCII bytes are valid UTF-8, so String's invariant is preserved.
            let bytes = err.as_mut_vec();
            let spaces = gutter + 2 + marker.col();
            bytes.reserve(spaces + 3);

            for _ in 0..gutter {
                bytes.push(b' ');
            }
            bytes.push(b' ');
            bytes.push(b'|');
            for _ in 0..marker.col() {
                bytes.push(b' ');
            }
            bytes.push(b' ');
            bytes.push(b'^');
            bytes.push(b'\n');
        }
    }
    err.push_str(error.info());
    err
}
