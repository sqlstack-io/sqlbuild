//! Resolve `sqb format` selectors to the adapter, discovered inputs and selected files.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyFrozenSet, PyString, PyStringMethods, PyTuple};
use sqlbuild_core::format_command::errors::FormatSelectorError;
use sqlbuild_core::format_command::main::exact_selector_names::exact_selector_names;
use sqlbuild_core::format_command::main::lint_sql_paths::lint_sql_paths;
use sqlbuild_core::format_command::main::partition_lint_selectors::partition_lint_selectors;
use sqlbuild_core::format_command::main::resolve_lint_paths::resolve_lint_paths;
use sqlbuild_rules::lint_text::main::scan_fixture_null_candidates::scan_fixture_null_candidates;

use crate::bindings::_helpers::compile_driver::discovery::discover_project_inputs;
use crate::bindings::_helpers::compile_driver::project::build_project_graph;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_effective_adapter_name;
use crate::bindings::_helpers::discovery::declaration_records::discover_model_schema_files;
use crate::bindings::_helpers::discovery::model_records::discover_model_files;
use crate::bindings::_helpers::discovery::project_config::load_local_config_file;
use crate::bindings::_helpers::discovery::project_config::load_project_config_file;
use crate::bindings::_helpers::discovery::snapshot::directory_snapshot_scope;
use crate::bindings::_helpers::discovery::sql_test_records::discover_test_files;
use crate::bindings::_helpers::discovery::yaml_records::{
    discover_schema_files, discover_source_files,
};
use crate::bindings::_helpers::planner_selection::selectors::select_project_graph;
use crate::bindings::_helpers::planner_selection::unit_tests::reject_unit_test_selectors;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::command_runtime::main::resolve_cli_adapter::resolve_cli_adapter;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

const SELECTION_CODE: &str = "S007";

/// The adapter, the selected files when a selector scopes the run, and the discovered inputs.
pub(crate) struct LintInputs<'py> {
    pub(crate) adapter: Bound<'py, PyAny>,
    pub(crate) selected: Option<BTreeSet<PathBuf>>,
    pub(crate) discovered: Bound<'py, PyAny>,
}

/// The project, its selectors and the inputs discovered for it.
struct Scope<'a, 'py> {
    directory: PathBuf,
    discovered: &'a Bound<'py, PyAny>,
    adapter: &'a Bound<'py, PyAny>,
}

fn planner_error(py: Python<'_>, message: &str, code: &str) -> PyErr {
    let raised = (|| -> PyResult<Bound<'_, PyAny>> {
        let keywords: Bound<'_, PyDict> = PyDict::new(py);
        keywords.set_item("code", code)?;
        error_class(py, ErrorId::PlannerInput)?.call((message,), Some(&keywords))
    })();
    match raised {
        Ok(instance) => PyErr::from_value(instance),
        Err(failure) => failure,
    }
}

fn selector_error(py: Python<'_>, error: &FormatSelectorError) -> PyErr {
    planner_error(py, &error.message, error.code)
}

fn resolved(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn validate(py: Python<'_>, paths: &BTreeSet<PathBuf>) -> PyResult<()> {
    if paths.is_empty() {
        return Err(planner_error(
            py,
            "lint selection matched no SQL files",
            SELECTION_CODE,
        ));
    }
    Ok(())
}

fn discovered_adapter<'py>(
    project_dir: &Bound<'py, PyAny>,
    discovered: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let adapter_name: Bound<'py, PyAny> = resolve_effective_adapter_name(
        &native_field(discovered, "project_config")?,
        &native_field(discovered, "local_config")?,
    )?;
    resolve_cli_adapter(&adapter_name, project_dir)
}

fn selectors<'py>(py: Python<'py>, values: &[String]) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyTuple::new(py, values)?.into_any())
}

/// Model names and resolved files, by configured name or file stem.
fn model_names_and_paths(scope: &Scope<'_, '_>) -> PyResult<Vec<(String, PathBuf)>> {
    let mut pairs: Vec<(String, PathBuf)> = Vec::new();
    for model_file in native_field(scope.discovered, "model_files")?.try_iter()? {
        let model_file: Bound<'_, PyAny> = model_file?;
        let relative_path: Bound<'_, PyAny> = native_field(&model_file, "relative_path")?;
        let configured: Bound<'_, PyAny> =
            native_field(&model_file, "header_values")?.call_method1("get", ("name",))?;
        let name: String = match configured.downcast::<PyString>() {
            Ok(text) if !text.to_str()?.is_empty() => text.to_str()?.to_owned(),
            _ => native_field(&relative_path, "stem")?.extract()?,
        };
        let path: PathBuf = scope.directory.join(relative_path.extract::<PathBuf>()?);
        pairs.push((name, resolved(&path)));
    }
    Ok(pairs)
}

