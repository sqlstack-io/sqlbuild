use crate::analysis_route::_helpers::clickhouse_types::{unspaced, unwrapped};
use crate::analysis_route::models::TypeMatch;

/// A declared and an inferred ClickHouse type compared without top-level wrappers or spacing.
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
