//! Shared builders for local scenario SQL conversion tests.

/// Owned (name, type) column pairs.
pub(crate) fn columns(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(name, data_type)| ((*name).to_owned(), (*data_type).to_owned()))
        .collect()
}
