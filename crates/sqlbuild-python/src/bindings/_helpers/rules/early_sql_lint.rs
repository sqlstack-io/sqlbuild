//! Speculative SQL checks started once compile inputs are ready, and the lint settings they use.

use std::path::PathBuf;
use std::time::Instant;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::thread::JoinHandle;

use pyo3::exceptions::{PyException, PyRuntimeError};
use pyo3::prelude::{
    Bound, IntoPyObject, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyRefMut,
    PyResult, Python,
};
use pyo3::types::PyTuple;
use pyo3::{pyclass, pymethods};
use sqlbuild_rules::lint_text::models::LintSqlOptions;
use sqlbuild_rules::rule_cache::main::rules_bulk_cache_path::rules_bulk_cache_path;
use sqlbuild_rules::rules::main::selected_sql_codes::selected_sql_codes;

use crate::bindings::_helpers::boundary::panics::compiler_guard;
use crate::bindings::_helpers::compile_attachment::sql_expansion_context::present;
use crate::bindings::_helpers::compiled_views::relation_keys::model_key_row;
use crate::bindings::_helpers::compiled_views::relation_keys::{
    RelationKeyRow, RelationKeys, one_row_seed_names, relation_keys, source_key_input,
};
use crate::bindings::_helpers::native_threads::interruptible_join::{Joined, join_interruptibly};
use crate::bindings::_helpers::records::fields::field as record_field;
use crate::bindings::_helpers::render::authored_sql::SqlExpansionInputs;
use crate::bindings::_helpers::rules::lint_expansion::{
    LintExpansionSources, lint_expansion_inputs,
};
use crate::bindings::_helpers::rules::lint_inputs::model_lint_expansion;
use crate::bindings::_helpers::rules::lint_pipeline::{
    CompiledSource, LintFile, LintOutcome, LintRequest, LintSettings, lint, lint_sources,
};
use crate::bindings::_helpers::rules::rule_cache::stored_entries;
use crate::bindings::_helpers::rules::rules_config::rules_config_of;

/// The lint thresholds used when the project leaves them unset.
const DEFAULT_MAX_LITERAL_LENGTH: i64 = 100;
const DEFAULT_MAX_RANKING_ORDER_BY: i64 = 5;
const MAX_LITERAL_LENGTH: &str = "max_literal_length";
const MAX_RANKING_ORDER_BY: &str = "max_ranking_order_by";
const LINT_THREAD: &str = "sqlbuild-lint";
const SQL_RULES_CACHE_FILE: &str = "sql.json";

/// The SQL rule result cache file of `project_dir`.
pub(crate) fn sql_rule_cache_path(project_dir: &Path) -> PathBuf {
    rules_bulk_cache_path(project_dir, SQL_RULES_CACHE_FILE)
}

/// The native SQL lint settings that SQL rules run with.
pub(crate) fn sql_lint_settings(
    dialect: &str,
    codes: &[String],
    threshold: impl Fn(&str) -> PyResult<Option<i64>>,
    relation_keys: RelationKeys,
) -> PyResult<LintSettings> {
    let defaults: LintSettings = LintSettings::default();
    Ok(LintSettings {
        header_rules_enabled: false,
        sql: LintSqlOptions {
            dialect: dialect.to_owned(),
            enabled_rules: Some(codes.to_vec()),
            max_literal_length: threshold(MAX_LITERAL_LENGTH)?
                .unwrap_or(DEFAULT_MAX_LITERAL_LENGTH),
            max_ranking_order_by: threshold(MAX_RANKING_ORDER_BY)?
                .unwrap_or(DEFAULT_MAX_RANKING_ORDER_BY),
            ..defaults.sql
        },
        relation_keys,
        ..defaults
    })
}

/// What early SQL lint runs with: the project, its selected SQL rule codes and lint settings.
struct EarlyLintPlan {
    project_dir: PathBuf,
    codes: Vec<String>,
    settings: LintSettings,
}

