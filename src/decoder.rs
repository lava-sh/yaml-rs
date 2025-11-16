use std::io::{Error, ErrorKind};

use encoding::{DecoderTrap, label::encoding_from_whatwg_label};

pub fn encode(
    data: &[u8],
    encoding: Option<&str>,
    encoder_errors: Option<&str>,
) -> Result<String, Error> {
    let is_utf8 = matches!(encoding, None | Some("utf-8") | Some("UTF-8"));

    if is_utf8 {
        return match encoder_errors {
            None | Some("ignore") | Some("replace") => {
                Ok(String::from_utf8_lossy(data).into_owned())
            }
            Some("strict") => {
                // SAFETY: `data` has been validated as UTF-8 by `from_utf8` above.
                match std::str::from_utf8(data) {
                    Ok(_) => unsafe { Ok(String::from_utf8_unchecked(data.to_vec())) },
                    Err(err) => Err(Error::new(
                        ErrorKind::InvalidInput,
                        format!("failed to encode bytes: {err}"),
                    )),
                }
            }
            Some(other) => Err(Error::new(
                ErrorKind::InvalidInput,
                format!("invalid decoder: {other}"),
            )),
        };
    }

    // Choose windows-1252 as default encoding on Windows platforms and utf-8 on all other platforms.
    let encoding_label = encoding.unwrap_or(if cfg!(target_family = "windows") {
        "windows-1252"
    } else {
        "utf-8"
    });

    let decoder_trap = match encoder_errors {
        Some("strict") => DecoderTrap::Strict,
        Some("ignore") => DecoderTrap::Ignore,
        Some("replace") => DecoderTrap::Replace,
        Some(other) => {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("invalid decoder: {other}"),
            ));
        }
        None => DecoderTrap::Ignore,
    };

    let decoder = encoding_from_whatwg_label(encoding_label).ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidData,
            format!("invalid encoding: {encoding_label}"),
        )
    })?;

    decoder
        .decode(data, decoder_trap)
        .map_err(|e| Error::new(ErrorKind::InvalidInput, format!("decoding error: {e}")))
}
