//! Decode immutable compressed control-store bodies.

use std::io::Read;

use flate2::read::GzDecoder;
use serde_json::Value;

use crate::store::errors::ControlError;

/// A compressed body selected as hexadecimal text with `hex_column`; bytes are refused, since the store returns them as one JSON number per byte.
pub(crate) fn decompress(value: &Value) -> Result<Vec<u8>, ControlError> {
    let Value::String(text) = value else {
        return Err(ControlError::Invalid(
            "a stored body must be selected as hexadecimal text, not bytes".into(),
        ));
    };
    let invalid = || ControlError::Invalid("stored body is not hexadecimal text".into());
    let (pairs, rest) = text.as_bytes().as_chunks::<2>();
    if !rest.is_empty() {
        return Err(invalid());
    }
    let digit = |byte: u8| char::from(byte).to_digit(16).ok_or_else(invalid);
    let bytes: Vec<u8> = pairs
        .iter()
        .map(|[high, low]| u8::try_from(digit(*high)? * 16 + digit(*low)?).map_err(|_| invalid()))
        .collect::<Result<Vec<u8>, ControlError>>()?;
    let mut output: Vec<u8> = Vec::new();
    GzDecoder::new(bytes.as_slice()).read_to_end(&mut output)?;
    Ok(output)
}