/// The plan early SQL lint runs with, when the project selects SQL rules without a warm cache.
fn early_lint_plan(
    inputs: &Bound<'_, PyAny>,
    dialect: &str,
    no_cache: bool,
) -> PyResult<Option<EarlyLintPlan>> {
    let py: Python<'_> = inputs.py();
    let project_dir: Bound<'_, PyAny> =
        record_field(&record_field(inputs, "discovered_inputs")?, "project_dir")?;
    if project_dir.is_none() {
        return Ok(None);
    }
    let project_dir: PathBuf = project_dir.extract()?;
    for diagnostic in record_field(inputs, "diagnostics")?.try_iter()? {
        if record_field(&diagnostic?, "is_error")?.is_truthy()? {
            return Ok(None);
        }
    }
    let Ok(mut config) = rules_config_of(&project_dir)? else {
        return Ok(None);
    };
    config.cache.enabled &= !no_cache;
    if config.cache.enabled && !stored_entries(&sql_rule_cache_path(&project_dir))?.is_empty() {
        return Ok(None);
    }
    let codes: Vec<String> =
        compiler_guard(|| Ok(selected_sql_codes(config.select.as_deref(), &config.ignore)))?;
    if codes.is_empty() {
        return Ok(None);
    }
    let mut rows: Vec<RelationKeyRow> = Vec::new();
    for model in record_field(inputs, "model_inputs")?.try_iter()? {
        rows.push(model_key_row(&model?)?);
    }
    let seed_inputs: Vec<Bound<'_, PyAny>> = record_field(inputs, "seed_inputs")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    let one_row_seeds: Vec<String> = one_row_seed_names(&seed_inputs)?;
    let mut seeds: Vec<Bound<'_, PyAny>> = Vec::new();
    for seed in &seed_inputs {
        seeds.push(record_field(seed, "schema_entry")?);
    }
    let mut sources: Vec<Bound<'_, PyAny>> = Vec::new();
    for source in record_field(inputs, "source_inputs")?.try_iter()? {
        sources.push(source_key_input(&source?)?);
    }
    let keys: RelationKeys = relation_keys(
        py,
        rows,
        (
            PyTuple::new(py, seeds)?.as_any(),
            PyTuple::new(py, sources)?.as_any(),
        ),
        &one_row_seeds,
    )?;
    let settings: LintSettings = sql_lint_settings(
        dialect,
        &codes,
        |key| Ok(config.thresholds.get(key).map(|value| i64::from(*value))),
        keys,
    )?;
    Ok(Some(EarlyLintPlan {
        project_dir,
        codes,
        settings,
    }))
}

/// Early SQL lint's outcome and the expansion inputs it linted with.
pub(crate) struct EarlyLintResult {
    pub(crate) outcome: LintOutcome,
    pub(crate) context: SqlExpansionInputs,
}

/// One early SQL lint running on a native thread, for one project and SQL rule codes.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct PreparedSqlLint {
    codes: Vec<String>,
    project_dir: PathBuf,
    thread: Mutex<Option<JoinHandle<PyResult<EarlyLintResult>>>>,
    result: OnceLock<Result<EarlyLintResult, PyErr>>,
    stop: Arc<AtomicBool>,
}

impl PreparedSqlLint {
    /// Whether this lint ran for `project_dir` with exactly `codes`.
    pub(crate) fn matches(&self, project_dir: &Path, codes: &[String]) -> bool {
        self.project_dir == project_dir && same_codes(&self.codes, codes)
    }

    /// Whether this lint ran with exactly `codes`.
    pub(crate) fn has_codes(&self, codes: &[String]) -> bool {
        same_codes(&self.codes, codes)
    }

    /// Whether the lint's result is already known.
    fn finished(&self) -> bool {
        self.result.get().is_some()
    }

    /// The lint's result, waiting without the GIL until it finishes or a signal handler raises.
    pub(crate) fn result(&self, py: Python<'_>) -> &Result<EarlyLintResult, PyErr> {
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

fn same_codes(left: &[String], right: &[String]) -> bool {
    left.iter().collect::<BTreeSet<_>>() == right.iter().collect::<BTreeSet<_>>()
}

/// Start early SQL lint on a native thread when the project selects SQL rules without a cache.
fn start_early_lint(
    (inputs, dialect): (&Bound<'_, PyAny>, &str),
    reuse: ExpansionReuse,
    no_cache: bool,
    stop: &Arc<AtomicBool>,
) -> PyResult<Option<PreparedSqlLint>> {
    let Some(plan) = early_lint_plan(inputs, dialect, no_cache)? else {
        return Ok(None);
    };
    let mut compiled: HashMap<PathBuf, CompiledSource> = HashMap::new();
    for model in record_field(inputs, "model_inputs")?.try_iter()? {
        if let Some((path, expansion)) = model_lint_expansion(&model?)? {
            compiled.insert(path, expansion);
        }
    }
    let work: EarlyLintWork = EarlyLintWork {
        project_dir: plan.project_dir.clone(),
        settings: plan.settings,
        reuse,
        compiled,
        stop: Arc::clone(stop),
    };
    let thread: JoinHandle<PyResult<EarlyLintResult>> = std::thread::Builder::new()
        .name(LINT_THREAD.to_owned())
        .spawn(move || Python::attach(|py| early_lint(py, &work)))
        .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
    Ok(Some(PreparedSqlLint {
        codes: plan.codes,
        project_dir: plan.project_dir,
        thread: Mutex::new(Some(thread)),
        result: OnceLock::new(),
        stop: Arc::clone(stop),
    }))
}

/// What one early SQL lint thread lints: the project, its settings and the compile's expansions.
struct EarlyLintWork {
    project_dir: PathBuf,
    settings: LintSettings,
    reuse: ExpansionReuse,
    compiled: HashMap<PathBuf, CompiledSource>,
    stop: Arc<AtomicBool>,
}

/// Expand and lint the project's SQL early, stopping between files once `stop` is set.
fn early_lint(py: Python<'_>, work: &EarlyLintWork) -> PyResult<EarlyLintResult> {
    let project_dir: &Path = &work.project_dir;
    let context: SqlExpansionInputs = lint_expansion_inputs(LintExpansionSources {
        project_dir: project_dir.into_pyobject(py)?.into_any(),
        value_renderer: Some(work.reuse.value_renderer(py)),
        compile: Some(&work.reuse),
        discovered_inputs: Some(work.reuse.discovered_inputs(py)?),
        declaration_scope: None,
        static_declaration_scope: present(work.reuse.declaration_scope(py)?),
    })?;
    let files: Vec<LintFile> = lint_sources(project_dir, None, None)?;
    let outcome: LintOutcome = lint(
        py,
        &LintRequest {
            project_dir,
            settings: &work.settings,
            dynamic_output_paths: &HashSet::new(),
            compiled_expansions: &work.compiled,
            context: Some(&context),
            skip_unexpandable: false,
            stop: Some(&work.stop),
        },
        files,
    )?;
    Ok(EarlyLintResult { outcome, context })
}

/// The compiler's inputs and adapter, which lint expansion reuses for the same discovery.
pub(crate) struct ExpansionReuse {
    inputs: Py<PyAny>,
    value_renderer: Py<PyAny>,
}

impl ExpansionReuse {
    fn clone_ref(&self, py: Python<'_>) -> Self {
        Self {
            inputs: self.inputs.clone_ref(py),
            value_renderer: self.value_renderer.clone_ref(py),
        }
    }

    /// The discovery the compile ran on.
    pub(crate) fn discovered_inputs<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        record_field(self.inputs.bind(py), "discovered_inputs")
    }

    /// The compile's declaration scope.
    pub(crate) fn declaration_scope<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        record_field(self.inputs.bind(py), "declaration_scope")
    }

    /// The compile's macro context, carrying its target and vars.
    pub(crate) fn macro_context<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        record_field(self.inputs.bind(py), "macro_context")
    }

