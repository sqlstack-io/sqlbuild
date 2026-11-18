//! Decode immutable compressed control-store bodies.

use std::io::Read;

use flate2::read::GzDecoder;
use serde_json::Value;

use crate::store::errors::ControlError;

/// A compressed body read as a blob, or as hexadecimal text, which a store returns far faster for large reads.
pub(crate) fn decompress(value: &Value) -> Result<Vec<u8>, ControlError> {
    let invalid =
        || ControlError::Invalid("stored body is neither bytes nor hexadecimal text".into());
    let bytes: Vec<u8> = match value {
        Value::Array(items) => items
            .iter()
            .map(|item| {
                item.as_u64()
                    .ok_or_else(invalid)
                    .and_then(|byte| u8::try_from(byte).map_err(|_| invalid()))
            })
            .collect::<Result<Vec<u8>, ControlError>>()?,
        Value::String(text) => {
            let (pairs, rest) = text.as_bytes().as_chunks::<2>();
            if !rest.is_empty() {
                return Err(invalid());
            }
            let digit = |byte: u8| char::from(byte).to_digit(16).ok_or_else(invalid);
            pairs
                .iter()
                .map(|[high, low]| {
                    u8::try_from(digit(*high)? * 16 + digit(*low)?).map_err(|_| invalid())
                })
                .collect::<Result<Vec<u8>, ControlError>>()?
        }
        other => serde_json::from_value(other.clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?,
    };
    let mut output: Vec<u8> = Vec::new();
    GzDecoder::new(bytes.as_slice()).read_to_end(&mut output)?;
    Ok(output)
}
