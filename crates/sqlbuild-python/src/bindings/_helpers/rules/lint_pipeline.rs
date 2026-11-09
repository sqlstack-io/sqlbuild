//! The lint pipeline over native data: header rules, expanded bodies, native SQL lint and suppressions.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::{Bound, IntoPyObject, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use sqlbuild_analysis::sql_macros::main::parse_sql_macro::parse_sql_macro;
use sqlbuild_analysis::sql_macros::main::sql_macro_body_form::sql_macro_body_form;
use sqlbuild_analysis::sql_macros::models::{SqlMacroForm, SqlMacroParameter};
use sqlbuild_core::panics::main::catch_compiler_panic::catch_compiler_panic;
use sqlbuild_core::text::main::python_isspace::python_isspace;
use sqlbuild_core::text::main::python_str_repr::python_str_repr;
use sqlbuild_core::text::main::python_strip::python_strip;
use sqlbuild_rules::dsl_headers::main::declares_table_returns::declares_table_returns;
use sqlbuild_rules::dsl_headers::main::measurement_body_ranges::measurement_body_ranges;
use sqlbuild_rules::dsl_headers::main::scan_headers::scan_headers;
use sqlbuild_rules::dsl_headers::main::sql_body_ranges::sql_body_ranges;
use sqlbuild_rules::dsl_headers::models::{
    HeaderRulesConfig, HeaderSeverity, HeaderSpan, HeaderViolation,
};
use sqlbuild_rules::lint_text::constants::SUPPRESSION_CODE;
use sqlbuild_rules::lint_text::main::defined_cte_names::defined_cte_names;
use sqlbuild_rules::lint_text::main::dependency_sites::dependency_sites;
use sqlbuild_rules::lint_text::main::first_header_only::first_header_only;
use sqlbuild_rules::lint_text::main::frame_sql_macro_expression::frame_sql_macro_expression;
use sqlbuild_rules::lint_text::main::lint_sql_bodies::lint_sql_bodies;
use sqlbuild_rules::lint_text::main::named_declaration_role_kind::named_declaration_role_kind;
use sqlbuild_rules::lint_text::main::neutralize_audit_parameters::neutralize_audit_parameters;
use sqlbuild_rules::lint_text::main::neutralize_context_interpolation::neutralize_context_interpolation;
use sqlbuild_rules::lint_text::main::neutralize_sql_macro_parameters::neutralize_sql_macro_parameters;
use sqlbuild_rules::lint_text::main::neutralize_test_parameters::neutralize_test_parameters;
use sqlbuild_rules::lint_text::main::preparation::prepare;
use sqlbuild_rules::lint_text::main::reject_inline_suppressions::reject_inline_suppressions;
use sqlbuild_rules::lint_text::models::{
    AuthoredDiagnostic, ExpansionSpan, FramedExpression, InterpolationSite, LintSqlBody,
    LintSqlOptions, Neutralized, SuppressionFinding,
};
use sqlbuild_rules::lint_text::types::PreparedSql;
use sqlbuild_rules::project_tree::main::project_real_path::project_real_path;
use sqlbuild_rules::project_tree::main::project_real_paths::project_real_paths;
use sqlclean::sql_lint::main::fill_finding_path::fill_finding_path;

use crate::bindings::_helpers::boundary::panics::value_error;
use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::construct::{new_record_with, record_class};
use crate::bindings::_helpers::records::fields::field as record_field;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};
use crate::bindings::_helpers::render::authored_sql::SqlExpansionInputs;
use crate::bindings::_helpers::render::authored_sql::{AuthoredExpansion, expand_with_inputs};
use crate::bindings::_helpers::render::macro_call_rows::SpanRow;
use crate::bindings::_helpers::rules::dsl_headers::header_violations;
use crate::bindings::_helpers::rules::native_header_parser::{
    FUNCTION_KIND, HeaderFailure, NativeHeaderParser, external_identifiers,
};
use crate::bindings::types::CompilerDetach;

pub(crate) const NATIVE_ENGINE: &str = "native";
const HEADER_ENGINE: &str = "sqlbuild";
const HOOKS_FOLDER: &str = "hooks";
const AUDIT_KIND: &str = "audit";
const SINGULAR_AUDIT_KIND: &str = "singular_audit";
const SQL_HOOK_KIND: &str = "sql_hook";
const CEREMONIAL_KINDS: [&str; 2] = ["TEST", "SCENARIO"];
const SQL_MACRO_KIND: &str = "MACRO";
const SQL_TEST_KIND: &str = "TEST";
const INTERPOLATION_MARKERS: [&str; 2] = ["@", "${"];
const FAULT_SEVERITY: &str = "FAULT";
const WARNING_SEVERITY: &str = "WARNING";

/// Resolved lint settings; the defaults are `LintConfig`'s.
#[derive(Clone, Debug)]
pub(crate) struct LintSettings {
    pub(crate) native_enabled: bool,
    pub(crate) header_rules_enabled: bool,
    pub(crate) header_rules: HeaderRulesConfig,
    pub(crate) sql: LintSqlOptions,
    /// Declared keys of each relation, in declaration order.
    pub(crate) relation_keys: Vec<(String, Vec<Vec<String>>)>,
}

