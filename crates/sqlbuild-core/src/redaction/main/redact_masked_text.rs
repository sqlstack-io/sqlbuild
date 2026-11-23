//! `redact_masked_text`: exact masking and pattern redaction applied to text as one step.

use crate::redaction::_helpers::patterns::named_redacted;
use crate::redaction::_helpers::readings::redacted_readings;

/// `redact_text` after `mask`, with `mask` also applied to both readings of text with NUL.
#[must_use]
pub fn redact_masked_text(text: &str, mask: impl Fn(&str) -> String) -> String {
    redacted_readings(text, |reading| named_redacted(&mask(reading)))
}
