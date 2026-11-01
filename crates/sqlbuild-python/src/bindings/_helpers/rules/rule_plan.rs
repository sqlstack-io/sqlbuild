//! Plan one rules run: the catalogue, selected rules, project slice and settings it evaluates.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyFrozenSet, PySet, PySetMethods, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_rules::project_tree::main::pure_path_posix::pure_path_posix;
use std::path::Path;

use crate::bindings::_helpers::boundary::keyword_request::Request;
use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::compiled_views::records::project_with;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::{field, replaced};
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::rules::catalogue::{
    references_custom_rules, resolved_ignore_selectors, rule_catalogue, select_rules,
    verify_custom_rules,
};
use crate::bindings::_helpers::rules::constants::{SQL_RULE_PREFIX, TYPE_PROOF_RULE_CODES};
use crate::bindings::_helpers::rules::fact_replay::{project_attribute, project_models};
use crate::bindings::_helpers::rules::rule_run::freed_memory_released;

const MODEL_RESOURCE_TYPE: &str = "model";

/// Build the catalogue and select the rules, project slice and settings one run evaluates.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn plan_rule_run<'py>(
    options: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let options = Request::new("plan_rule_run", options)?;
    let py: Python<'py> = options.py();
    let graph: Bound<'py, PyAny> = options.get("graph")?;
    let project: Bound<'py, PyAny> = field(&graph, "project")?;
    freed_memory_released(py)?;
    let selected_keys: Bound<'py, PyAny> = options.get("selected_keys")?;
    rule_run_plan(
        (
            &project,
            &options.get("config")?,
            &options.get("project_dir")?,
        ),
        Some((&graph, &selected_keys)),
        options.get("no_sql_analysis")?.is_truthy()?,
    )
}

/// `plan_rule_run`'s plan of `project`, selected by `(graph, selected_keys)` when given.
pub(crate) fn rule_run_plan<'py>(
    (project, config, project_dir): (&Bound<'py, PyAny>, &Bound<'py, PyAny>, &Bound<'py, PyAny>),
    selection: Option<(&Bound<'py, PyAny>, &Bound<'py, PyAny>)>,
    no_sql_analysis: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    let effective_config: Bound<'py, PyAny> = resolved_ignore_selectors(config, project)?;
    let project_dir: Bound<'py, PyAny> = project_dir.call_method0("resolve")?;
    let include_custom: bool = references_custom_rules(&effective_config)?;
    let catalogue: Vec<Bound<'py, PyAny>> =
        rule_catalogue(&effective_config, &project_dir, include_custom)?;
    let catalogue_tuple: Bound<'py, PyTuple> = PyTuple::new(py, &catalogue)?;
    let custom_payloads: Bound<'py, PyAny> = call_with(
        &host::entry(py, UserCodeHost::CustomRulePayloads)?,
        &[
            ("catalogue", catalogue_tuple.clone().into_any()),
            ("project_dir", project_dir.clone()),
        ],
    )?;
    let mut selected: Vec<Bound<'py, PyAny>> = select_rules(
        &catalogue,
        &effective_config,
        &project_dir,
        Some(custom_payloads.clone()),
    )?;
    let mut skipped_type_proof_rules: Vec<Bound<'py, PyAny>> = Vec::new();
    if no_sql_analysis
        || !field(&project_attribute(project, "settings")?, "sql_analysis")?.is_truthy()?
    {
        let mut kept: Vec<Bound<'py, PyAny>> = Vec::with_capacity(selected.len());
        for rule in selected {
            let code: Bound<'py, PyAny> = field(&rule, "code")?;
            if TYPE_PROOF_RULE_CODES.contains(&code.extract::<String>()?.as_str()) {
                skipped_type_proof_rules.push(code);
            } else {
                kept.push(rule);
            }
        }
        selected = kept;
    }
    if no_sql_analysis {
        let mut kept: Vec<Bound<'py, PyAny>> = Vec::with_capacity(selected.len());
        for rule in selected {
            if !field(&rule, "code")?
                .extract::<String>()?
                .starts_with(SQL_RULE_PREFIX)
            {
                kept.push(rule);
            }
        }
        selected = kept;
    }
    let mut custom_rules: Vec<Bound<'py, PyAny>> = Vec::new();
    for rule in &selected {
        if field(rule, "custom")?.is_truthy()? {
            custom_rules.push(rule.clone());
        }
    }
    if !custom_rules.is_empty() {
        verify_custom_rules(&custom_rules, &project_dir)?;
    }
    let model_paths: Option<Bound<'py, PyFrozenSet>> = match selection {
        Some((graph, selected_keys)) => selected_model_paths(graph, selected_keys)?,
        None => None,
    };
    let selected_project: Bound<'py, PyAny> = match &model_paths {
        None => project.clone(),
        Some(paths) => selected_models_project(project, paths)?,
    };
    let mut codes: Vec<Bound<'py, PyAny>> = Vec::with_capacity(selected.len());
    for rule in &selected {
        codes.push(field(rule, "code")?);
    }
    let selection_config: Bound<'py, PyAny> =
        selection_rules_config(&effective_config, model_paths.as_ref(), &selected_project)?;
    let selected_config: Bound<'py, PyAny> = replaced(
        &selection_config,
        &[
            ("select", PyTuple::new(py, codes)?.into_any()),
            ("ignore", PyTuple::empty(py).into_any()),
        ],
    )?;
    new_record_with(
        py,
        RecordId::RuleRunPlan,
        &[
            ("effective_config", effective_config),
            ("project_dir", project_dir),
            ("catalogue", catalogue_tuple.into_any()),
            ("custom_payloads", custom_payloads),
            ("selected", PyTuple::new(py, selected)?.into_any()),
            (
                "skipped_type_proof_rules",
                PyTuple::new(py, skipped_type_proof_rules)?.into_any(),
            ),
            (
                "model_paths",
                model_paths.map_or_else(|| py.None().into_bound(py), |paths| paths.into_any()),
            ),
            ("project", selected_project),
            ("config", selected_config),
        ],
    )
}

