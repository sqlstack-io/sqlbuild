//! `redact_error_text`: error text redacted before it is cut, so a cut can never expose part of a secret.

use crate::redaction::_helpers::patterns::error_redacted;
use crate::redaction::_helpers::readings::redacted_readings;
use crate::redaction::constants::ERROR_TEXT_CHARS;

/// Named secrets and URI passwords redacted, even where a warehouse cut the URI before its host, then at most 2,000 characters; a second pass changes nothing.
#[must_use]
pub fn redact_error_text(text: &str) -> String {
    let redacted = redacted_readings(text, error_redacted);
    if redacted.chars().count() <= ERROR_TEXT_CHARS {
        return redacted;
    }
    let mut kept: String = redacted.chars().take(ERROR_TEXT_CHARS - 1).collect();
    kept.push('…');
    kept
}