impl Default for LintSettings {
    fn default() -> Self {
        Self {
            native_enabled: true,
            header_rules_enabled: true,
            header_rules: HeaderRulesConfig {
                line_width: 100,
                max_description_lines: 10,
            },
            sql: LintSqlOptions {
                dialect: "generic".to_owned(),
                enabled_rules: None,
                ignored_rules: Vec::new(),
                max_literal_length: 100,
                max_ranking_order_by: 5,
            },
            relation_keys: Vec::new(),
        }
    }
}

impl LintSettings {
    /// The settings a `LintConfig` record holds.
    pub(crate) fn from_record(config: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut relation_keys: Vec<(String, Vec<Vec<String>>)> = Vec::new();
        for item in record_field(config, "relation_keys")?
            .call_method0("items")?
            .try_iter()?
        {
            relation_keys.push(item?.extract()?);
        }
        Ok(Self {
            native_enabled: record_field(config, "native_enabled")?.is_truthy()?,
            header_rules_enabled: record_field(config, "header_rules_enabled")?.is_truthy()?,
            header_rules: HeaderRulesConfig {
                line_width: record_field(config, "line_width")?.extract()?,
                max_description_lines: record_field(config, "max_description_lines")?.extract()?,
            },
            sql: LintSqlOptions {
                dialect: record_field(config, "dialect")?.extract()?,
                enabled_rules: record_field(config, "enabled_native_rules")?.extract()?,
                ignored_rules: record_field(config, "ignored_native_rules")?.extract()?,
                max_literal_length: record_field(config, "max_literal_length")?.extract()?,
                max_ranking_order_by: record_field(config, "max_ranking_order_by")?.extract()?,
            },
            relation_keys,
        })
    }

    /// The declared keys of `relation`, when it declares any.
    fn keys_of(&self, relation: &str) -> Option<&Vec<Vec<String>>> {
        self.relation_keys
            .iter()
            .find(|(name, _)| name == relation)
            .map(|(_, keys)| keys)
            .filter(|keys| !keys.is_empty())
    }
}

/// One authored SQL file and its text.
#[derive(Clone, Debug)]
pub(crate) struct LintFile {
    pub(crate) path: PathBuf,
    pub(crate) contents: String,
}

/// A compile's authored-to-expanded SQL for one file, reused instead of expanding again.
#[derive(Clone, Debug)]
pub(crate) struct CompiledExpansion {
    pub(crate) authored_sql: String,
    pub(crate) expanded_sql: String,
    pub(crate) passes: Vec<Vec<ExpansionSpan>>,
}

impl CompiledExpansion {
    /// The expansion a `CompiledSqlExpansion` record holds.
    pub(crate) fn from_record(record: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut passes: Vec<Vec<ExpansionSpan>> = Vec::new();
        for pass in record_field(record, "passes")?.try_iter()? {
            let mut spans: Vec<ExpansionSpan> = Vec::new();
            for span in pass?.try_iter()? {
                let span: Bound<'_, PyAny> = span?;
                spans.push(ExpansionSpan {
                    source_start: record_field(&span, "source_start")?.extract()?,
                    source_end: record_field(&span, "source_end")?.extract()?,
                    output_start: record_field(&span, "output_start")?.extract()?,
                    output_end: record_field(&span, "output_end")?.extract()?,
                });
            }
            passes.push(spans);
        }
        Ok(Self {
            authored_sql: record_field(record, "authored_sql")?.extract()?,
            expanded_sql: record_field(record, "expanded_sql")?.extract()?,
            passes,
        })
    }
}

/// A compile's expansion of one file: owned, or the compile's shared texts read when linted.
pub(crate) enum CompiledSource {
    Owned(CompiledExpansion),
    Shared {
        authored_sql: Arc<str>,
        expanded_sql: Arc<str>,
        passes: Vec<Vec<ExpansionSpan>>,
    },
}

impl CompiledSource {
    /// The owned expansion, readable without the GIL.
    fn owned(&self) -> Option<&CompiledExpansion> {
        match self {
            Self::Owned(expansion) => Some(expansion),
            Self::Shared { .. } => None,
        }
    }

    /// The shared texts as an expansion of their own, built only while one file is linted.
    fn shared(&self) -> Option<CompiledExpansion> {
        match self {
            Self::Owned(_) => None,
            Self::Shared {
                authored_sql,
                expanded_sql,
                passes,
            } => Some(CompiledExpansion {
                authored_sql: authored_sql.to_string(),
                expanded_sql: expanded_sql.to_string(),
                passes: passes.clone(),
            }),
        }
    }
}

/// Expansion span rows as lint spans.
pub(crate) fn span_pass(rows: &[SpanRow]) -> Vec<ExpansionSpan> {
    rows.iter()
        .map(|row| ExpansionSpan {
            source_start: offset(row.source_start),
            source_end: offset(row.source_end),
            output_start: offset(row.output_start),
            output_end: offset(row.output_end),
        })
        .collect()
}

fn offset(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// A proposed authored-source replacement in the violation's file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LintEditRow {
    pub(crate) start: i64,
    pub(crate) end: i64,
    pub(crate) replacement: String,
}

