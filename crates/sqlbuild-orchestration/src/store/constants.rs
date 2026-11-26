//! Control-store limits and schema.

/// The PostgreSQL pool's connection limit.
pub(crate) const POOL_CONNECTIONS: u32 = 10;
/// How long a request waits for a pooled PostgreSQL connection, in seconds.
pub(crate) const POOL_WAIT_SECONDS: u64 = 30;

/// A SQLite WAL past this size is reset right after the commit that grew it.
pub(crate) const LARGE_WAL_BYTES: u64 = 64 * 1024 * 1024;

/// The control-store schema template, rendered per database type.
pub(crate) const SCHEMA: &str = include_str!("schema.sql");

/// Indexes, applied on every open; like `SCHEMA`, a change needs a numbered migration.
pub(crate) const INDEXES: &str = include_str!("indexes.sql");

/// Why a run whose pinned snapshot no longer decodes fails on its own.
pub(crate) const UNREADABLE_SNAPSHOT: &str = "this run's snapshot was published by an earlier SQLBuild release and can't be read; rerun it after its location publishes again";
/// Why a request that reads a published snapshot that no longer decodes, such as a job page, a group run request or an ad hoc plan, fails on its own.
pub(crate) const UNREADABLE_PUBLICATION: &str = "this location's published snapshot came from an earlier SQLBuild release and this server can't read it; it works again after the location publishes again";
