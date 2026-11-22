//! `redact_text`: obvious named secrets and URI passwords removed from diagnostic text.

use crate::redaction::_helpers::patterns::named_redacted;
use crate::redaction::_helpers::readings::redacted_readings;

/// Authorization headers, bearer tokens, `name=value` secrets, SQL passwords, then URI passwords redacted, in both readings of text with NUL.
#[must_use]
pub fn redact_text(value: &str) -> String {
    redacted_readings(value, named_redacted)
}
