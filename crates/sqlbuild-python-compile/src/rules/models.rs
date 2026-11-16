//! Public records of rules.

use crate::compile_attachment::models::ExpansionVars;
use crate::rules::_helpers::checking::early_builtin_rules::take_started;
use crate::rules::_helpers::checking::early_builtin_rules::{SLOT, Slot};
use crate::rules::_helpers::checking::native_header_parser::DESCRIPTION_KEY;
use crate::rules::_helpers::checking::native_header_parser::SENTINEL_PATH;
use crate::rules::_helpers::checking::native_header_parser::map_value;
use crate::rules::_helpers::checking::native_header_parser::python_equal;
use crate::rules::_helpers::configuration::rules_config::rules_error;
use crate::rules::_helpers::lint::early_sql_lint::same_codes;
use crate::rules::_helpers::lint::early_sql_lint::start_early_lint;
use crate::rules::_helpers::lint::lint_pipeline::FAULT_SEVERITY;
use crate::rules::_helpers::lint::lint_pipeline::HEADER_ENGINE;
use crate::rules::_helpers::lint::lint_pipeline::NATIVE_ENGINE;
use crate::rules::_helpers::lint::lint_pipeline::WARNING_SEVERITY;
use crate::rules::_helpers::lint::lint_pipeline::offset;
use pyo3::exceptions::PyException;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::Bound;
use pyo3::prelude::IntoPyObject;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyErr;
use pyo3::prelude::PyRefMut;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyString;
use pyo3::types::PyTuple;
use sqlbuild_core::panics::main::catch_compiler_panic::catch_compiler_panic;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_threads::main::join_interruptibly::join_interruptibly;
use sqlbuild_python_base::native_threads::models::Joined;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as record_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::render::models::SqlExpansionInputs;
use sqlbuild_python_front::sqltext::main::header_failure::header_failure;
use sqlbuild_rules::dsl_headers::models::HeaderSeverity;
use sqlbuild_rules::dsl_headers::models::HeaderViolation;
use sqlbuild_rules::dsl_headers::models::ParseFailure;
use sqlbuild_rules::dsl_headers::models::ParseOutcome;
use sqlbuild_rules::dsl_headers::types::HeaderParser;
use sqlbuild_rules::lint_text::constants::SUPPRESSION_CODE;
use sqlbuild_rules::lint_text::models::AuthoredDiagnostic;
use sqlbuild_rules::lint_text::models::SuppressionFinding;
use sqlbuild_sqltext::compiler::main::header_word_value::header_word_value;
use sqlbuild_sqltext::compiler::main::model_header_single_parsing::parse_one;
use sqlbuild_sqltext::compiler::models::AuthoredValue;
use sqlbuild_sqltext::compiler::models::HeaderWord;
use sqlbuild_sqltext::header_strings::main::quote_failure::quote_failure;
use sqlbuild_sqltext::header_strings::models::QuoteFailure;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::PoisonError;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;
use std::time::Instant;

/// What one compile's early built-in rules start from.
#[derive(Debug)]
pub struct EarlyRulesSettings {
    pub project_dir: PathBuf,
    pub dialect: String,
    pub no_sql_analysis: bool,
    /// Whether `--no-cache` turned the Rules cache off, as the rules phase does.
    pub no_cache: bool,
}

/// Arms early built-in rules for one compile analysis on this thread; dropping it stops them.
#[derive(Debug)]
pub struct EarlyRulesScope {
    pub(crate) owner: bool,
}

impl EarlyRulesScope {
    /// Claim this thread's slot unarmed; a nested compile analysis on this thread claims nothing.
    pub(crate) fn new() -> Self {
        let owner: bool = SLOT.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_some() {
                return false;
            }
            *slot = Some(Slot::Spent);
            true
        });
        Self { owner }
    }

    /// Start early rules with `settings` once the compile recovers its model types.
    pub fn arm(&self, settings: EarlyRulesSettings) {
        if !self.owner {
            return;
        }
        SLOT.with(|slot| {
            if let Some(current @ Slot::Spent) = slot.borrow_mut().as_mut() {
                *current = Slot::Armed(settings);
            }
        });
    }

    /// Cancel and join early rules this scope armed that started and were not taken.
    pub fn stop(&self, py: Python<'_>) {
        if self.owner
            && let Some(rules) = take_started()
        {
            rules.discard(py);
        }
    }
}

impl Drop for EarlyRulesScope {
    fn drop(&mut self) {
        if !self.owner {
            return;
        }
        let slot: Option<Slot> = SLOT.with(|slot| slot.borrow_mut().take());
        drop(slot);
    }
}

