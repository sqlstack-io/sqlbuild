//! Control stores preserve Recall's logical columns and add a committed-order event cursor.

use crate::schema::_helpers::relational_tables::relational_table;
use crate::schema::constants::ALL_TABLES;
use crate::schema::main::table_spec::table_spec;

/// PostgreSQL partitions invocation facts; globally deduplicated artifacts remain unpartitioned.
#[must_use]
pub fn control_table_statements(postgres: bool) -> Vec<String> {
    ALL_TABLES
        .iter()
        .flat_map(|table| relational_table(table_spec(*table), postgres, true))
        .collect()
}
