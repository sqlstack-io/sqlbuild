//! Public constants of execution history.

use crate::execution_history::_helpers::getters::read_postgres_execution_history;
use crate::execution_history::_helpers::getters::read_sqlite_execution_history;
use crate::execution_history::models::PostgresExecutionHistory;
use crate::execution_history::models::SqliteExecutionHistory;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub const GETTERS: &[GetterEntry] = &[
    GetterEntry {
        class: class_of::<PostgresExecutionHistory>,
        read: read_postgres_execution_history,
    },
    GetterEntry {
        class: class_of::<SqliteExecutionHistory>,
        read: read_sqlite_execution_history,
    },
];
