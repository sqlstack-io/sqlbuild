//! A column name without the double quotes of a quoted identifier.

/// The name without the double quotes of a quoted identifier, such as `listingId` for `"listingId"`.
#[must_use]
pub fn unquoted_column_name(name: &str) -> &str {
    name.strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .filter(|inner| !inner.is_empty() && !inner.contains('"'))
        .unwrap_or(name)
}
