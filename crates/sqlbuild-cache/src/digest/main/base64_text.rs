//! Standard padded base64 text of bytes, as Python's `base64.b64encode(data).decode()` gives.

use crate::digest::constants::BASE64_ALPHABET;

/// Standard padded base64 of `data`.
#[must_use]
pub fn base64_text(data: &[u8]) -> String {
    let mut encoded = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let bytes = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let triple = u32::from(bytes[0]) << 16 | u32::from(bytes[1]) << 8 | u32::from(bytes[2]);
        for index in 0..4 {
            if index <= chunk.len() {
                encoded.push(char::from(
                    BASE64_ALPHABET[(triple >> (18 - 6 * index) & 63) as usize],
                ));
            } else {
                encoded.push('=');
            }
        }
    }
    encoded
}
