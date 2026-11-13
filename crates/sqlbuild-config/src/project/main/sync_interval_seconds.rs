//! Parse a safety sync interval the way `[control] sync_at_least_every` accepts it.

/// Whole seconds, `0` for `"0"`, `"0s"` or `"off"`; the error completes "<setting> ...".
pub fn sync_interval_seconds(text: &str) -> Result<u64, &'static str> {
    crate::project::_helpers::sections::sync_interval(text)
}
