//! Lint authored SQL files: header rules, then expanded bodies through the native SQL engine.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{pyfunction, wrap_pyfunction};

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::compile_attachment::sql_expansion_context::ExpansionVars;
use crate::bindings::_helpers::compile_attachment::sql_expansion_context::present;
use crate::bindings::_helpers::render::authored_sql::SqlExpansionInputs;
use crate::bindings::_helpers::rules::lint_expansion::{
    LintExpansionSources, lint_expansion_inputs,
};
use crate::bindings::_helpers::rules::lint_pipeline::{
    CompiledExpansion, CompiledSource, LintFile, LintOutcome, LintRequest, LintSettings, lint,
    lint_sources,
};

/// The selected paths, or `None` for every file.
pub(crate) fn selected_paths(selected: &Bound<'_, PyAny>) -> PyResult<Option<BTreeSet<PathBuf>>> {
    if selected.is_none() {
        return Ok(None);
    }
    selected
        .try_iter()?
        .map(|path| path?.extract())
        .collect::<PyResult<_>>()
        .map(Some)
}

/// The files of a `{path: contents}` mapping, in its order.
pub(crate) fn given_files(files: &Bound<'_, PyAny>) -> PyResult<Option<Vec<LintFile>>> {
    if files.is_none() {
        return Ok(None);
    }
    files
        .downcast::<PyDict>()?
        .iter()
        .map(|(path, contents)| {
            Ok(LintFile {
                path: path.extract()?,
                contents: contents.extract()?,
            })
        })
        .collect::<PyResult<_>>()
        .map(Some)
}

/// The compile's expansions of a `{path: CompiledSqlExpansion}` mapping.
pub(crate) fn compiled_expansion_records(
    expansions: &Bound<'_, PyAny>,
) -> PyResult<HashMap<PathBuf, CompiledSource>> {
    let mut compiled: HashMap<PathBuf, CompiledSource> = HashMap::new();
    if expansions.is_none() {
        return Ok(compiled);
    }
    for (path, expansion) in expansions.downcast::<PyDict>()?.iter() {
        if !expansion.is_none() {
            compiled.insert(
                path.extract()?,
                CompiledSource::Owned(CompiledExpansion::from_record(&expansion)?),
            );
        }
    }
    Ok(compiled)
}

/// Collect supported SQL-analysis inputs of a project.
#[pyfunction]
#[pyo3(signature = (*, project_dir, selected_paths = None))]
fn collect_lint_project_files<'py>(
    py: Python<'py>,
    project_dir: PathBuf,
    selected_paths: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyDict>> {
    let selected: Option<BTreeSet<PathBuf>> = match selected_paths {
        Some(paths) => self::selected_paths(&paths)?,
        None => None,
    };
    let files: Bound<'py, PyDict> = PyDict::new(py);
    for file in lint_sources(&project_dir, selected.as_ref(), None)? {
        files.set_item(file.path, file.contents)?;
    }
    Ok(files)
}

/// Lint all DSL files without modifying anything; optionally skip unexpandable SQL.
#[pyfunction]
#[pyo3(signature = (**options))]
fn run_lint<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "run_lint",
        options,
        (
            &["project_dir", "config"],
            &[
                "value_renderer",
                "selected_paths",
                "discovered_inputs",
                "dynamic_output_paths",
                "compiled_expansions",
                "expansion_context",
                "source_files",
                "skip_unexpandable",
            ],
        ),
    )?;
    let project_dir: Bound<'py, PyAny> = arguments.value("project_dir")?;
    let settings: LintSettings = LintSettings::from_record(&arguments.value("config")?)?;
    let project_path: PathBuf = project_dir.extract()?;
    let files: Vec<LintFile> = lint_sources(
        &project_path,
        selected_paths(&arguments.value("selected_paths")?)?.as_ref(),
        given_files(&arguments.value("source_files")?)?,
    )?;
    let given: Bound<'py, PyAny> = arguments.value("expansion_context")?;
    let context: Option<SqlExpansionInputs> = if given.is_truthy()? {
        Some(SqlExpansionInputs::from_record(&given)?)
    } else if settings.native_enabled {
        let renderer: Bound<'py, PyAny> = arguments.value("value_renderer")?;
        Some(lint_expansion_inputs(LintExpansionSources {
            vars: ExpansionVars::ProjectDefault(None),
            project_dir: project_dir.clone(),
            value_renderer: renderer.is_truthy()?.then_some(renderer),
            discovered_inputs: present(arguments.value("discovered_inputs")?),
            declaration_scope: None,
            static_declaration_scope: None,
        })?)
    } else {
        None
    };
    let mut dynamic_output_paths: HashSet<PathBuf> = HashSet::new();
    let dynamic: Bound<'py, PyAny> = arguments.value("dynamic_output_paths")?;
    if !dynamic.is_none() {
        for path in dynamic.try_iter()? {
            dynamic_output_paths.insert(path?.extract()?);
        }
    }
    let outcome: LintOutcome = lint(
        py,
        &LintRequest {
            project_dir: &project_path,
            settings: &settings,
            dynamic_output_paths: &dynamic_output_paths,
            compiled_expansions: &compiled_expansion_records(
                &arguments.value("compiled_expansions")?,
            )?,
            context: context.as_ref(),
            skip_unexpandable: arguments.flag("skip_unexpandable")?,
            stop: None,
        },
        files,
    )?;
    outcome.record(py)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_lint, module)?)?;
    module.add_function(wrap_pyfunction!(collect_lint_project_files, module)?)?;
    Ok(())
}
