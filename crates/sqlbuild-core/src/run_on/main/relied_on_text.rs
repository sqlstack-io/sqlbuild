//! What a local test relied on from the project, as its result row and `--inspect` say.

use crate::run_on::constants::RELIED_ON_LABEL;

/// `relied on: HASH (allowed), TO_DATE (override)`; `None` when it relied on nothing.
#[must_use]
pub fn relied_on_text(relied_on: &[String]) -> Option<String> {
    (!relied_on.is_empty()).then(|| format!("{RELIED_ON_LABEL}: {}", relied_on.join(", ")))
}
