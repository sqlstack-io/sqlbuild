//! The refusal of `replace_key` on adapters that cannot replace groups of rows.

use crate::planner_strategy::constants::GROUP_REPLACEMENT_UNSUPPORTED_DIALECTS;

/// Why `model_name` cannot set `replace_key` on an adapter with SQL dialect `dialect`, if it cannot.
pub fn group_replacement_refusal(
    model_name: &str,
    (adapter_name, dialect): (&str, Option<&str>),
    has_replace_key: bool,
) -> Option<String> {
    let unsupported: bool = has_replace_key
        && dialect.is_some_and(|dialect| GROUP_REPLACEMENT_UNSUPPORTED_DIALECTS.contains(&dialect));
    unsupported.then(|| {
        format!(
            "model '{model_name}' sets replace_key, but adapter '{adapter_name}' cannot replace \
             groups of rows; use unique_key with one row per key, or replace groups on any \
             adapter other than BigQuery"
        )
    })
}