/// Early SQL lint's outcome and the expansion inputs it linted with.
#[derive(Debug)]
pub struct EarlyLintResult {
    pub outcome: LintOutcome,
    pub context: SqlExpansionInputs,
}

/// One early SQL lint running on a native thread, for one project and SQL rule codes.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct PreparedSqlLint {
    pub(crate) codes: Vec<String>,
    pub(crate) project_dir: PathBuf,
    pub(crate) thread: Mutex<Option<JoinHandle<PyResult<EarlyLintResult>>>>,
    pub(crate) result: OnceLock<Result<EarlyLintResult, PyErr>>,
    pub(crate) stop: Arc<AtomicBool>,
}

impl PreparedSqlLint {
    /// Whether this lint ran for `project_dir` with exactly `codes`.
    pub fn matches(&self, project_dir: &Path, codes: &[String]) -> bool {
        self.project_dir == project_dir && same_codes(&self.codes, codes)
    }

    /// Whether this lint ran with exactly `codes`.
    pub fn has_codes(&self, codes: &[String]) -> bool {
        same_codes(&self.codes, codes)
    }

    /// Whether the lint's result is already known.
    pub(crate) fn finished(&self) -> bool {
        self.result.get().is_some()
    }

    /// The lint's result, waiting without the GIL until it finishes or a signal handler raises.
    pub fn result(&self, py: Python<'_>) -> &Result<EarlyLintResult, PyErr> {
        self.result.get_or_init(|| {
            let thread = self
                .thread
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take();
            let Some(thread) = thread else {
                return Err(PyRuntimeError::new_err("early SQL lint was already joined"));
            };
            match join_interruptibly(py, thread, || self.stop.store(true, Ordering::Release)) {
                Joined::Finished(joined) => joined.unwrap_or_else(|_| {
                    Err(PyRuntimeError::new_err("early SQL lint thread panicked"))
                }),
                Joined::Interrupted(signal) => Err(signal),
            }
        })
    }
}

/// The compiler's inputs and adapter, which lint expansion reuses for the same discovery.
#[derive(Debug)]
pub struct ExpansionReuse {
    pub(crate) inputs: pyo3::prelude::Py<PyAny>,
    pub(crate) value_renderer: pyo3::prelude::Py<PyAny>,
}

impl ExpansionReuse {
    pub(crate) fn clone_ref(&self, py: Python<'_>) -> Self {
        Self {
            inputs: self.inputs.clone_ref(py),
            value_renderer: self.value_renderer.clone_ref(py),
        }
    }

    /// The discovery the compile ran on.
    pub fn discovered_inputs<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        record_field(self.inputs.bind(py), "discovered_inputs")
    }

    /// The compile's declaration scope.
    pub fn declaration_scope<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        record_field(self.inputs.bind(py), "declaration_scope")
    }

    /// The compile's target and vars, read from its macro context.
    pub fn expansion_vars<'py>(&self, py: Python<'py>) -> PyResult<ExpansionVars<'py>> {
        let context: Bound<'py, PyAny> = record_field(self.inputs.bind(py), "macro_context")?;
        Ok(ExpansionVars::Compile {
            target_name: record_field(&context, "target_name")?,
            vars: record_field(&context, "vars")?,
        })
    }

    /// The adapter that renders declared values.
    pub fn value_renderer<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        self.value_renderer.bind(py).clone()
    }
}

/// Own speculative SQL checks for one compilation invocation.
#[pyclass(module = "sqlbuild._native")]
#[derive(Debug)]
pub struct EarlySqlLint {
    #[pyo3(get)]
    pub enabled: bool,
    /// Whether `--no-cache` turned the Rules cache off, so a warm SQL rule cache is ignored.
    pub(crate) no_cache: bool,
    pub(crate) preparation: Option<pyo3::prelude::Py<PreparedSqlLint>>,
    pub(crate) expansion_reuse: Option<ExpansionReuse>,
    /// The dialect and adapter the `on_inputs_ready` call passes to `start`.
    pub(crate) ready_arguments: Option<(String, pyo3::prelude::Py<PyAny>)>,
    pub(crate) stop_requested: Arc<AtomicBool>,
}

