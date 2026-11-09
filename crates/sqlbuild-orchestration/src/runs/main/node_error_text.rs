//! A failed node's error text, redacted before it is cut so a cut can never expose part of a secret.

use std::sync::LazyLock;

use regex::{Captures, Regex};
use sqlbuild_core::redaction::main::redact_text::redact_text;

use crate::runs::constants::{BARE_USERINFO_PATTERN, NODE_ERROR_CHARS};

static BARE_USERINFO: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(BARE_USERINFO_PATTERN).ok());

/// Named secrets and URI passwords redacted, even where a warehouse cut the URI before its host, then at most `NODE_ERROR_CHARS` characters; a second pass changes nothing.
#[must_use]
pub fn node_error_text(text: &str) -> String {
    let mut redacted = redact_text(text);
    if let Some(pattern) = BARE_USERINFO.as_ref() {
        redacted = pattern
            .replace_all(&redacted, |found: &Captures<'_>| {
                if found[2].bytes().all(|byte| byte.is_ascii_digit()) {
                    found[0].to_owned()
                } else {
                    format!("{}[REDACTED]", &found[1])
                }
            })
            .into_owned();
    }
    if redacted.chars().count() <= NODE_ERROR_CHARS {
        return redacted;
    }
    let mut kept: String = redacted.chars().take(NODE_ERROR_CHARS - 1).collect();
    kept.push('…');
    kept
}
