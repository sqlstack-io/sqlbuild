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

/// SQLLoad's plan of a project, as SQLBuild shows and checks it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedProject {
    /// The plan as `sqlload plan` prints it.
    pub lines: Vec<String>,
    /// Every selected table can load.
    pub ready: bool,
    /// The destination table name of each table the plan loads.
    pub tables: Vec<String>,
    /// Required destination tables the plan leaves out.
    pub missing: Vec<String>,
}

/// A project's run: each table's load, the destination tables its plan held, and those it was required to and didn't.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRun {
    pub tables: Vec<LoadedTable>,
    pub planned: Vec<String>,
    /// Required destination tables the plan left out; nothing loads when there are any.
    pub missing: Vec<String>,
}
