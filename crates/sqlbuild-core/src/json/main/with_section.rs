//! Add a named section to a JSON document.

use serde_json::Value;

/// `payload`, a JSON document, with `section` added under `name`, pretty-printed with a trailing newline.
pub fn with_section(
    payload: &str,
    name: &str,
    section: Value,
) -> Result<String, serde_json::Error> {
    let mut document: Value = serde_json::from_str(payload)?;
    if let Value::Object(fields) = &mut document {
        fields.insert(name.to_owned(), section);
    }
    Ok(format!("{}\n", serde_json::to_string_pretty(&document)?))
}