/// The selected non-SQL-lint codes a model selection evaluates over the whole project.
pub(crate) fn whole_project_codes(plan: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let mut codes: Vec<String> = Vec::new();
    if field(plan, "model_paths")?.is_none() {
        return Ok(codes);
    }
    for rule in field(plan, "selected")?.try_iter()? {
        let code: String = field(&rule?, "code")?.extract()?;
        if !code.starts_with(SQL_RULE_PREFIX) {
            codes.push(code);
        }
    }
    Ok(codes)
}

/// The relative paths of the selected models, or `None` when the run evaluates every model.
fn selected_model_paths<'py>(
    graph: &Bound<'py, PyAny>,
    selected_keys: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyFrozenSet>>> {
    let py: Python<'py> = graph.py();
    if selected_keys.is_none() {
        return Ok(None);
    }
    let names: Bound<'py, PySet> = PySet::empty(py)?;
    for key in selected_keys.try_iter()? {
        let key: Bound<'py, PyAny> = key?;
        if field(&key, "resource_type")?.eq(MODEL_RESOURCE_TYPE)? {
            names.add(field(&key, "name")?)?;
        }
    }
    let paths: Bound<'py, PySet> = PySet::empty(py)?;
    for model in project_models(&field(graph, "project")?)? {
        if names.contains(&model.name)? {
            paths.add(model.posix)?;
        }
    }
    Ok(Some(PyFrozenSet::new(py, paths.iter())?))
}

/// `project` keeping only the models whose relative path is in `model_paths`.
fn selected_models_project<'py>(
    project: &Bound<'py, PyAny>,
    model_paths: &Bound<'py, PyFrozenSet>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    let mut models: Vec<Bound<'py, PyAny>> = Vec::new();
    for entry in project_models(project)? {
        if model_paths.contains(&entry.posix)? {
            models.push(entry.model);
        }
    }
    project_with(
        project,
        Some(keywords(
            py,
            &[("models", PyTuple::new(py, models)?.into_any())],
        )?),
    )
}

/// `config` keeping only the exceptions that apply to the selected models.
fn selection_rules_config<'py>(
    config: &Bound<'py, PyAny>,
    model_paths: Option<&Bound<'py, PyFrozenSet>>,
    project: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = config.py();
    let Some(model_paths) = model_paths else {
        return Ok(config.clone());
    };
    let model_names: Bound<'py, PySet> = PySet::empty(py)?;
    for model in project_models(project)? {
        model_names.add(model.name)?;
    }
    let mut rule_exceptions: Vec<Bound<'py, PyAny>> = Vec::new();
    for entry in field(config, "rule_exceptions")?.try_iter()? {
        let entry: Bound<'py, PyAny> = entry?;
        let path: String = field(&entry, "path")?.extract()?;
        let path: String = String::from_utf8_lossy(&pure_path_posix(Path::new(&path))).into_owned();
        if model_paths.contains(path)? {
            rule_exceptions.push(entry);
        }
    }
    let mut graph_edge_exceptions: Vec<Bound<'py, PyAny>> = Vec::new();
    for entry in field(config, "graph_edge_exceptions")?.try_iter()? {
        let entry: Bound<'py, PyAny> = entry?;
        if model_names.contains(field(&entry, "consumer")?)? {
            graph_edge_exceptions.push(entry);
        }
    }
    replaced(
        config,
        &[
            (
                "rule_exceptions",
                PyTuple::new(py, rule_exceptions)?.into_any(),
            ),
            (
                "graph_edge_exceptions",
                PyTuple::new(py, graph_edge_exceptions)?.into_any(),
            ),
        ],
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(plan_rule_run, module)?)?;
    Ok(())
}
