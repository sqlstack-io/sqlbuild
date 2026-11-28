//! The few words a `Failed (N):` line gives for why an item failed.

use crate::run_output::constants::{ENGINE_ERROR_MARK, SHORT_REASON_WIDTH};

/// The engine's error kind, such as `Binder Error`, else the message's first clause, cut to fit.
#[must_use]
pub fn short_reason(code: Option<&str>, message: &str) -> String {
    let first: &str = message.lines().next().unwrap_or_default().trim();
    if let Some(at) = first.find(ENGINE_ERROR_MARK) {
        let kind: &str = first[..at].rsplit(' ').next().unwrap_or_default();
        if !kind.is_empty() {
            return format!("{kind} Error");
        }
    }
    let clause: &str = first
        .split(". ")
        .next()
        .unwrap_or(first)
        .trim_end_matches('.');
    let text: String = match code {
        Some(code) if !code.is_empty() => format!("[{code}] {clause}"),
        _ => clause.to_owned(),
    };
    if text.chars().count() <= SHORT_REASON_WIDTH {
        return text;
    }
    let kept: String = text.chars().take(SHORT_REASON_WIDTH - 3).collect();
    format!("{}...", kept.trim_end())
}
