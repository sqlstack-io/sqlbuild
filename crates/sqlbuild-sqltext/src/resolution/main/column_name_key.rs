//! The key that matches a declared column name to a warehouse or inferred column name.

use crate::resolution::main::unquoted_column_name::unquoted_column_name;

/// The case-folded unquoted name, so `"listingId"`, `listingId` and `LISTINGID` share one key.
#[must_use]
pub fn column_name_key(name: &str) -> String {
    unquoted_column_name(name).to_lowercase()
}
