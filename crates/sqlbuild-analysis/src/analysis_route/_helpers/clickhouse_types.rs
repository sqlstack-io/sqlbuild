use crate::analysis_route::constants::{LOW_CARDINALITY_WRAPPER, NULLABLE_WRAPPER};

/// `type_sql` without whitespace outside its quoted strings.
pub(crate) fn unspaced(type_sql: &str) -> String {
    let mut quoted: bool = false;
    type_sql
        .chars()
        .filter(|character| {
            if *character == '\'' {
                quoted = !quoted;
            }
            quoted || !character.is_whitespace()
        })
        .collect()
}

/// The type inside its top-level `Nullable` and `LowCardinality` wrappers, and whether it is nullable.
pub(crate) fn unwrapped(type_sql: &str) -> (&str, bool) {
    let mut current: &str = type_sql.trim();
    let mut nullable: bool = false;
    loop {
        if let Some(inner) = wrapped(current, NULLABLE_WRAPPER) {
            nullable = true;
            current = inner;
        } else if let Some(inner) = wrapped(current, LOW_CARDINALITY_WRAPPER) {
            current = inner;
        } else {
            return (current, nullable);
        }
    }
}

/// The argument of `wrapper(...)` when the whole of `type_sql` is that one call.
fn wrapped<'a>(type_sql: &'a str, wrapper: &str) -> Option<&'a str> {
    let head: &str = type_sql.get(..wrapper.len())?;
    if !head.eq_ignore_ascii_case(wrapper) {
        return None;
    }
    let inner: &str = type_sql[wrapper.len()..]
        .trim_start()
        .strip_prefix('(')?
        .strip_suffix(')')?;
    let mut depth: usize = 0;
    for character in inner.chars() {
        match character {
            '(' => depth += 1,
            ')' if depth == 0 => return None,
            ')' => depth -= 1,
            _ => {}
        }
    }
    Some(inner.trim())
}
