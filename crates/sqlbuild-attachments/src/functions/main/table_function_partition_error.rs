//! A whole-partition table function called without `OVER (PARTITION BY ...)`.

use std::collections::HashSet;

use crate::functions::constants::TABLE_FUNCTION_KIND;

/// The first `(kind, name, partitioned)` call of a whole-partition function that has no partition.
#[must_use]
pub fn table_function_partition_error(
    owner: &str,
    calls: &[(&str, &str, bool)],
    whole_partition: &HashSet<String>,
) -> Option<String> {
    calls.iter().find_map(|&(kind, name, partitioned)| {
        (kind == TABLE_FUNCTION_KIND && !partitioned && whole_partition.contains(name)).then(|| {
            format!(
                "{owner} calls Python table function '{name}' without OVER (PARTITION BY ...); \
                 it receives one whole partition per call, so add OVER (PARTITION BY <columns>) \
                 after the call"
            )
        })
    })
}
