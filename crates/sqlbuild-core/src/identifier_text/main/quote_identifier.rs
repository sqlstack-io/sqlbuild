//! Standard SQL double-quoted identifiers, including embedded quote escaping.

#[must_use]
pub fn quote_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
