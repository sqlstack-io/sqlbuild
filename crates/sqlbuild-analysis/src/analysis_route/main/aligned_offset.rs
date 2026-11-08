/// The character of `source` that `derived`'s `offset` came from, when the text before or after holds.
#[must_use]
pub fn aligned_offset(derived: &str, offset: usize, source: &str) -> Option<usize> {
    crate::analysis_route::_helpers::analysis::located_gap::aligned(derived, offset, source)
}
