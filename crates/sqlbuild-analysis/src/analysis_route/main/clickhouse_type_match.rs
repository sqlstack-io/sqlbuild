use crate::analysis_route::constants::{LOW_CARDINALITY_WRAPPER, NULLABLE_WRAPPER};
use crate::analysis_route::models::TypeMatch;

/// A declared and an inferred ClickHouse type compared without top-level wrappers or spacing,
/// nullability apart. A declared type without `Nullable` forbids null even when the inferred type
/// is unknown. An arm compares spellings it has respelt through its aliases.
#[must_use]
pub fn clickhouse_type_match(
    declared: &str,
    inferred: Option<&str>,
    inferred_nullable: bool,
) -> TypeMatch {
    let (declared_base, declared_nullable) = unwrapped(declared);
    let Some(inferred) = inferred else {
        return TypeMatch::NotCompared {
            nullable_into_non_null: !declared_nullable && inferred_nullable,
        };
    };
    let (inferred_base, wrapped_nullable) = unwrapped(inferred);
    TypeMatch::Compared {
        same_type: unspaced(declared_base) == unspaced(inferred_base),
        nullable_into_non_null: !declared_nullable && (inferred_nullable || wrapped_nullable),
    }
}

/// `type_sql` without whitespace outside its quoted strings, so `Decimal(10, 2)` is `Decimal(10,2)`
/// and an `Enum8('a b' = 1)` value keeps its space.
fn unspaced(type_sql: &str) -> String {
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
fn unwrapped(type_sql: &str) -> (&str, bool) {
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
