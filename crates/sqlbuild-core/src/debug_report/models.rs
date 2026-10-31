//! `sqb debug` report lines and the facts they are built from.

use std::path::PathBuf;

/// The outcome a diagnostic line reports (`DebugCheckStatus`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugStatus {
    Ok,
    Error,
    Skip,
}

impl DebugStatus {
    /// The status value shown in text and JSON output.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Error => "ERROR",
            Self::Skip => "SKIP",
        }
    }
}

/// One diagnostic line: a label, a message and an optional check status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebugLine {
    pub label: String,
    pub message: String,
    pub status: Option<DebugStatus>,
    pub status_message: Option<String>,
}

/// Every section of the `sqb debug` report.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DebugReport {
    pub runtime: Vec<DebugLine>,
    pub configuration: Vec<DebugLine>,
    pub providers: Vec<DebugLine>,
    pub connection: Vec<DebugLine>,
    /// Exact local database paths, not the shortened display values in connection lines.
    pub local_files: Vec<String>,
}

impl DebugReport {
    /// No line reports an error.
    #[must_use]
    pub fn success(&self) -> bool {
        [
            &self.runtime,
            &self.configuration,
            &self.providers,
            &self.connection,
        ]
        .into_iter()
        .flatten()
        .all(|line| line.status != Some(DebugStatus::Error))
    }
}

/// The interpreter and platform SQLBuild runs on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebugRuntime {
    pub sqlbuild_version: Option<String>,
    pub python_version: String,
    pub python_path: String,
    pub os_info: String,
}

/// The project configuration files `sqb debug` reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebugConfigFiles {
    pub project_file: PathBuf,
    /// The local config file, when it exists.
    pub local_file: Option<PathBuf>,
}

/// The resolved project, adapter, target and namespace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebugProjectFacts {
    /// The project config file, as Python prints the path.
    pub project_file: String,
    /// The local config file, when it exists.
    pub local_file: Option<String>,
    pub project_name: String,
    pub adapter_name: String,
    pub target_name: Option<String>,
    pub connection_name: Option<String>,
    /// The effective database, when it is a non-empty string.
    pub database: Option<String>,
    /// The effective schema, when it is a non-empty string.
    pub schema: Option<String>,
}

/// One command group's warehouse as `sqb debug` resolved it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WarehouseCheck {
    /// The adapter has no session warehouse, but the target authors one at `source`.
    NotUsed {
        group: String,
        adapter_name: String,
        source: String,
    },
    Unresolved {
        group: String,
        error: String,
    },
    Resolved {
        group: String,
        warehouse: Option<String>,
        source: String,
    },
}

/// One discovered provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebugProvider {
    pub name: String,
    pub relative_path: String,
    pub class_name: String,
}

/// One connection setting value, as Python would print it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionValue {
    /// `None`.
    Null,
    /// A `bool`, `int` or `float`, as `str(value)`; never shortened.
    Scalar(String),
    /// Any other value, as `str(value)`.
    Text(String),
}

/// What the live connection and `SELECT 1` checks found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionCheck {
    /// `--no-connection` was passed.
    Skipped,
    /// Connecting raised this message.
    ConnectFailed(String),
    /// Connected; the query raised this message when it failed.
    Connected { query_error: Option<String> },
}

impl DebugLine {
    /// A line without a check status.
    #[must_use]
    pub fn plain(label: &str, message: &str) -> Self {
        Self {
            label: label.to_owned(),
            message: message.to_owned(),
            status: None,
            status_message: None,
        }
    }

    /// A line with a check status and an optional status message.
    #[must_use]
    pub fn checked(
        label: &str,
        message: &str,
        (status, status_message): (DebugStatus, Option<&str>),
    ) -> Self {
        Self {
            label: label.to_owned(),
            message: message.to_owned(),
            status: Some(status),
            status_message: status_message.map(str::to_owned),
        }
    }
}

/// Why `sqb debug` failed: the first failed check as an error type, code and message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebugFailure {
    pub error_type: String,
    pub error_code: String,
    pub message: String,
}
