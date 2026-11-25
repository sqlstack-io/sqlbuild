//! Build result facts and their aggregate counts.

/// The status, error code, warning count and audit outcomes of one model result.
#[derive(Debug, Clone, Default)]
pub struct ModelResultFacts {
    pub status: String,
    pub error_code: Option<String>,
    pub warning_count: usize,
    pub audit_outcomes: Vec<String>,
}

/// The statuses and outcomes of every result one build produced.
#[derive(Debug, Clone, Default)]
pub struct BuildResultFacts {
    pub models: Vec<ModelResultFacts>,
    pub seed_statuses: Vec<String>,
    /// `(status, warning count)` of each function result.
    pub functions: Vec<(String, usize)>,
    pub load_statuses: Vec<String>,
    pub test_outcomes: Vec<String>,
    pub source_audit_outcomes: Vec<String>,
    pub end_audit_outcomes: Vec<String>,
}

/// Aggregate counts and whether the build failed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BuildCounts {
    pub success: usize,
    /// Failed items, timed-out items included.
    pub failure: usize,
    /// Timed-out models and tests, also counted in `failure`.
    pub timeout: usize,
    pub skipped: usize,
    pub warning: usize,
    pub insufficient: usize,
    /// Error-severity audit failures that held nothing back, counted in no other field.
    pub ignored_audit_failures: usize,
    /// End-of-build audits whose reads the target doesn't have, counted in no other field.
    pub not_run: usize,
    pub failed: bool,
}
