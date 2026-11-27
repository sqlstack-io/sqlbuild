//! Plain data of test and scenario run output: counts, the live footer, failures and the header.

/// How many items passed, failed, timed out and did not run, and how many there are.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunCounts {
    pub pass: usize,
    pub fail: usize,
    pub timeout: usize,
    pub not_run: usize,
    pub total: usize,
}

impl RunCounts {
    /// Items with a result so far.
    #[must_use]
    pub fn done(&self) -> usize {
        self.pass + self.fail + self.timeout + self.not_run
    }

    /// Whether every item passed.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.pass == self.total
    }
}

/// What the live footer shows while items run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProgressFooter {
    /// `Testing` or `Running scenarios`.
    pub verb: String,
    /// The plural noun of a heartbeat line: `tests` or `scenarios`.
    pub noun: String,
    pub counts: RunCounts,
    /// Items executing now.
    pub running: usize,
    /// Items that finished and wait for an earlier one before their row prints.
    pub held: usize,
    /// The item running longest, and for how many seconds.
    pub oldest: Option<(String, f64)>,
    pub elapsed: f64,
}

/// How one row of a failure's detail is styled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RowTone {
    /// Column names and other context, dimmed.
    #[default]
    Header,
    /// A row only the expected side has.
    Expected,
    /// A row only the actual side has.
    Actual,
    /// An error's own rows, such as an assertion's.
    Error,
    /// Plain text.
    Plain,
}

/// One row of a failure's detail, with a dim note after it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FailureRow {
    pub tone: RowTone,
    pub text: String,
    pub note: String,
}

/// One labelled part of a failure's detail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailureField {
    /// The error: its code, message and help.
    Error {
        code: Option<String>,
        message: String,
        help: Option<String>,
    },
    /// A dim label in the type column, then text wrapped on whitespace.
    Labelled { label: String, text: String },
    /// Text under the label column that is never wrapped or cut, such as a path.
    Verbatim(String),
    /// Rows under the label column, such as a row diff.
    Rows(Vec<FailureRow>),
    /// A code frame: a source line with a caret under the column.
    Frame {
        line: i64,
        column: i64,
        width: i64,
        snippet: String,
    },
}

/// One failed item's detail in the `Failures:` section.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FailureEntry {
    /// The full name, never cut.
    pub name: String,
    /// `test, local, 5.45s`.
    pub meta: String,
    pub fields: Vec<FailureField>,
}

/// One cause that keeps items from running locally, as the header counts it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeaderCause {
    /// `fn:HASH`, or `project function`.
    pub label: String,
    pub count: usize,
    /// `SQ0008 · 12 models`.
    pub detail: String,
}

/// One `[N] reason  where` row of `--explain`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExplainRow {
    pub count: usize,
    pub reason: String,
    pub place: String,
}

/// The header before a run: how many items run where, and why some can't run locally.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunHeader {
    /// `Tests` or `Scenarios`.
    pub title: String,
    /// The singular and plural noun: `test`, `tests`.
    pub nouns: (String, String),
    pub selected: usize,
    pub local: usize,
    pub warehouse: usize,
    /// Items that can't run: under `--run-on local`, those DuckDB can't run.
    pub not_run: usize,
    /// Whether items that can't run locally are errors, as under `--run-on local`.
    pub local_only: bool,
    /// The causes, most items first.
    pub causes: Vec<HeaderCause>,
    /// The `[N] reason` rows, shown with `--explain`.
    pub explain: Vec<ExplainRow>,
}
