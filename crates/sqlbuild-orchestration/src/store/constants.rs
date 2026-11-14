//! Control-store limits.

/// A SQLite WAL past this size is reset right after the commit that grew it.
pub(crate) const LARGE_WAL_BYTES: u64 = 64 * 1024 * 1024;
