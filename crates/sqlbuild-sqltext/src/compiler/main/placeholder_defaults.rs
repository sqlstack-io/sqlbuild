//! Python's `substitute_placeholder_defaults`.

use crate::compiler::main::placeholder_default_edits::placeholder_default_edits;

/// `substitute_placeholder_defaults`: `@@@name` becomes its default; unknown names stay.
pub fn placeholder_defaults(sql: &str, placeholders: &[(String, String)]) -> String {
    let mut result: String = String::with_capacity(sql.len());
    let mut copied: usize = 0;
    for (start, end, default) in placeholder_default_edits(sql, placeholders) {
        result.push_str(&sql[copied..start]);
        result.push_str(default);
        copied = end;
    }
    result.push_str(&sql[copied..]);
    result
}