/// Resolve unexpanded model-name selectors without compiling the project graph.
fn exact_model_paths(
    py: Python<'_>,
    scope: &Scope<'_, '_>,
    (select, exclude): (&[String], &[String]),
) -> PyResult<Option<BTreeSet<PathBuf>>> {
    let (Some(selected), Some(excluded)) =
        (exact_selector_names(select), exact_selector_names(exclude))
    else {
        return Ok(None);
    };
    let pairs: Vec<(String, PathBuf)> = model_names_and_paths(scope)?;
    let available: BTreeSet<String> = pairs.iter().map(|(name, _)| name.clone()).collect();
    if let Some(unknown) = selected
        .union(&excluded)
        .find(|name| !available.contains(*name))
    {
        return Err(planner_error(
            py,
            &format!("unknown selector name '{unknown}'"),
            SELECTION_CODE,
        ));
    }
    let names: &BTreeSet<String> = if selected.is_empty() {
        &available
    } else {
        &selected
    };
    Ok(Some(
        pairs
            .into_iter()
            .filter(|(name, _)| names.contains(name) && !excluded.contains(name))
            .map(|(_, path)| path)
            .collect(),
    ))
}

/// Resolve model selectors to authored model files.
fn model_paths(
    py: Python<'_>,
    scope: &Scope<'_, '_>,
    (select, exclude): (&[String], &[String]),
) -> PyResult<BTreeSet<PathBuf>> {
    if let Some(paths) = exact_model_paths(py, scope, (select, exclude))? {
        return Ok(paths);
    }
    let graph: Bound<'_, PyAny> = build_project_graph(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", scope.discovered.clone()),
                ("adapter", scope.adapter.clone()),
            ],
        )?),
    )?;
    let keys: Bound<'_, PyAny> = select_project_graph(
        &graph.clone(),
        &selectors(py, select)?,
        &selectors(py, exclude)?,
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    let model_type: Bound<'_, PyAny> = enum_member(py, RecordId::CompiledResourceType, "MODEL")?;
    let mut names: BTreeSet<String> = BTreeSet::new();
    for key in keys.try_iter()? {
        let key: Bound<'_, PyAny> = key?;
        if native_field(&key, "resource_type")?.eq(&model_type)? {
            names.insert(native_field(&key, "name")?.extract()?);
        }
    }
    let mut paths: BTreeSet<PathBuf> = BTreeSet::new();
    for model in native_field(&graph, "project")?
        .getattr("models")?
        .try_iter()?
    {
        let model: Bound<'_, PyAny> = model?;
        if names.contains(&native_field(&model, "name")?.extract::<String>()?) {
            let relative: PathBuf = native_field(&model, "relative_path")?.extract()?;
            paths.insert(resolved(&scope.directory.join(relative)));
        }
    }
    Ok(paths)
}

/// Default or path-based format scopes, resolved before graph selector expansion.
fn combined_paths(
    py: Python<'_>,
    scope: &Scope<'_, '_>,
    (select, exclude): (&[String], &[String]),
) -> PyResult<Option<BTreeSet<PathBuf>>> {
    if select
        .iter()
        .chain(exclude)
        .any(|selector| selector.trim().is_empty())
    {
        return Ok(None);
    }
    let (selected_paths, selected_models) =
        partition_lint_selectors(select).map_err(|error| selector_error(py, &error))?;
    let (excluded_paths, excluded_models) =
        partition_lint_selectors(exclude).map_err(|error| selector_error(py, &error))?;
    if selected_paths.is_empty() && excluded_paths.is_empty() && !select.is_empty() {
        return Ok(None);
    }
    let directory: &Path = &scope.directory;
    let mut selected: BTreeSet<PathBuf> = if select.is_empty() {
        lint_sql_paths(directory, &[])
    } else if selected_paths.is_empty() {
        BTreeSet::new()
    } else {
        lint_sql_paths(directory, &selected_paths)
    };
    if !select.is_empty() && !selected_models.is_empty() {
        selected.extend(model_paths(py, scope, (&selected_models, &[]))?);
    }
    let mut excluded: BTreeSet<PathBuf> = if excluded_paths.is_empty() {
        BTreeSet::new()
    } else {
        lint_sql_paths(directory, &excluded_paths)
    };
    if !excluded_models.is_empty() {
        excluded.extend(model_paths(py, scope, (&excluded_models, &[]))?);
    }
    Ok(Some(selected.difference(&excluded).cloned().collect()))
}

