use std::{
    borrow::Cow,
    io::{Error, ErrorKind},
};

use simdutf8::basic::from_utf8;

pub fn encode<'a>(
    data: &'a [u8],
    encoding: Option<&str>,
    encoder_errors: Option<&str>,
) -> Result<Cow<'a, str>, Error> {
    let is_utf8 = matches!(encoding, None | Some("utf-8" | "UTF-8"));

    if is_utf8 {
        return match encoder_errors {
            None | Some("ignore" | "replace") => match from_utf8(data) {
                Ok(s) => Ok(Cow::Borrowed(s)),
                Err(_) => Ok(String::from_utf8_lossy(data)),
            },
            Some("strict") => from_utf8(data).map(Cow::Borrowed).map_err(|err| {
                Error::new(
                    ErrorKind::InvalidInput,
                    format!("failed to encode bytes: {err}"),
                )
            }),
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

    let encoding_comp = match encoding_label {
        "shift_jis" | "shift-jis" | "sjis" => encoding_rs::SHIFT_JIS,
        "big5" => encoding_rs::BIG5,
        "gbk" | "gb18030" => encoding_rs::GBK,
        "euc-kr" | "euckr" => encoding_rs::EUC_KR,
        "iso-2022-jp" => encoding_rs::ISO_2022_JP,
        "windows-1252" | "cp1252" | "iso-8859-1" | "latin1" => encoding_rs::WINDOWS_1252,
        "windows-1251" => encoding_rs::WINDOWS_1251,
        "windows-1250" => encoding_rs::WINDOWS_1250,
        "iso-8859-2" => encoding_rs::ISO_8859_2,
        "iso-8859-5" => encoding_rs::ISO_8859_5,
        "iso-8859-6" => encoding_rs::ISO_8859_6,
        "iso-8859-7" => encoding_rs::ISO_8859_7,
        "iso-8859-8" => encoding_rs::ISO_8859_8,
        "euc-jp" | "eucjp" => encoding_rs::EUC_JP,
        _ => encoding_rs::Encoding::for_label(encoding_label.as_bytes()).ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidData,
                format!("invalid encoding: {encoding_label}"),
            )
        })?,
    };

    let cow = match encoder_errors {
        Some("strict") => encoding_comp
            .decode_without_bom_handling_and_without_replacement(data)
            .ok_or_else(|| {
                Error::new(ErrorKind::InvalidInput, "decoding error: malformed input")
            })?,
        Some("ignore" | "replace") | None => encoding_comp.decode_without_bom_handling(data).0,
        Some(other) => {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("invalid decoder: {other}"),
            ));
        }
    };
    Ok(cow)
}
