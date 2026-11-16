//! Whether a target's schema or database resolves only inside a model.

use crate::templates::_helpers::deferred::{CONTEXT_PREFIX, PLACEHOLDER_OPEN, placeholders};

/// Whether `template` holds a `${CTX:key}` placeholder, so it names a location only inside a model.
pub fn holds_context_placeholder(template: &str) -> bool {
    !placeholders(
        template,
        &format!("{PLACEHOLDER_OPEN}{CONTEXT_PREFIX}"),
        None,
    )
    .is_empty()
}
