//! Decode immutable compressed control-store bodies.

use std::io::Read;

use flate2::read::GzDecoder;
use serde_json::Value;

use crate::store::errors::ControlError;

pub(crate) fn decompress(value: &Value) -> Result<Vec<u8>, ControlError> {
    let bytes: Vec<u8> = serde_json::from_value(value.clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let mut output: Vec<u8> = Vec::new();
    GzDecoder::new(bytes.as_slice()).read_to_end(&mut output)?;
    Ok(output)
}
