/// The mode new artifacts are created with before the process umask, as Python's `os.open`.
pub const NEW_FILE_MODE: u32 = 0o666;
/// The existing SQL-test cache snapshot format, shared by native readers and writers.
pub(crate) const SQL_TEST_CACHE_VERSION: u64 = 5;
pub(crate) const SQL_TEST_CACHE_FILE: &str = "sql-test-artifacts.json";
pub(crate) const MAX_SQL_TEST_CACHE_BYTES: u64 = 10_000_000;
pub(crate) const MAX_SQL_TEST_CACHE_RECORDS: usize = 100_000;
/// The extension every compiled SQL test artifact carries.
pub(crate) const SQL_EXTENSION: &str = "sql";
#[cfg(unix)]
pub(crate) const NANOS_PER_SECOND: i64 = 1_000_000_000;
/// Below this many files a sequential write is cheaper than starting a pool.
pub(crate) const PARALLEL_WRITE_MIN_FILES: usize = 64;
/// Writer threads for one artifact batch, matching SQL test planning's fixed pool.
pub(crate) const WRITE_WORKERS: usize = 4;
