//! The control-store schema baseline.

/// The oldest schema version this build upgrades; migrations at or below it are squashed away.
pub(crate) const BASELINE_VERSION: i64 = 1;

/// The checksum unversioned SQLite stores record for the baseline schema (releases before versioning).
pub(crate) const SQLITE_BASELINE_CHECKSUM: &str =
    "228a951fe04d75eedcefdb44095afe652b178d9b2f32f3a5287c143877d39ce2";

/// The checksum unversioned PostgreSQL stores record for the baseline schema.
pub(crate) const POSTGRES_BASELINE_CHECKSUM: &str =
    "163045e32da43a8b0ddf17ea22f6dd41d1ff7bffe81836f3cbb35a8754915c3e";