/// One lint violation against the file at index `file` of its run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LintViolationRow {
    pub(crate) file: usize,
    pub(crate) line: i64,
    pub(crate) column: i64,
    pub(crate) code: String,
    pub(crate) message: String,
    pub(crate) fault: bool,
    pub(crate) engine: &'static str,
    pub(crate) end: Option<(i64, i64)>,
    pub(crate) remediation: Option<String>,
    pub(crate) fix: Option<LintEditRow>,
    pub(crate) fix_unavailable_reason: Option<String>,
}

impl LintViolationRow {
    pub(crate) fn header(file: usize, violation: HeaderViolation) -> Self {
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

    pub(crate) fn diagnostic(diagnostic: AuthoredDiagnostic) -> Self {
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

    pub(crate) fn suppression(finding: SuppressionFinding) -> Self {
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
pub(crate) struct ViolationClasses<'py> {
    violation: Bound<'py, PyAny>,
    edit: Bound<'py, PyAny>,
    severity: Bound<'py, PyAny>,
}

impl<'py> ViolationClasses<'py> {
    pub(crate) fn new(py: Python<'py>) -> PyResult<Self> {
        Ok(Self {
            violation: record_class(py, RecordId::LintViolation)?,
            edit: record_class(py, RecordId::LintEdit)?,
            severity: record_class(py, RecordId::LintSeverity)?,
        })
    }

    /// `row` as a `LintViolation` against `file_path`.
    pub(crate) fn violation(
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
pub(crate) struct LintOutcome {
    /// The linted files in the order they were given.
    pub(crate) files: Vec<LintFile>,
    pub(crate) violations: Vec<LintViolationRow>,
    /// Skipped files by index, with why they could not be expanded.
    pub(crate) unexpandable: Vec<(usize, String)>,
}

impl LintOutcome {
    /// This outcome as a `LintRunResult`.
    pub(crate) fn record<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
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

/// What one lint run reads besides its files.
pub(crate) struct LintRequest<'a> {
    pub(crate) project_dir: &'a Path,
    pub(crate) settings: &'a LintSettings,
    /// Resolved paths of models whose output columns a dynamic `*` proves.
    pub(crate) dynamic_output_paths: &'a HashSet<PathBuf>,
    pub(crate) compiled_expansions: &'a HashMap<PathBuf, CompiledSource>,
    /// The expansion inputs; without them bodies are not expanded or SQL-linted.
    pub(crate) context: Option<&'a SqlExpansionInputs>,
    pub(crate) skip_unexpandable: bool,
    /// Set once the run's result is no longer needed.
    pub(crate) stop: Option<&'a AtomicBool>,
}

/// `path` resolved through the filesystem, or unchanged when it cannot be.
pub(crate) fn resolved(path: &Path) -> PathBuf {
    project_real_path(path)
}

/// `given` filtered to the selection, or the project's lint files read from disk.
pub(crate) fn lint_sources(
    project_dir: &Path,
    selected: Option<&BTreeSet<PathBuf>>,
    given: Option<Vec<LintFile>>,
) -> PyResult<Vec<LintFile>> {
    if let Some(given) = given {
        let Some(paths) = selected else {
            return Ok(given);
        };
        let files: Vec<&Path> = given.iter().map(|file| file.path.as_path()).collect();
        let keep: Vec<bool> = project_real_paths(&files)
            .iter()
            .map(|path| paths.contains(path))
            .collect();
        return Ok(given
            .into_iter()
            .zip(keep)
            .filter_map(|(file, kept)| kept.then_some(file))
            .collect());
    }
    let collected: Vec<(PathBuf, String)> =
        sqlbuild_core::format_command::main::collect_lint_files::collect_lint_files(
            project_dir,
            selected,
        )
        .map_err(|error| pyo3::exceptions::PyOSError::new_err(error.to_string()))?;
    Ok(collected
        .into_iter()
        .map(|(path, contents)| LintFile { path, contents })
        .collect())
}

/// Raise `LintStoppedError` once `stop` is set.
fn check_stop(py: Python<'_>, stop: Option<&AtomicBool>) -> PyResult<()> {
    if stop.is_some_and(|stop| stop.load(Ordering::Acquire)) {
        return Err(PyErr::from_value(
            error_class(py, ErrorId::LintStopped)?.call0()?,
        ));
    }
    Ok(())
}

/// Where one body's expanded SQL comes from.
enum BodySource {
    /// The compile's expansion of the same authored SQL.
    Compiled(String, Vec<Vec<ExpansionSpan>>),
    /// Text without interpolation, linted as written.
    Plain,
    /// Text that user macros expand.
    Expand,
}

/// One authored body, neutralized and ready to expand.
struct PlannedBody {
    start: usize,
    input: String,
    sites: Vec<InterpolationSite>,
    source: BodySource,
    /// The form of a SQL macro file's body, or `Expression` for a scalar function's, which decides the statement lint parses it as.
    macro_form: Option<SqlMacroForm>,
}

/// How a file's bodies expand, shared by each of its bodies.
struct BodyTraits {
    external_identifiers: Vec<String>,
    ceremonial: bool,
    dynamic_star: bool,
}

/// One file's header violations and, when expanding, its planned bodies.
struct FilePlan {
    file: usize,
    header_violations: Vec<HeaderViolation>,
    expansion: Option<PlannedBodies>,
}

/// A file's body traits and planned bodies, or why its headers could not be read.
type PlannedBodies = Result<(BodyTraits, Vec<PlannedBody>), HeaderFailure>;

/// Why a file could not be planned.
enum PlanFailure {
    OutsideProject { path: PathBuf },
    Native(String),
}

/// One expanded body awaiting native preparation.
struct PendingBody {
    start: usize,
    input: String,
    expanded: String,
    passes: Vec<Vec<ExpansionSpan>>,
    sites: Vec<InterpolationSite>,
    macro_form: Option<SqlMacroForm>,
    expression_frame: Option<(i64, i64)>,
}

/// One file's expanded bodies and the expansion failure that stopped it, if any.
struct ExpandedFile {
    file: usize,
    traits: BodyTraits,
    bodies: Vec<PendingBody>,
    failure: Option<PyErr>,
}

fn path_parts(relative: &Path) -> Vec<String> {
    relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect()
}

/// Lint `files`: header rules, then expanded bodies through the native SQL engine.
pub(crate) fn lint(
    py: Python<'_>,
    request: &LintRequest<'_>,
    files: Vec<LintFile>,
) -> PyResult<LintOutcome> {
    let mut order: Vec<usize> = (0..files.len()).collect();
    order.sort_by(|left, right| files[*left].path.cmp(&files[*right].path));
    let expanding: bool = request.context.is_some();
    let plans: Vec<Result<FilePlan, PlanFailure>> = py
        .compiler_detach(|| Ok(plan_files(request, &files, &order, expanding)))
        .map_err(value_error)?;
    let mut header_rows: Vec<LintViolationRow> = Vec::new();
    let mut expanded_files: Vec<ExpandedFile> = Vec::new();
    let mut scan_failure: Option<PyErr> = None;
    for plan in plans {
        let plan: FilePlan = match plan {
            Ok(plan) => plan,
            Err(failure) => {
                scan_failure = Some(plan_error(request.project_dir, failure));
                break;
            }
        };
        header_rows.extend(
            plan.header_violations
                .into_iter()
                .map(|violation| LintViolationRow::header(plan.file, violation)),
        );
        let (Some(expansion), Some(context)) = (plan.expansion, request.context) else {
            continue;
        };
        check_stop(py, request.stop)?;
        let (traits, bodies) = match expansion {
            Ok(planned) => planned,
            Err(failure) => {
                scan_failure = Some(failure.into_pyerr(py));
                break;
            }
        };
        let file: &LintFile = &files[plan.file];
        let (traits, bodies) = match shared_plan(request, file) {
            Some(Ok(planned)) => planned,
            Some(Err(failure)) => {
                scan_failure = Some(failure.into_pyerr(py));
                break;
            }
            None => (traits, bodies),
        };
        let expanded: ExpandedFile =
            match expand_bodies(py, context, file, (plan.file, traits, bodies)) {
                Ok(expanded) => expanded,
                Err(error) if error.is_instance_of::<PyException>(py) => {
                    scan_failure = Some(error);
                    break;
                }
                Err(error) => return Err(error),
            };
        let failed: bool = expanded.failure.is_some();
        expanded_files.push(expanded);
        if failed && !request.skip_unexpandable {
            break;
        }
    }
    check_stop(py, request.stop)?;
    let (bodies, unexpandable) = prepared_bodies(py, request, &files, expanded_files)?;
    if let Some(error) = scan_failure {
        return Err(error);
    }
    let mut rows: Vec<LintViolationRow> = header_rows;
    if !bodies.is_empty() && request.settings.native_enabled {
        check_stop(py, request.stop)?;
        let options: &LintSqlOptions = &request.settings.sql;
        let diagnostics: Vec<AuthoredDiagnostic> = py
            .compiler_detach(|| {
                let texts: Vec<&str> = files.iter().map(|file| file.contents.as_str()).collect();
                Ok(lint_sql_bodies(&bodies, &texts, options))
            })
            .map_err(value_error)?;
        rows.extend(diagnostics.into_iter().map(LintViolationRow::diagnostic));
        check_stop(py, request.stop)?;
    }
    let violations: Vec<LintViolationRow> =
        with_directive_findings(&files, request.project_dir, rows)?;
    Ok(LintOutcome {
        files,
        violations,
        unexpandable,
    })
}

/// A file's bodies planned again against its shared compile expansion, read only for this file.
fn shared_plan(request: &LintRequest<'_>, file: &LintFile) -> Option<PlannedBodies> {
    let compiled: CompiledExpansion = request.compiled_expansions.get(&file.path)?.shared()?;
    let Ok(relative) = file.path.strip_prefix(request.project_dir) else {
        return None;
    };
    let parts: Vec<String> = path_parts(relative);
    let text: Vec<char> = file.contents.chars().collect();
    let headers: Vec<HeaderSpan> = scan_headers(&text, first_header_only(&parts));
    Some(plan_bodies(
        request,
        file,
        (&text, &headers),
        (&parts, Some(&compiled)),
    ))
}

/// Each file's headers, header violations and bodies in path order, up to the first failure.
fn plan_files(
    request: &LintRequest<'_>,
    files: &[LintFile],
    order: &[usize],
    expanding: bool,
) -> Vec<Result<FilePlan, PlanFailure>> {
    let mut plans: Vec<Result<FilePlan, PlanFailure>> = Vec::new();
    for &index in order {
        let plan: Result<FilePlan, PlanFailure> =
            plan_file(request, &files[index], index, expanding);
        let failed: bool = plan.is_err();
        plans.push(plan);
        if failed {
            break;
        }
    }
    plans
}

fn plan_file(
    request: &LintRequest<'_>,
    file: &LintFile,
    index: usize,
    expanding: bool,
) -> Result<FilePlan, PlanFailure> {
    let Ok(relative) = file.path.strip_prefix(request.project_dir) else {
        return Err(PlanFailure::OutsideProject {
            path: file.path.clone(),
        });
    };
    let parts: Vec<String> = path_parts(relative);
    let text: Vec<char> = file.contents.chars().collect();
    let headers: Vec<HeaderSpan> = scan_headers(&text, first_header_only(&parts));
    let header_violations: Vec<HeaderViolation> = if request.settings.header_rules_enabled {
        header_violations(&text, &headers, request.settings.header_rules)
            .map_err(PlanFailure::Native)?
    } else {
        Vec::new()
    };
    let compiled: Option<&CompiledExpansion> = request
        .compiled_expansions
        .get(&file.path)
        .and_then(CompiledSource::owned);
    let expansion =
        expanding.then(|| plan_bodies(request, file, (&text, &headers), (&parts, compiled)));
    Ok(FilePlan {
        file: index,
        header_violations,
        expansion,
    })
}

/// A file's bodies, neutralized and matched against the compile's expansion.
fn plan_bodies(
    request: &LintRequest<'_>,
    file: &LintFile,
    (text, headers): (&[char], &[HeaderSpan]),
    (parts, compiled): (&[String], Option<&CompiledExpansion>),
) -> Result<(BodyTraits, Vec<PlannedBody>), HeaderFailure> {
    let kind: Option<&str> = named_declaration_role_kind(parts);
    let in_hook_directory: bool = parts.first().is_some_and(|part| part == HOOKS_FOLDER);
    let dynamic_star: bool = !request.dynamic_output_paths.is_empty()
        && request.dynamic_output_paths.contains(&resolved(&file.path));
    let external: Vec<String> = external_identifiers(text, headers, &NativeHeaderParser)?;
    let sql_macro: bool = headers.iter().any(|header| header.kind == SQL_MACRO_KIND);
    let mut ceremonial: bool = headers
        .iter()
        .any(|header| CEREMONIAL_KINDS.contains(&header.kind.as_str()));
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    if matches!(kind, Some(AUDIT_KIND | SINGULAR_AUDIT_KIND)) {
        ranges = measurement_body_ranges(text);
    }
    if ranges.is_empty() {
        ranges = sql_body_ranges(text, headers);
    }
    let sql_test: bool = headers.iter().any(|header| header.kind == SQL_TEST_KIND);
    let parameters: Vec<SqlMacroParameter> = if sql_macro {
        sql_macro_parameters(file)
    } else {
        Vec::new()
    };
    let neutralize_macro = |body: &str| neutralize_sql_macro_parameters(body, &parameters);
    let other: Option<fn(&str) -> Neutralized> = if sql_test {
        Some(neutralize_test_parameters)
    } else {
        neutralizer(&BodyRole {
            kind,
            in_project: true,
            in_hook_directory,
        })
    };
    let neutralize: Option<&dyn Fn(&str) -> Neutralized> = if sql_macro {
        Some(&neutralize_macro)
    } else {
        other
            .as_ref()
            .map(|function| function as &dyn Fn(&str) -> Neutralized)
    };
    let mut bodies: Vec<PlannedBody> = ranges
        .into_iter()
        .map(|range| plan_body(text, range, neutralize, compiled))
        .collect();
    if sql_macro {
        for body in &mut bodies {
            let form: SqlMacroForm = sql_macro_body_form(&body.input);
            ceremonial = ceremonial || form == SqlMacroForm::CteList;
            body.macro_form = Some(form);
        }
    } else if headers.iter().any(|header| {
        header.kind == FUNCTION_KIND && !declares_table_returns(&text[header.start..header.end])
    }) {
        for body in &mut bodies {
            if sql_macro_body_form(&body.input) == SqlMacroForm::Expression {
                body.macro_form = Some(SqlMacroForm::Expression);
            }
        }
    }
    Ok((
        BodyTraits {
            external_identifiers: external,
            ceremonial,
            dynamic_star,
        },
        bodies,
    ))
}

/// A lint file's path classification.
pub(crate) struct BodyRole<'a> {
    pub(crate) kind: Option<&'a str>,
    pub(crate) in_project: bool,
    pub(crate) in_hook_directory: bool,
}

/// How a body's interpolation is neutralized before expansion: audit parameters, or hook context.
fn neutralizer(role: &BodyRole<'_>) -> Option<fn(&str) -> Neutralized> {
    if role.in_project && role.kind == Some(AUDIT_KIND) {
        Some(neutralize_audit_parameters)
    } else if role.in_hook_directory || (role.in_project && role.kind == Some(SQL_HOOK_KIND)) {
        Some(neutralize_context_interpolation)
    } else {
        None
    }
}

/// One authored body expanded and prepared for native SQL lint, outside a lint run.
pub(crate) struct SingleBody {
    pub(crate) body: LintSqlBody,
    pub(crate) body_end: usize,
    pub(crate) dependency_relations: Vec<(String, String)>,
}

/// One body to expand and prepare outside a lint run.
pub(crate) struct SingleBodyRequest<'a, 'py> {
    pub(crate) context: &'a SqlExpansionInputs,
    pub(crate) file_path: &'a Bound<'py, PyAny>,
    pub(crate) contents: &'a str,
    pub(crate) range: (usize, usize),
    pub(crate) role: BodyRole<'a>,
    pub(crate) dialect: &'a str,
}

/// Expand and prepare the body at `range` of `contents` as `role` reads it.
pub(crate) fn prepare_single_body(request: &SingleBodyRequest<'_, '_>) -> PyResult<SingleBody> {
    let SingleBodyRequest {
        context,
        file_path,
        contents,
        range,
        role,
        dialect,
    } = request;
    let context: &SqlExpansionInputs = context;
    let file_path: &Bound<'_, PyAny> = file_path;
    let range: (usize, usize) = *range;
    let dialect: &str = dialect;
    let py: Python<'_> = file_path.py();
    let text: Vec<char> = contents.chars().collect();
    let neutralize: Option<fn(&str) -> Neutralized> = neutralizer(role);
    let planned: PlannedBody = plan_body(
        &text,
        range,
        neutralize
            .as_ref()
            .map(|function| function as &dyn Fn(&str) -> Neutralized),
        None,
    );
    let (expanded, passes) = match planned.source {
        BodySource::Expand => {
            let expansion: AuthoredExpansion =
                expand_with_inputs(context, &planned.input, file_path)
                    .map_err(|error| expansion_error(file_path, error))?;
            (
                expansion.sql,
                expansion
                    .passes
                    .iter()
                    .map(|rows| span_pass(rows))
                    .collect(),
            )
        }
        BodySource::Compiled(expanded, passes) => (expanded, passes),
        BodySource::Plain => (planned.input.clone(), Vec::new()),
    };
    let pending: PendingBody = PendingBody {
        start: planned.start,
        input: planned.input,
        expanded,
        passes,
        sites: planned.sites,
        macro_form: None,
        expression_frame: None,
    };
    let prior: Vec<usize> = prior_sites(&pending.sites);
    let prepared: Option<PreparedSql> = py
        .compiler_detach(|| prepare(&pending.expanded, &pending.input, &prior, dialect))
        .map_err(value_error)?;
    let Some(prepared) = prepared else {
        return Err(not_prepared(py, &file_path.extract::<PathBuf>()?)?);
    };
    let traits: BodyTraits = BodyTraits {
        external_identifiers: Vec::new(),
        ceremonial: false,
        dynamic_star: false,
    };
    let (body, dependency_relations) = lint_body((0, &traits), pending, prepared)?;
    Ok(SingleBody {
        body,
        body_end: range.1,
        dependency_relations,
    })
}

/// The `NativeLintError` of a body native preparation declined.
fn not_prepared(py: Python<'_>, path: &Path) -> PyResult<PyErr> {
    Ok(PyErr::from_value(
        error_class(py, ErrorId::NativeLint)?.call1((format!(
            "{} could not be prepared for native SQL lint",
            path.display()
        ),))?,
    ))
}

/// The declared parameters of a SQL macro file, or none when its header does not parse.
fn sql_macro_parameters(file: &LintFile) -> Vec<SqlMacroParameter> {
    let name: String = file
        .path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    parse_sql_macro(&name, &file.path.to_string_lossy(), &file.contents)
        .map(|definition| definition.parameters)
        .unwrap_or_default()
}

fn plan_body(
    text: &[char],
    (mut start, end): (usize, usize),
    neutralize: Option<&dyn Fn(&str) -> Neutralized>,
    compiled: Option<&CompiledExpansion>,
) -> PlannedBody {
    let body: String = text[start..end].iter().collect();
    let (mut input, sites): (String, Vec<InterpolationSite>) = match neutralize {
        Some(neutralize) => {
            let neutralized: Neutralized = neutralize(&body);
            (neutralized.text, neutralized.sites)
        }
        None => (body.clone(), Vec::new()),
    };
    let source: BodySource = match compiled {
        Some(compiled) if python_strip(&body) == compiled.authored_sql => {
            let leading: usize =
                body.chars().count() - body.trim_start_matches(python_isspace).chars().count();
            start += leading;
            input.clone_from(&compiled.authored_sql);
            BodySource::Compiled(compiled.expanded_sql.clone(), compiled.passes.clone())
        }
        _ if INTERPOLATION_MARKERS
            .iter()
            .all(|marker| !input.contains(marker)) =>
        {
            BodySource::Plain
        }
        _ => BodySource::Expand,
    };
    PlannedBody {
        start,
        input,
        sites,
        source,
        macro_form: None,
    }
}

/// The `ValueError` `PurePath.relative_to` raises for a path outside `project_dir`.
pub(crate) fn outside_project(path: &Path, project_dir: &Path) -> PyErr {
    PyValueError::new_err(format!(
        "{} is not in the subpath of {}",
        python_str_repr(&path.to_string_lossy()),
        python_str_repr(&project_dir.to_string_lossy()),
    ))
}

fn plan_error(project_dir: &Path, failure: PlanFailure) -> PyErr {
    match failure {
        PlanFailure::OutsideProject { path } => outside_project(&path, project_dir),
        PlanFailure::Native(message) => value_error(message),
    }
}

/// Expand a file's bodies in order; a compile failure stops the file with its partial work.
fn expand_bodies(
    py: Python<'_>,
    context: &SqlExpansionInputs,
    file: &LintFile,
    (index, traits, planned): (usize, BodyTraits, Vec<PlannedBody>),
) -> PyResult<ExpandedFile> {
    let mut bodies: Vec<PendingBody> = Vec::with_capacity(planned.len());
    let mut failure: Option<PyErr> = None;
    let mut path: Option<Bound<'_, PyAny>> = None;
    for mut body in planned {
        let (expanded, passes) = match std::mem::replace(&mut body.source, BodySource::Plain) {
            BodySource::Compiled(expanded, passes) => (expanded, passes),
            BodySource::Plain => (body.input.clone(), Vec::new()),
            BodySource::Expand => {
                let file_path: &Bound<'_, PyAny> = match &path {
                    Some(path) => path,
                    None => path.insert(file.path.clone().into_pyobject(py)?.into_any()),
                };
                match expand_with_inputs(context, &body.input, file_path) {
                    Ok(AuthoredExpansion { sql, passes }) => {
                        (sql, passes.iter().map(|rows| span_pass(rows)).collect())
                    }
                    Err(error) => {
                        let error: PyErr = expansion_error(file_path, error);
                        if !error
                            .value(py)
                            .is_instance(&error_class(py, ErrorId::ProjectCompile)?)?
                        {
                            return Err(error);
                        }
                        failure = Some(error);
                        break;
                    }
                }
            }
        };
        bodies.push(framed_body(body, (expanded, passes)));
    }
    Ok(ExpandedFile {
        file: index,
        traits,
        bodies,
        failure,
    })
}

/// One expanded body, an expression macro or function body framed as the `SELECT` lint parses it as.
fn framed_body(
    body: PlannedBody,
    (expanded, mut passes): (String, Vec<Vec<ExpansionSpan>>),
) -> PendingBody {
    let (expanded, expression_frame) = if body.macro_form == Some(SqlMacroForm::Expression) {
        let framed: FramedExpression = frame_sql_macro_expression(&expanded);
        passes.push(framed.pass);
        (framed.text, Some(framed.frame))
    } else {
        (expanded, None)
    };
    PendingBody {
        start: body.start,
        input: body.input,
        expanded,
        passes,
        sites: body.sites,
        macro_form: body.macro_form,
        expression_frame,
    }
}

/// A compile-input error while expanding as `ProjectCompileError`; any other error unchanged.
fn expansion_error(file_path: &Bound<'_, PyAny>, error: PyErr) -> PyErr {
    let py: Python<'_> = file_path.py();
    let wrapped = (|| -> PyResult<Option<PyErr>> {
        if !error
            .value(py)
            .is_instance(&error_class(py, ErrorId::CompileInput)?)?
        {
            return Ok(None);
        }
        let message: String = format!(
            "{} could not be expanded, so its SQL cannot be linted: {}",
            file_path.str()?,
            error.value(py).str()?
        );
        Ok(Some(PyErr::from_value(
            error_class(py, ErrorId::ProjectCompile)?.call1((message,))?,
        )))
    })();
    match wrapped {
        Ok(Some(wrapped)) => {
            wrapped.set_cause(py, Some(error));
            wrapped
        }
        Ok(None) => error,
        Err(wrapping_error) => wrapping_error,
    }
}

/// Prepared bodies to lint, and skipped files by index with why they could not expand.
type PreparedBodies = (Vec<LintSqlBody>, Vec<(usize, String)>);

/// Prepare every expanded body natively, then publish bodies or failures in file order.
fn prepared_bodies(
    py: Python<'_>,
    request: &LintRequest<'_>,
    files: &[LintFile],
    expanded_files: Vec<ExpandedFile>,
) -> PyResult<PreparedBodies> {
    let dialect: &str = &request.settings.sql.dialect;
    let prepared: Vec<Result<Option<PreparedSql>, String>> = py
        .compiler_detach(|| {
            Ok(expanded_files
                .iter()
                .flat_map(|file| &file.bodies)
                .map(|body| {
                    let prior: Vec<usize> = prior_sites(&body.sites);
                    catch_compiler_panic(|| prepare(&body.expanded, &body.input, &prior, dialect))
                })
                .collect())
        })
        .map_err(value_error)?;
    let mut prepared: Vec<Option<PreparedSql>> = prepared
        .into_iter()
        .collect::<Result<_, String>>()
        .map_err(value_error)?;
    let mut bodies: Vec<LintSqlBody> = Vec::new();
    let mut unexpandable: Vec<(usize, String)> = Vec::new();
    let mut position: usize = 0;
    for file in expanded_files {
        let mut file_bodies: Vec<LintSqlBody> = Vec::with_capacity(file.bodies.len());
        for body in file.bodies {
            let Some(result) = prepared[position].take() else {
                return Err(not_prepared(py, &files[file.file].path)?);
            };
            position += 1;
            let (mut lint_body, relations) = lint_body((file.file, &file.traits), body, result)?;
            lint_body.relation_keys = relation_keys(request.settings, relations);
            file_bodies.push(lint_body);
        }
        match file.failure {
            None => bodies.extend(file_bodies),
            Some(failure) if request.skip_unexpandable => {
                let message: String = failure.value(py).str()?.extract()?;
                let prefix: String = format!("{} ", files[file.file].path.display());
                unexpandable.push((
                    file.file,
                    message.strip_prefix(&prefix).unwrap_or(&message).to_owned(),
                ));
            }
            Some(failure) => return Err(failure),
        }
    }
    Ok((bodies, unexpandable))
}

/// One prepared body as the native SQL engine lints it.
fn lint_body(
    (file, traits): (usize, &BodyTraits),
    body: PendingBody,
    (lint_text, prepared_sites, mut externally_referenced_ctes): PreparedSql,
) -> PyResult<(LintSqlBody, Vec<(String, String)>)> {
    let dependencies = dependency_sites(&prepared_sites).map_err(value_error)?;
    let mut passes: Vec<Vec<ExpansionSpan>> = Vec::with_capacity(body.passes.len() + 2);
    passes.push(sentinel_spans(&body.sites));
    passes.extend(body.passes);
    passes.push(sentinel_spans(&prepared_sites));
    if body.macro_form == Some(SqlMacroForm::CteList) {
        for name in defined_cte_names(&lint_text) {
            if !externally_referenced_ctes.contains(&name) {
                externally_referenced_ctes.push(name);
            }
        }
    }
    let body = LintSqlBody {
        file,
        body_start: offset(body.start),
        lint_text,
        passes,
        external_identifiers: traits.external_identifiers.clone(),
        dependency_identifiers: dependencies.identifiers,
        externally_referenced_ctes,
        allows_ceremonial_select: traits.ceremonial,
        allows_dynamic_output_star: traits.dynamic_star,
        allows_empty_fixture_star: traits.ceremonial,
        relation_keys: Vec::new(),
        relation_names: relation_names(&dependencies.relations),
        expression_frame: body.expression_frame,
    };
    Ok((body, dependencies.relations))
}

/// `path` relative to `project_dir` with `/` separators, or as given outside the project.
pub(crate) fn project_relative_path(path: &Path, project_dir: &Path) -> String {
    let relative: &Path = path.strip_prefix(project_dir).unwrap_or(path);
    relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<String>>()
        .join("/")
}

/// The relation name each dependency sentinel stands for, without its `kind:` prefix.
pub(crate) fn relation_names(relations: &[(String, String)]) -> Vec<(String, String)> {
    relations
        .iter()
        .map(|(sentinel, identity)| {
            let name: &str = identity
                .split_once(':')
                .map_or(identity.as_str(), |(_, name)| name);
            (sentinel.clone(), name.to_owned())
        })
        .collect()
}

/// The declared keys of each dependency relation that declares any, by its sentinel.
pub(crate) fn relation_keys(
    settings: &LintSettings,
    relations: Vec<(String, String)>,
) -> Vec<(String, Vec<Vec<String>>)> {
    let mut keyed: Vec<(String, Vec<Vec<String>>)> = Vec::new();
    for (sentinel, name) in relations {
        if let Some(keys) = settings.keys_of(&name) {
            keyed.push((sentinel, keys.clone()));
        }
    }
    keyed
}

/// Where each pre-expansion site's sentinel starts in the neutralized body.
fn prior_sites(sites: &[InterpolationSite]) -> Vec<usize> {
    sites.iter().map(|site| site.neutralized_start).collect()
}

fn sentinel_spans(sites: &[InterpolationSite]) -> Vec<ExpansionSpan> {
    sites
        .iter()
        .map(|site| ExpansionSpan {
            source_start: offset(site.original_start),
            source_end: offset(site.original_end),
            output_start: offset(site.neutralized_start),
            output_end: offset(site.neutralized_end),
        })
        .collect()
}

/// `rows` with file paths in their help and a finding per inline directive, sorted by place.
fn with_directive_findings(
    files: &[LintFile],
    project_dir: &Path,
    mut violations: Vec<LintViolationRow>,
) -> PyResult<Vec<LintViolationRow>> {
    let paths: Vec<String> = files
        .iter()
        .map(|file| project_relative_path(&file.path, project_dir))
        .collect();
    let sources: Vec<(&str, &str)> = paths
        .iter()
        .zip(files)
        .map(|(path, file)| (path.as_str(), file.contents.as_str()))
        .collect();
    let findings: Vec<SuppressionFinding> = catch_compiler_panic(|| {
        reject_inline_suppressions(&sources).map_err(|error| error.to_string())
    })
    .map_err(value_error)?;
    for violation in &mut violations {
        if let Some(remediation) = &mut violation.remediation {
            *remediation = fill_finding_path(remediation, &paths[violation.file]);
        }
    }
    violations.extend(findings.into_iter().map(LintViolationRow::suppression));
    violations.sort_by(|left, right| {
        (
            files[left.file].path.to_string_lossy(),
            left.line,
            left.column,
        )
            .cmp(&(
                files[right.file].path.to_string_lossy(),
                right.line,
                right.column,
            ))
    });
    Ok(violations)
}
