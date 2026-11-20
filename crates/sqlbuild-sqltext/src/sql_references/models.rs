//! Logical SQL references and the outcome of extracting them from one SQL text.

/// One `__ref`, `__source`, `__seed`, `__udf`, `__dbt_ref` or `__table_fn` call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlReference {
    /// Python `SqlReferenceKind` value, such as `ref` or `table_fn`.
    pub kind: &'static str,
    pub name: String,
    pub package: Option<String>,
    /// Top-level arguments of a table function's call suffix; `None` for other kinds.
    pub call_argument_count: Option<usize>,
    /// Whether a table function's call is followed by `OVER (PARTITION BY ...)`.
    pub call_partitioned: bool,
}

/// One reference call compile cannot replace, with Python's P012 message and help.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvalidReferenceCall {
    /// Python `SqlReferenceKind` value of the call.
    pub kind: &'static str,
    /// The call text from its prefix through its closing parenthesis.
    pub call: String,
    /// The code-point offset of the call in the scanned text.
    pub start: usize,
    pub message: String,
    pub help: String,
    pub corrected_call: String,
}

/// The valid references and rejected calls of one SQL text, each in authored order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReferenceScan {
    pub references: Vec<SqlReference>,
    pub invalid_calls: Vec<InvalidReferenceCall>,
}

/// Python's reference scan error and the code-point start of the quote, comment or call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceScanFailure {
    pub message: String,
    pub start: usize,
}

/// The references of one SQL text, or the error that stops the scan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReferenceExtraction {
    /// Every reference and rejected call in authored order, exactly as Python scans them.
    Extracted(ReferenceScan),
    /// The `CompileInputError` raised for this text.
    Failed(ReferenceScanFailure),
}
