//! The forms a secret value takes in text: as it is, percent-encoded in a URL, and escaped in JSON.

use std::collections::BTreeSet;
use std::fmt::Write;

/// `secret`, its percent-encodings (every reserved byte, or all but `/`), and its JSON-escaped body.
pub(crate) fn secret_forms(secret: &str) -> BTreeSet<String> {
    let mut forms: BTreeSet<String> = BTreeSet::from([secret.to_owned()]);
    forms.insert(percent_encoded(secret, b""));
    forms.insert(percent_encoded(secret, b"/"));
    let escaped = serde_json::to_string(secret).unwrap_or_default();
    forms.insert(
        escaped
            .trim_start_matches('"')
            .trim_end_matches('"')
            .to_owned(),
    );
    forms.retain(|form| !form.is_empty());
    forms
}

/// `text` with every byte but unreserved characters and `safe` as `%XX`, as `urllib.parse.quote` writes it.
fn percent_encoded(text: &str, safe: &[u8]) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) || safe.contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}