impl EarlySqlLint {
    /// A manager that starts speculative checks only when `enabled`.
    pub fn new(enabled: bool, no_cache: bool) -> Self {
        Self {
            enabled,
            no_cache,
            preparation: None,
            expansion_reuse: None,
            ready_arguments: None,
            stop_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// The running early SQL lint, if one started.
    pub fn preparation(&self) -> Option<&pyo3::prelude::Py<PreparedSqlLint>> {
        self.preparation.as_ref()
    }

    /// The compile's discovery and declaration scope, if it had one.
    pub fn expansion_reuse(&self) -> Option<&ExpansionReuse> {
        self.expansion_reuse.as_ref()
    }

    /// This manager as the `on_inputs_ready` hook, which calls `start` with `dialect` and `value_renderer`.
    pub fn inputs_ready_hook<'py>(
        slf: &Bound<'py, Self>,
        dialect: &str,
        value_renderer: &Bound<'py, PyAny>,
    ) -> Bound<'py, PyAny> {
        slf.borrow_mut().ready_arguments =
            Some((dialect.to_owned(), value_renderer.clone().unbind()));
        slf.clone().into_any()
    }

    /// Abandon speculative work, return the milliseconds spent joining it, and raise Ctrl+C caught while waiting.
    pub fn stop(&self, py: Python<'_>) -> PyResult<u64> {
        let started: Instant = Instant::now();
        self.stop_requested.store(true, Ordering::Release);
        if let Some(preparation) = &self.preparation {
            let preparation: &PreparedSqlLint = preparation.get();
            let waiting: bool = !preparation.finished();
            if let Err(error) = preparation.result(py)
                && waiting
                && !error.is_instance_of::<PyException>(py)
            {
                return Err(error.clone_ref(py));
            }
        }
        Ok(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX))
    }
}

#[pymethods]
impl EarlySqlLint {
    #[new]
    #[pyo3(signature = (*, enabled, no_cache=false))]
    pub(crate) fn py_new(enabled: bool, no_cache: bool) -> Self {
        Self::new(enabled, no_cache)
    }

    pub(crate) fn __enter__(slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        slf
    }

    #[pyo3(signature = (*_exception))]
    pub(crate) fn __exit__(&self, py: Python<'_>, _exception: &Bound<'_, PyTuple>) {
        let _ = self.stop(py);
    }

    /// Keep the compile's expansion inputs, and start early SQL lint when enabled and SQL rules are selected.
    #[pyo3(signature = (*, inputs, dialect, value_renderer))]
    pub(crate) fn start(
        mut slf: PyRefMut<'_, Self>,
        inputs: &Bound<'_, PyAny>,
        dialect: &str,
        value_renderer: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let py: Python<'_> = inputs.py();
        let reuse: ExpansionReuse = ExpansionReuse {
            inputs: inputs.clone().unbind(),
            value_renderer: value_renderer.clone().unbind(),
        };
        if slf.enabled {
            let stop: Arc<AtomicBool> = Arc::clone(&slf.stop_requested);
            slf.preparation =
                start_early_lint((inputs, dialect), reuse.clone_ref(py), slf.no_cache, &stop)?
                    .map(|preparation| pyo3::prelude::Py::new(py, preparation))
                    .transpose()?;
        }
        slf.expansion_reuse = (!reuse.declaration_scope(py)?.is_none()).then_some(reuse);
        Ok(())
    }

    /// Start with the arguments `inputs_ready_hook` stored, as the compile's `on_inputs_ready` hook.
    #[pyo3(signature = (*, inputs))]
    pub(crate) fn __call__(slf: PyRefMut<'_, Self>, inputs: &Bound<'_, PyAny>) -> PyResult<()> {
        let py: Python<'_> = inputs.py();
        let Some((dialect, value_renderer)) = slf
            .ready_arguments
            .as_ref()
            .map(|(dialect, renderer)| (dialect.clone(), renderer.clone_ref(py)))
        else {
            return Err(PyRuntimeError::new_err(
                "EarlySqlLint was called before inputs_ready_hook armed it",
            ));
        };
        Self::start(slf, inputs, &dialect, value_renderer.bind(py))
    }

    /// Abandon unconsumed speculative work and return the milliseconds spent joining it.
    #[pyo3(name = "stop")]
    pub(crate) fn py_stop(&self, py: Python<'_>) -> PyResult<u64> {
        self.stop(py)
    }
}

/// One authored SQL file and its text.
#[derive(Clone, Debug)]
pub struct LintFile {
    pub path: PathBuf,
    pub contents: String,
}

/// A proposed authored-source replacement in the violation's file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LintEditRow {
    pub start: i64,
    pub end: i64,
    pub replacement: String,
}

