//! SQLite store pragmas and maintenance budgets.

pub const BUSY_TIMEOUT_MS: u64 = 5_000;
/// `journal_size_limit`: the WAL is truncated to this size after checkpoints.
pub const JOURNAL_SIZE_LIMIT_BYTES: u64 = 64 * 1024 * 1024;
/// Maintenance runs at most once per this many seconds.
pub const MAINTENANCE_INTERVAL_SECONDS: u64 = 24 * 60 * 60;
/// Total time one maintenance run may spend deleting.
pub const MAINTENANCE_BUDGET_MS: u64 = 2_000;
/// Pages released by one `incremental_vacuum`.
pub const INCREMENTAL_VACUUM_PAGES: u32 = 2_048;
pub const META_MAINTENANCE_OVERDUE_KEY: &str = "overdue";
/// The selector naming the most recently started invocation.
pub const LAST_INVOCATION_SELECTOR: &str = "last";
/// Invocations an ambiguous prefix lists; one more is read to tell whether there are others.
pub const LISTED_INVOCATION_CANDIDATES: usize = 10;
/// The first bytes of every SQLite database file.
pub const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\0";
/// What a store moved aside for corruption is renamed to, before its UTC timestamp.
pub const CORRUPT_SUFFIX: &str = ".corrupt-";
/// SQLite's companion files, moved along with a corrupt store.
pub const COMPANION_SUFFIXES: [&str; 2] = ["-wal", "-shm"];
/// Appended to the store path to name the lock file that serialises moving a corrupt store aside.
pub const QUARANTINE_LOCK_SUFFIX: &str = ".lock";
/// Pause between attempts to take the quarantine lock.
pub const QUARANTINE_LOCK_RETRY_MS: u64 = 10;
/// The step between decimal size units.
pub const DECIMAL_UNIT_STEP: f64 = 1000.0;
/// The step between binary size units.
pub const BINARY_UNIT_STEP: u64 = 1024;
/// Decimal size units above bytes, for omitted output.
pub const DECIMAL_UNITS: [&str; 4] = ["kB", "MB", "GB", "TB"];
/// Binary size units above bytes, for kept output.
pub const BINARY_UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
/// The `sqlite_schema.type` of a view.
pub(crate) const SQLITE_VIEW_TYPE: &str = "view";
/// The fewest characters of a run ID run selectors take, like a short git hash.
pub const MIN_RUN_ID_PREFIX: usize = 8;
/// How many runs an ambiguous run ID prefix lists.
pub const LISTED_RUN_CANDIDATES: usize = 5;
