//! `failure_reason`: a raised error's class and message on one line, redacted as warehouse error text.

use pyo3::PyErr;
use sqlbuild_core::redaction::main::redact_error_text::redact_error_text;

/// The driver's class and message on one line, redacted as warehouse error text.
#[must_use]
pub fn failure_reason(error: &PyErr) -> String {
    redact_error_text(&error.to_string())
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
