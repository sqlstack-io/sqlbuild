//! Speculative SQL checks started once compile inputs are ready, and the lint settings they use.

use crate::rules::main::configuration::rules_config_of::rules_config_of;
use crate::rules::models::EarlyLintResult;
use crate::rules::models::ExpansionReuse;
use crate::rules::models::PreparedSqlLint;
use std::path::PathBuf;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, IntoPyObject, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_rules::lint_text::models::LintSqlOptions;
use sqlbuild_rules::rule_cache::main::rules_bulk_cache_path::rules_bulk_cache_path;
use sqlbuild_rules::rules::main::selected_sql_codes::selected_sql_codes;

use crate::compile_attachment::main::present::present;
use crate::rules::_helpers::lint::lint_expansion::LintExpansionSources;
use crate::rules::_helpers::lint::lint_expansion::lint_expansion_inputs;
use crate::rules::_helpers::lint::lint_inputs::model_lint_expansion;
use crate::rules::_helpers::lint::lint_pipeline::CompiledSource;
use crate::rules::_helpers::lint::lint_pipeline::LintRequest;
use crate::rules::_helpers::lint::lint_pipeline::LintSettings;
use crate::rules::_helpers::lint::lint_pipeline::lint;
use crate::rules::_helpers::lint::lint_pipeline::lint_sources;
use crate::rules::_helpers::rule::rule_cache::stored_entries;
use crate::rules::models::LintFile;
use crate::rules::models::LintOutcome;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::records::main::fields::field::field as record_field;
use sqlbuild_python_front::render::models::SqlExpansionInputs;
use sqlbuild_python_project::compiled_views::main::compiled_models::model_key_row::model_key_row;
use sqlbuild_python_project::compiled_views::main::compiled_models::one_row_seed_names::one_row_seed_names;
use sqlbuild_python_project::compiled_views::main::compiled_models::relation_keys::relation_keys;
use sqlbuild_python_project::compiled_views::main::compiled_models::source_key_input::source_key_input;
use sqlbuild_python_project::compiled_views::models::RelationKeyRow;
use sqlbuild_python_project::compiled_views::types::RelationKeys;

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

pub(crate) fn same_codes(left: &[String], right: &[String]) -> bool {
    left.iter().collect::<BTreeSet<_>>() == right.iter().collect::<BTreeSet<_>>()
}

/// Start early SQL lint on a native thread when the project selects SQL rules without a cache.
pub(crate) fn start_early_lint(
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
        vars: work.reuse.expansion_vars(py)?,
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