    /// The adapter that renders declared values.
    pub(crate) fn value_renderer<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        self.value_renderer.bind(py).clone()
    }
}

/// Own speculative SQL checks for one compilation invocation.
#[pyclass(module = "sqlbuild._native")]
pub(crate) struct EarlySqlLint {
    #[pyo3(get)]
    pub(crate) enabled: bool,
    /// Whether `--no-cache` turned the Rules cache off, so a warm SQL rule cache is ignored.
    no_cache: bool,
    preparation: Option<Py<PreparedSqlLint>>,
    expansion_reuse: Option<ExpansionReuse>,
    /// The dialect and adapter the `on_inputs_ready` call passes to `start`.
    ready_arguments: Option<(String, Py<PyAny>)>,
    stop_requested: Arc<AtomicBool>,
}

impl EarlySqlLint {
    /// A manager that starts speculative checks only when `enabled`.
    pub(crate) fn new(enabled: bool, no_cache: bool) -> Self {
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
    pub(crate) fn preparation(&self) -> Option<&Py<PreparedSqlLint>> {
        self.preparation.as_ref()
    }

    /// The compile's discovery and declaration scope, if it had one.
    pub(crate) fn expansion_reuse(&self) -> Option<&ExpansionReuse> {
        self.expansion_reuse.as_ref()
    }

    /// This manager as the `on_inputs_ready` hook, which calls `start` with `dialect` and `value_renderer`.
    pub(crate) fn inputs_ready_hook<'py>(
        slf: &Bound<'py, Self>,
        dialect: &str,
        value_renderer: &Bound<'py, PyAny>,
    ) -> Bound<'py, PyAny> {
        slf.borrow_mut().ready_arguments =
            Some((dialect.to_owned(), value_renderer.clone().unbind()));
        slf.clone().into_any()
    }

    /// Abandon speculative work, return the milliseconds spent joining it, and raise Ctrl+C caught while waiting.
    pub(crate) fn stop(&self, py: Python<'_>) -> PyResult<u64> {
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
    fn py_new(enabled: bool, no_cache: bool) -> Self {
        Self::new(enabled, no_cache)
    }

    fn __enter__(slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        slf
    }

    #[pyo3(signature = (*_exception))]
    fn __exit__(&self, py: Python<'_>, _exception: &Bound<'_, PyTuple>) {
        let _ = self.stop(py);
    }

    /// Keep the compile's expansion inputs, and start early SQL lint when enabled and SQL rules are selected.
    #[pyo3(signature = (*, inputs, dialect, value_renderer))]
    fn start(
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
                    .map(|preparation| Py::new(py, preparation))
                    .transpose()?;
        }
        slf.expansion_reuse = (!reuse.declaration_scope(py)?.is_none()).then_some(reuse);
        Ok(())
    }

    /// Start with the arguments `inputs_ready_hook` stored, as the compile's `on_inputs_ready` hook.
    #[pyo3(signature = (*, inputs))]
    fn __call__(slf: PyRefMut<'_, Self>, inputs: &Bound<'_, PyAny>) -> PyResult<()> {
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
    fn py_stop(&self, py: Python<'_>) -> PyResult<u64> {
        self.stop(py)
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<EarlySqlLint>()?;
    module.add_class::<PreparedSqlLint>()?;
    Ok(())
}
