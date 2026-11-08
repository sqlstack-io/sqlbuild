//! The compiled relation marker call pattern.

use std::sync::LazyLock;

use regex::Regex;

use crate::relation_markers::constants::RELATION_MARKER_PATTERN;

static RELATION_MARKER: LazyLock<Result<Regex, String>> =
    LazyLock::new(|| Regex::new(RELATION_MARKER_PATTERN).map_err(|error| error.to_string()));

/// The compiled relation marker call pattern, or its compile error.
pub(crate) fn relation_marker_pattern() -> Result<&'static Regex, String> {
    RELATION_MARKER.as_ref().map_err(Clone::clone)
}
