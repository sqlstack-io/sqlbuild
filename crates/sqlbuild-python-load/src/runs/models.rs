//! What a run reports for each table.

/// One table's load, as SQLBuild records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedTable {
    pub table: String,
    /// The rows it loaded, or why it failed.
    pub loaded: Result<u64, String>,
    /// SQLLoad's `created ...` and `altered ...` lines.
    pub changes: Vec<String>,
    /// Whether a failed load may succeed when run again unchanged.
    pub retryable: bool,
}
