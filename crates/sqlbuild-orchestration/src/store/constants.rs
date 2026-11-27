//! Control-store limits and schema.

/// The PostgreSQL pool's connection limit.
pub(crate) const POOL_CONNECTIONS: u32 = 10;
/// How long a request waits for a pooled PostgreSQL connection, in seconds.
pub(crate) const POOL_WAIT_SECONDS: u64 = 30;
/// How long one statement may run on a server session before PostgreSQL cancels it.
pub(crate) const STATEMENT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);
/// How long a server session may sit idle inside a transaction before PostgreSQL ends it, so a stuck task never holds one open.
pub(crate) const IDLE_IN_TRANSACTION_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(120);
/// Both limits for a migration or the publication of a large snapshot.
pub(crate) const LONG_TRANSACTION_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(1800);
/// The SQLSTATE of a session PostgreSQL ended for sitting idle in a transaction.
pub(crate) const IDLE_IN_TRANSACTION_CODE: &str = "25P03";
/// The SQLSTATE of a statement PostgreSQL cancelled, which a statement timeout raises.
pub(crate) const QUERY_CANCELED_CODE: &str = "57014";
/// Cut-off sessions kept for the health probe to report; older ones are dropped once logged.
pub(crate) const PENDING_CUT_OFFS: usize = 16;

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
