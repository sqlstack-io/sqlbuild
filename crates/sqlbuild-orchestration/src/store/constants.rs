//! Control-store limits and schema.

/// A SQLite WAL past this size is reset right after the commit that grew it.
pub(crate) const LARGE_WAL_BYTES: u64 = 64 * 1024 * 1024;

/// The control-store schema template, rendered per database type.
pub(crate) const SCHEMA: &str = include_str!("schema.sql");

/// Indexes, applied on every open; like `SCHEMA`, a change needs a numbered migration.
pub(crate) const INDEXES: &str = include_str!("indexes.sql");
