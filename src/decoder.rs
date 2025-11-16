use std::io::{Error, ErrorKind};

use encoding::{DecoderTrap, label::encoding_from_whatwg_label};

#[inline]
pub fn encode(
    data: &[u8],
    encoding: Option<&str>,
    encoder_errors: Option<&str>,
) -> Result<String, Error> {
    let decoder = if let Some(encoding) = encoding {
        if let Some(encoder) = encoding_from_whatwg_label(encoding) {
            Some(encoder)
        } else {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "Invalid encoder {encoding}. For valid encoders see https://encoding.spec.whatwg.org/#concept-encoding-get"
                ),
            ));
        }
    } else {
        if cfg!(target_family = "windows") {
            encoding_from_whatwg_label("windows-1252")
        } else {
            encoding_from_whatwg_label("utf-8")
        }
    };
    let decoder_trap = if let Some(encoder_errors) = encoder_errors {
        Some(match encoder_errors {
            "strict" => DecoderTrap::Strict,
            "ignore" => DecoderTrap::Ignore,
            "replace" => DecoderTrap::Replace,
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!("Invalid decoder error handling: {encoder_errors}"),
                ));
            }
        })
    } else {
        Some(DecoderTrap::Ignore)
    };
    Ok(if let Some(encoder) = decoder {
        encoder
            .decode(data, *decoder_trap.as_ref().unwrap())
            .map_err(|e| {
                Error::new(
                    ErrorKind::InvalidInput,
                    format!("Invalid UTF8 character: {e}"),
                )
            })?
    } else if encoder_errors.is_none() || encoder_errors == Some("ignore") {
        String::from_utf8_lossy(data).to_string()
    } else {
        std::str::from_utf8(data)
            .map_err(|e| {
                Error::new(
                    ErrorKind::InvalidInput,
                    format!("Invalid UTF8 character: {e}"),
                )
            })?
            .to_string()
    })
}
