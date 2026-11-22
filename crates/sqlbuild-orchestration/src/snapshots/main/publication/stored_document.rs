//! A snapshot document as it is stored: its content hash and gzip body.

use serde_json::Value;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

use crate::snapshots::_helpers::content::json_bytes;
use crate::store::errors::ControlError;
use crate::store::main::compress::compress;

/// The hash `sqb_documents` keys `document` by, and its compressed body.
pub(crate) fn stored_document(document: &Value) -> Result<(String, Vec<u8>), ControlError> {
    let bytes = json_bytes(document)?;
    Ok((sha256_hex(&bytes), compress(&bytes)?))
}