/// The contract inputs selected fixtures need, without importing Python runtime resources.
fn selected_contract_inputs<'py>(
    project_dir: &Bound<'py, PyAny>,
    selected_test_paths: Bound<'py, PyAny>,
    referenced_model_names: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    let project_config: Bound<'py, PyAny> = load_project_config_file(py, project_dir)
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    let local_config: Bound<'py, PyAny> = load_local_config_file(py, project_dir)
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    let disabled: Bound<'py, PyAny> = PyBool::new(py, false).to_owned().into_any();
    let scope: Bound<'py, PyAny> = pyo3::IntoPyObjectExt::into_bound_py_any(
        directory_snapshot_scope(project_dir.clone()),
        py,
    )?;
    within(&scope, |_| {
        new_record_with(
            py,
            RecordId::DiscoveredProjectInputs,
            &[
                ("project_config", project_config),
                ("local_config", local_config),
                ("project_dir", project_dir.clone()),
                (
                    "model_files",
                    discover_model_files(
                        py,
                        Some(keywords(
                            py,
                            &[
                                ("project_dir", project_dir.clone()),
                                ("extract_implicit_alias_columns", disabled.clone()),
                                ("extract_output_column_locations", disabled.clone()),
                                ("selected_model_names", referenced_model_names),
                            ],
                        )?),
                    )
                    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?,
                ),
                (
                    "model_schema_files",
                    discover_model_schema_files(project_dir, None)?.into_any(),
                ),
                (
                    "schema_files",
                    discover_schema_files(project_dir)?.into_any(),
                ),
                (
                    "source_files",
                    discover_source_files(project_dir, None)?.into_any(),
                ),
                (
                    "test_files",
                    discover_test_files(
                        &project_dir.clone(),
                        Some(selected_test_paths).filter(|value| !value.is_none()),
                        None,
                    )
                    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?,
                ),
            ],
        )
    })
}

/// Inputs for a run scoped by path selectors, discovering only the contracts fixtures need.
fn path_scoped_inputs<'py>(
    project_dir: &Bound<'py, PyAny>,
    directory: &Path,
    paths: BTreeSet<PathBuf>,
) -> PyResult<LintInputs<'py>> {
    let py: Python<'py> = project_dir.py();
    validate(py, &paths)?;
    let candidates = scan_fixture_null_candidates(directory, Some(&paths)).map_err(value_error)?;
    let candidate_paths: Vec<PathBuf> = candidates.paths.into_iter().map(PathBuf::from).collect();
    let discovered: Bound<'py, PyAny> = selected_contract_inputs(
        project_dir,
        PyFrozenSet::new(py, candidate_paths)?.into_any(),
        PyFrozenSet::new(py, candidates.model_names)?.into_any(),
    )?;
    Ok(LintInputs {
        adapter: discovered_adapter(project_dir, &discovered)?,
        selected: Some(paths),
        discovered,
    })
}

/// Resolve the adapter and optional file scope through canonical selectors.
pub(crate) fn resolve_lint_inputs<'py>(
    project_dir: &Bound<'py, PyAny>,
    (select, exclude): (&[String], &[String]),
) -> PyResult<LintInputs<'py>> {
    let py: Python<'py> = project_dir.py();
    let directory: PathBuf = project_dir.extract()?;
    if let Some(paths) = resolve_lint_paths(&directory, (select, exclude)) {
        return path_scoped_inputs(project_dir, &directory, paths);
    }
    let disabled: Bound<'py, PyAny> = PyBool::new(py, false).to_owned().into_any();
    let discovered: Bound<'py, PyAny> = discover_project_inputs(
        py,
        Some(keywords(
            py,
            &[
                ("project_dir", project_dir.clone()),
                ("sql_analysis_enabled_override", disabled.clone()),
                ("extract_output_column_locations", disabled),
            ],
        )?),
    )?;
    reject_unit_test_selectors(
        &selectors(py, select)?,
        &selectors(py, exclude)?,
        &discovered.clone(),
    )?;
    let adapter: Bound<'py, PyAny> = discovered_adapter(project_dir, &discovered)?;
    if select.is_empty() && exclude.is_empty() {
        return Ok(LintInputs {
            adapter,
            selected: None,
            discovered,
        });
    }
    let scope: Scope<'_, 'py> = Scope {
        directory,
        discovered: &discovered,
        adapter: &adapter,
    };
    let paths: BTreeSet<PathBuf> = match combined_paths(py, &scope, (select, exclude))? {
        Some(paths) => paths,
        None => model_paths(py, &scope, (select, exclude))?,
    };
    validate(py, &paths)?;
    Ok(LintInputs {
        adapter,
        selected: Some(paths),
        discovered,
    })
}
