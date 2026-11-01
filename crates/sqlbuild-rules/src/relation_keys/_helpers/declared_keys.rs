//! Unique keys one relation declares through its incremental config and column audits.

use crate::relation_keys::constants::{
    INCREMENTAL_MATERIALIZATION, KEY_PRESERVING_STRATEGIES, KEYED_DELETE_INSERT_STRATEGY,
    MICROBATCH_MODE, NOT_NULL_AUDIT, UNIQUE_AUDIT,
};
use crate::relation_keys::models::{DeclaredKeyColumn, DeclaredRelation};

/// `unique_key` only where the incremental strategy keeps one row per key.
pub(crate) fn trusted_unique_key(relation: &DeclaredRelation) -> Vec<Vec<String>> {
    let strategy: String = relation.incremental_strategy.to_lowercase();
    let trusted: bool = relation.materialized.to_lowercase() == INCREMENTAL_MATERIALIZATION
        && relation.incremental_mode.to_lowercase() != MICROBATCH_MODE
        && KEY_PRESERVING_STRATEGIES.contains(&strategy.as_str())
        && !(strategy == KEYED_DELETE_INSERT_STRATEGY && relation.has_cursor);
    match &relation.unique_key {
        Some(key) if trusted && !key.is_empty() => vec![key.clone()],
        _ => Vec::new(),
    }
}

/// Columns with a `unique` audit that are also non-null by audit or `nullable false`.
pub(crate) fn non_null_unique_columns(relation: &DeclaredRelation) -> Vec<String> {
    let mut unique: Vec<String> = Vec::new();
    let mut not_null: Vec<String> = Vec::new();
    for column in &relation.columns {
        if has_audit(column, UNIQUE_AUDIT) {
            unique.push(column.name.clone());
        }
        if has_audit(column, NOT_NULL_AUDIT) || column.not_nullable {
            not_null.push(str::to_lowercase(&column.name));
        }
    }
    for (definition, argument) in &relation.audits {
        let Some(argument) = argument else {
            continue;
        };
        if definition == NOT_NULL_AUDIT {
            not_null.push(str::to_lowercase(argument));
        } else if definition == UNIQUE_AUDIT {
            unique.push(argument.clone());
        }
    }
    unique.retain(|column| not_null.contains(&str::to_lowercase(column)));
    unique
}

fn has_audit(column: &DeclaredKeyColumn, name: &str) -> bool {
    column.audits.iter().any(|audit| audit == name)
}
