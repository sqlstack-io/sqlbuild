//! The control-store schema baseline.

/// The oldest schema version this build upgrades; migrations at or below it are squashed away.
pub(crate) const BASELINE_VERSION: i64 = 1;

/// What unversioned SQLite stores record for the baseline: releases built from LF, then CRLF (Windows), SQL.
pub(crate) const SQLITE_BASELINE_CHECKSUMS: &[&str] = &[
    "228a951fe04d75eedcefdb44095afe652b178d9b2f32f3a5287c143877d39ce2",
    "4cb5c0c4f392a2b3f4f2229e2ab8401870cd37b65346ada92cc70aa3ca2e0542",
];

/// What unversioned PostgreSQL stores record for the baseline: releases built from LF, then CRLF, SQL.
pub(crate) const POSTGRES_BASELINE_CHECKSUMS: &[&str] = &[
    "163045e32da43a8b0ddf17ea22f6dd41d1ff7bffe81836f3cbb35a8754915c3e",
    "90758add355967a65ddfcf233b5e66893f92449e17c84e8f9bab42fb703917ab",
];
