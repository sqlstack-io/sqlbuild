//! Numbered control-store migrations and what opening a store would do with them.

use crate::store_migrations::types::StoreMigrationStep;

/// One numbered control-store migration, with its SQL for each store type and an optional data step.
#[derive(Clone, Copy, Debug)]
pub struct StoreMigration {
    /// The file name without its number, such as `add_skipped_runs`.
    pub name: &'static str,
    /// Statements for SQLite, separated by `;`.
    pub sqlite: &'static str,
    /// Statements for PostgreSQL, separated by `;`.
    pub postgres: &'static str,
    /// A data change SQL can't make, such as rewriting compressed documents, run after the statements.
    pub(crate) step: Option<StoreMigrationStep>,
}

/// What opening a control store would do to its schema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreMigrationPlan {
    /// No store yet: create one from the current schema at `version`.
    Create { version: i64 },
    /// The store is at the current `version` already.
    Current { version: i64 },
    /// Apply the numbered migrations `names`, in order, from `from` to `to`.
    Upgrade {
        from: i64,
        to: i64,
        names: Vec<String>,
    },
}
