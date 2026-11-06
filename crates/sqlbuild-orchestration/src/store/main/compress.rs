//! Shared encoding for immutable, content-addressed control-store bodies.

use std::io::Write;

use flate2::{Compression, write::GzEncoder};

use crate::store::errors::ControlError;

pub(crate) fn compress(bytes: &[u8]) -> Result<Vec<u8>, ControlError> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes)?;
    Ok(encoder.finish()?)
}
