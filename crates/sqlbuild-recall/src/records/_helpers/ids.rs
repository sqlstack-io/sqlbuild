//! Deterministic record IDs and content digests.

use crate::records::constants::{
    CONTENT_HASH_BUFFER_BYTES, CONTENT_HASH_HEX, DIAGNOSTIC_RECORD_ID_INFIX, OUTPUT_RECORD_ID_INFIX,
};
use crate::records::models::RecallValue;
use crate::schema::constants::UNHASHED_COLUMNS;

/// `<invocation_id>:command-output:<sequence>`, the command-output sink record ID scheme.
pub(crate) fn output_record_id(invocation_id: &str, sequence: u64) -> String {
    format!("{invocation_id}:{OUTPUT_RECORD_ID_INFIX}:{sequence}")
}

/// `<invocation_id>:diagnostic:<sequence>`.
pub(crate) fn diagnostic_record_id(invocation_id: &str, sequence: u64) -> String {
    format!("{invocation_id}:{DIAGNOSTIC_RECORD_ID_INFIX}:{sequence}")
}

fn framed(mut buffer: Vec<u8>, tag: u8, bytes: &[u8]) -> Vec<u8> {
    buffer.push(tag);
    buffer.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    buffer.extend_from_slice(bytes);
    buffer
}

fn valued(mut buffer: Vec<u8>, value: &RecallValue) -> Vec<u8> {
    match value {
        RecallValue::Null => buffer.push(b'0'),
        RecallValue::Integer(number) => {
            buffer.push(b'i');
            buffer.extend_from_slice(&number.to_le_bytes());
        }
        RecallValue::Real(number) => {
            buffer.push(b'r');
            buffer.extend_from_slice(&number.to_bits().to_le_bytes());
        }
        RecallValue::Text(text) => return framed(buffer, b't', text.as_bytes()),
        RecallValue::Blob(data) => return framed(buffer, b'b', data),
    }
    buffer
}

/// The first 128 bits of a hex BLAKE3 over every non-NULL named value except `ingested_at` and `content_hash`, by column name; a NULL column hashes like an absent one.
pub(crate) fn content_hash(values: &[(&str, RecallValue)]) -> String {
    let mut sorted: Vec<&(&str, RecallValue)> = values
        .iter()
        .filter(|(name, value)| {
            !UNHASHED_COLUMNS.contains(name) && !matches!(value, RecallValue::Null)
        })
        .collect();
    sorted.sort_by(|left, right| left.0.cmp(right.0));
    let encoded: Vec<u8> = sorted.into_iter().fold(
        Vec::with_capacity(CONTENT_HASH_BUFFER_BYTES),
        |buffer, (name, value)| valued(framed(buffer, b'n', name.as_bytes()), value),
    );
    blake3::hash(&encoded).to_hex()[..CONTENT_HASH_HEX].to_owned()
}