/// One lint violation against the file at index `file` of its run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LintViolationRow {
    pub file: usize,
    pub line: i64,
    pub column: i64,
    pub code: String,
    pub message: String,
    pub fault: bool,
    pub engine: &'static str,
    pub end: Option<(i64, i64)>,
    pub remediation: Option<String>,
    pub fix: Option<LintEditRow>,
    pub fix_unavailable_reason: Option<String>,
}

impl LintViolationRow {
    pub fn header(file: usize, violation: HeaderViolation) -> Self {
        Self {
            file,
            line: offset(violation.line),
            column: offset(violation.column),
            code: violation.code.to_owned(),
            message: violation.message,
            fault: violation.severity == HeaderSeverity::Fault,
            engine: HEADER_ENGINE,
            end: None,
            remediation: Some(violation.remediation),
            fix: None,
            fix_unavailable_reason: None,
        }
    }

    pub fn diagnostic(diagnostic: AuthoredDiagnostic) -> Self {
        Self {
            file: diagnostic.file,
            line: diagnostic.line,
            column: diagnostic.column,
            code: diagnostic.code,
            message: diagnostic.message,
            fault: diagnostic.fault,
            engine: NATIVE_ENGINE,
            end: diagnostic.end,
            remediation: Some(diagnostic.remediation),
            fix: diagnostic.fix.map(|edit| LintEditRow {
                start: edit.start,
                end: edit.end,
                replacement: edit.replacement,
            }),
            fix_unavailable_reason: diagnostic.fix_unavailable_reason,
        }
    }

    pub fn suppression(finding: SuppressionFinding) -> Self {
        Self {
            file: finding.file,
            line: offset(finding.line),
            column: 1,
            code: SUPPRESSION_CODE.to_owned(),
            message: finding.message,
            fault: finding.fault,
            engine: NATIVE_ENGINE,
            end: None,
            remediation: Some(finding.remediation.to_owned()),
            fix: finding.fix.map(|(start, end)| LintEditRow {
                start: offset(start),
                end: offset(end),
                replacement: String::new(),
            }),
            fix_unavailable_reason: finding.fix_unavailable_reason.map(str::to_owned),
        }
    }
}

/// The lint model classes violations are projected as.
#[derive(Debug)]
pub struct ViolationClasses<'py> {
    pub(crate) violation: Bound<'py, PyAny>,
    pub(crate) edit: Bound<'py, PyAny>,
    pub(crate) severity: Bound<'py, PyAny>,
}

impl<'py> ViolationClasses<'py> {
    pub fn new(py: Python<'py>) -> PyResult<Self> {
        Ok(Self {
            violation: record_class(py, RecordId::LintViolation)?,
            edit: record_class(py, RecordId::LintEdit)?,
            severity: record_class(py, RecordId::LintSeverity)?,
        })
    }

    /// `row` as a `LintViolation` against `file_path`.
    pub fn violation(
        &self,
        file_path: &Bound<'py, PyAny>,
        row: &LintViolationRow,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = file_path.py();
        let none = || py.None().into_bound(py);
        let text = |value: &Option<String>| {
            value
                .as_deref()
                .map_or_else(none, |text| PyString::new(py, text).into_any())
        };
        let code: Bound<'py, PyAny> = PyString::new(py, &row.code).into_any();
        let fix: Bound<'py, PyAny> = match &row.fix {
            Some(edit) => call_with(
                &self.edit,
                &[
                    ("file_path", file_path.clone()),
                    ("code", code.clone()),
                    ("start", edit.start.into_pyobject(py)?.into_any()),
                    ("end", edit.end.into_pyobject(py)?.into_any()),
                    (
                        "replacement",
                        PyString::new(py, &edit.replacement).into_any(),
                    ),
                ],
            )?,
            None => none(),
        };
        let (end_line, end_column) = match row.end {
            Some((line, column)) => (
                line.into_pyobject(py)?.into_any(),
                column.into_pyobject(py)?.into_any(),
            ),
            None => (none(), none()),
        };
        let severity: &str = if row.fault {
            FAULT_SEVERITY
        } else {
            WARNING_SEVERITY
        };
        call_with(
            &self.violation,
            &[
                ("file_path", file_path.clone()),
                ("line", row.line.into_pyobject(py)?.into_any()),
                ("column", row.column.into_pyobject(py)?.into_any()),
                ("code", code),
                ("message", PyString::new(py, &row.message).into_any()),
                ("severity", self.severity.getattr(severity)?),
                ("engine", PyString::new(py, row.engine).into_any()),
                ("end_line", end_line),
                ("end_column", end_column),
                ("remediation", text(&row.remediation)),
                ("fix", fix),
                ("fix_unavailable_reason", text(&row.fix_unavailable_reason)),
            ],
        )
    }
}

