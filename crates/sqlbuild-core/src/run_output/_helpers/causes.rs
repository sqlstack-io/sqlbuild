//! A short list of causes: `fn:HASH 3, project function 1, +2 more`.

use crate::run_output::constants::HEADER_CAUSE_LIMIT;
use crate::run_output::models::HeaderCause;

/// The first causes as `<label> <count>`, then `+N more` for the rest.
pub(crate) fn cause_list(causes: &[HeaderCause]) -> String {
    let mut parts: Vec<String> = causes
        .iter()
        .take(HEADER_CAUSE_LIMIT)
        .map(|cause| format!("{} {}", cause.label, cause.count))
        .collect();
    if causes.len() > HEADER_CAUSE_LIMIT {
        parts.push(format!("+{} more", causes.len() - HEADER_CAUSE_LIMIT));
    }
    parts.join(", ")
}
