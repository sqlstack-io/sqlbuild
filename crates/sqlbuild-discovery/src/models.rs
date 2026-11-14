//! Results native discovery hands to the Python facade.

use crate::tree::main::display_text::display_text;
use std::path::PathBuf;

/// The Python `DiscoveryError` subclass a native failure is raised as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    /// `ModelSqlParseError` (D002).
    ModelSql,
    /// `DeclarationParseError` (D013).
    Declaration,
    /// `SqlTestParseError` (D003).
    SqlTest,
    /// `SqlScenarioParseError` (D009).
    SqlScenario,
    /// `SchemaParseError` (D005).
    Schema,
    /// `SourceParseError` (D006).
    Source,
    /// `ProjectPathError` (D016).
    ProjectPath,
    /// `SqlHookParseError` (D014).
    SqlHook,
    /// `SqlAuditParseError` (D004).
    SqlAudit,
    /// `ResourceIdentityError` (D016).
    ResourceIdentity,
    /// `DiscoveryConflictError` (D007).
    Conflict,
    /// `SeedDiscoveryError` (D008).
    Seed,
    /// `OrchestrationDeclarationError` (D017).
    Orchestration,
}

impl FailureKind {
    /// The stable name the facade maps to an exception class.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ModelSql => "model_sql",
            Self::Declaration => "declaration",
            Self::SqlTest => "sql_test",
            Self::SqlScenario => "sql_scenario",
            Self::Schema => "schema",
            Self::Source => "source",
            Self::ProjectPath => "project_path",
            Self::SqlHook => "sql_hook",
            Self::SqlAudit => "sql_audit",
            Self::ResourceIdentity => "resource_identity",
            Self::Conflict => "conflict",
            Self::Seed => "seed",
            Self::Orchestration => "orchestration",
        }
    }
}

/// One discovery failure with Python's exact message and optional help.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryFailure {
    pub kind: FailureKind,
    pub message: String,
    pub help: Option<String>,
}

impl DiscoveryFailure {
    /// A failure without help text.
    pub fn new(kind: FailureKind, message: String) -> Self {
        Self {
            kind,
            message,
            help: None,
        }
    }
}

/// Why a whole discovery collection cannot be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StageFailure {
    /// A directory Python lists with `iterdir()` could not be listed: Python's `OSError`.
    Unlistable {
        relative_path: String,
        error: ReadFailure,
    },
    /// The discovery worker pool could not start.
    Internal(String),
}

/// The outcome of reading and parsing one authored file.
#[derive(Clone, Debug, PartialEq)]
pub enum FileOutcome<T> {
    Parsed(T),
    Failed(DiscoveryFailure),
    /// The file could not be read or decoded as Python's `read_text` would.
    Unreadable(ReadFailure),
}

/// One discovered file, by `/`-separated path relative to the project directory.
#[derive(Clone, Debug, PartialEq)]
pub struct DiscoveredFile<T> {
    pub relative_path: String,
    pub outcome: FileOutcome<T>,
}

/// A one-based line and code-point column range, as Python's `SourceLocation` stores it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineColumnSpan {
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

/// The project directory and the text Python prints before a relative path (`str(project_dir / x)`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectRoot {
    pub directory: PathBuf,
    pub display_prefix: String,
}

impl ProjectRoot {
    /// Python's `str(project_dir / relative_path)` for a `/`-separated relative path.
    pub fn display_path(&self, relative_path: &str) -> String {
        format!(
            "{}{}",
            self.display_prefix,
            display_text(relative_path).replace('/', std::path::MAIN_SEPARATOR_STR)
        )
    }
}

/// Why an authored file could not be read the way Python's `read_text` reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadFailure {
    /// A refused read: `OSError(errno, strerror, path)`, or `winerror` for a Windows listing.
    Io {
        errno: Option<i32>,
        winerror: Option<i32>,
        message: String,
    },
    /// Python's `UnicodeDecodeError("utf-8", bytes, start, end, reason)`.
    Decode {
        bytes: Vec<u8>,
        start: usize,
        end: usize,
        reason: &'static str,
    },
}