/// Every linted file, its sorted violations and the files expansion skipped.
#[derive(Debug)]
pub struct LintOutcome {
    /// The linted files in the order they were given.
    pub files: Vec<LintFile>,
    pub violations: Vec<LintViolationRow>,
    /// Skipped files by index, with why they could not be expanded.
    pub unexpandable: Vec<(usize, String)>,
}

impl LintOutcome {
    /// This outcome as a `LintRunResult`.
    pub fn record<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let paths: Vec<Bound<'py, PyAny>> = self
            .files
            .iter()
            .map(|file| Ok(file.path.clone().into_pyobject(py)?.into_any()))
            .collect::<PyResult<_>>()?;
        let classes: ViolationClasses<'py> = ViolationClasses::new(py)?;
        let violations: Vec<Bound<'py, PyAny>> = self
            .violations
            .iter()
            .map(|row| classes.violation(&paths[row.file], row))
            .collect::<PyResult<_>>()?;
        let texts: Bound<'py, PyDict> = PyDict::new(py);
        for (path, file) in paths.iter().zip(&self.files) {
            texts.set_item(path, &file.contents)?;
        }
        let unexpandable: Bound<'py, PyDict> = PyDict::new(py);
        for (file, message) in &self.unexpandable {
            unexpandable.set_item(&paths[*file], message)?;
        }
        new_record_with(
            py,
            RecordId::LintRunResult,
            &[
                (
                    "files_checked",
                    self.files.len().into_pyobject(py)?.into_any(),
                ),
                ("violations", PyTuple::new(py, violations)?.into_any()),
                ("formatted_files", PyTuple::empty(py).into_any()),
                ("source_texts", texts.into_any()),
                ("unexpandable", unexpandable.into_any()),
            ],
        )
    }
}

/// Header values parsed natively, compared as their Python projections compare.
#[derive(Debug, Default)]
pub struct NativeHeaderParser;

impl HeaderParser for NativeHeaderParser {
    type Values = AuthoredValue;
    /// A native failure's message.
    type Error = String;

    fn parse(&self, kind: &str, inner: &str) -> ParseOutcome<AuthoredValue, String> {
        let parsed = match catch_compiler_panic(|| Ok(parse_one(inner))) {
            Ok(parsed) => parsed,
            Err(message) => {
                return ParseOutcome::Raised(
                    message.clone(),
                    ParseFailure {
                        message,
                        help: None,
                        position: None,
                    },
                );
            }
        };
        let (value, _, error) = parsed;
        if let Some(value) = value {
            return ParseOutcome::Parsed(value);
        }
        let error: String = error.unwrap_or_else(|| "None".to_owned());
        let quote: Option<QuoteFailure> = quote_failure(&error, inner, (1, 1));
        let (detail, help, _) = header_failure(&error, inner, None);
        let failure: ParseFailure = ParseFailure {
            message: format!(
                "{kind}(...) in '{SENTINEL_PATH}' contains invalid SQLBuild header syntax: {detail}"
            ),
            help,
            position: quote.map(|quote| quote.position),
        };
        ParseOutcome::Rejected(failure)
    }

    fn description(&self, values: &AuthoredValue) -> Result<Option<String>, String> {
        Ok(match map_value(values, DESCRIPTION_KEY) {
            Some(AuthoredValue::String(text)) => Some(text.clone()),
            Some(AuthoredValue::BareWord(word)) if header_word_value(word) == HeaderWord::Text => {
                Some(word.clone())
            }
            _ => None,
        })
    }

    fn same(&self, left: &AuthoredValue, right: &AuthoredValue) -> Result<bool, String> {
        Ok(python_equal(left, right))
    }
}

/// Why a project's rules configuration is invalid.
#[derive(Debug)]
pub enum InvalidRulesConfig {
    /// The configuration could not be read: a `RulesError`.
    Unreadable(String),
    /// It uses a setting SQLBuild rejects: a `RulesConfigError` with help.
    Forbidden { message: String, help: String },
}

impl InvalidRulesConfig {
    pub(crate) fn into_pyerr(self, py: Python<'_>) -> PyErr {
        match self {
            Self::Unreadable(message) => rules_error(py, ErrorId::Rules, message, None),
            Self::Forbidden { message, help } => {
                rules_error(py, ErrorId::RulesConfig, message, Some(help))
            }
        }
    }
}
