//! The static command context of audit, check, seed and test commands: scope and relations.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyFrozenSet};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_python_base::native_host::models::Dialect;

use sqlbuild_python_compile::compile_driver::main::stages::build_deferred_locations::build_deferred_locations;
use sqlbuild_python_compile::compile_driver::main::stages::resolve_deferred_target_config::resolve_deferred_target_config;
use sqlbuild_python_compile::compile_driver::main::setup::compile_project_phase::compile_project_phase;
use sqlbuild_python_compile::compile_driver::main::setup::build_project_graph_from_compiled_project::build_project_graph_from_compiled_project;
use sqlbuild_python_compile::compile_driver::main::stages::unit_test_selectors_applied::unit_test_selectors_applied;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_planner::planner_entries::main::plans::resolve_static_command_scope::resolve_static_command_scope;
use sqlbuild_python_planner::planner_entries::main::plans::resolve_static_relation_context::resolve_static_relation_context;
use sqlbuild_python_project::graph::main::python_nodes::run_selection_from_inputs::run_selection_from_inputs;

/// Locations of the deferred target's relations, when the options defer to one.
fn deferred_locations<'py>(
    (discovered, adapter): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    options: &Bound<'py, PyAny>,
    project: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    let defer_to: Bound<'py, PyAny> = native_field(options, "defer_to")?;
    if defer_to.is_none() {
        return Ok(py.None().into_bound(py));
    }
    let deferred_target: Bound<'py, PyAny> = resolve_deferred_target_config(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered.clone()),
                ("defer_to", defer_to),
                (
                    "current_target_name",
                    native_field(project, "effective_target_name")?,
                ),
            ],
        )?),
    )?;
    Ok(build_deferred_locations(
        py,
        Some(keywords(
            py,
            &[
                ("project", project.clone()),
                ("deferred_target_config", deferred_target),
                ("effective_vars", native_field(project, "effective_vars")?),
                ("default_schema", adapter.call_method0("default_schema")?),
                (
                    "default_database",
                    adapter.call_method0("default_database")?,
                ),
                (
                    "render_qualified_name",
                    native_field(adapter, "render_qualified_name")?,
                ),
            ],
        )?),
    )?
    .into_any())
}

/// Run canonical compile, selector, and relation phases without build planning.
#[pyfunction]
#[pyo3(signature = (**request))]
pub(crate) fn compile_static_command_context<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let request: Request<'_, 'py> = Request::new("compile_static_command_context", request)?;
    let py: Python<'py> = request.py();
    let none = || py.None().into_bound(py);
    let discovered: Bound<'py, PyAny> = request.get("discovered_inputs")?;
    let adapter: Bound<'py, PyAny> = request.get("adapter")?;
    let options: Bound<'py, PyAny> =
        unit_test_selectors_applied(&request.get("options")?, &discovered)?;
    let mut selected_keys: Bound<'py, PyAny> =
        request.optional("selected_keys")?.unwrap_or_else(none);
    if selected_keys.is_none()
        && !native_field(&options, "sql_test_selection")?
            .getattr("models_selected")?
            .is_truthy()?
    {
        selected_keys = PyFrozenSet::empty(py)?.into_any();
    }
    let compiled: Bound<'py, PyAny> = compile_project_phase(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered.clone()),
                ("adapter", adapter.clone()),
                ("options", options.clone()),
                (
                    "on_progress",
                    request.optional("on_progress")?.unwrap_or_else(none),
                ),
            ],
        )?),
    )?;
    let project: Bound<'py, PyAny> = native_field(&compiled, "project")?;
    let selective: bool = native_field(&options, "select")?.is_truthy()?
        || native_field(&options, "exclude")?.is_truthy()?;
    if selected_keys.is_none()
        && selective
        && native_field(&options, "resolve_python_run_selectors")?.is_truthy()?
    {
        let graph: Bound<'py, PyAny> = build_project_graph_from_compiled_project(
            py,
            Some(keywords(py, &[("project", project.clone())])?),
        )?;
        let selection: Bound<'py, PyAny> = run_selection_from_inputs(
            (
                native_field(&options, "select")?.extract()?,
                native_field(&options, "exclude")?.extract()?,
            ),
            &graph,
            &discovered,
        )?;
        selected_keys = native_field(&selection, "sql_keys")?;
    }
    let deferred: Bound<'py, PyAny> =
        deferred_locations((&discovered, &adapter), &options, &project)?;
    let scope: Bound<'py, PyAny> = resolve_static_command_scope(
        &project,
        &new_record_with(
            py,
            RecordId::PlannerSelection,
            &[
                ("select", native_field(&options, "select")?),
                ("exclude", native_field(&options, "exclude")?),
                ("selected_keys", selected_keys),
                (
                    "sql_test_selection",
                    native_field(&options, "sql_test_selection")?,
                ),
            ],
        )?,
        native_field(&options, "auto_load_sources")?.extract()?,
    )?;
    let relations: Bound<'py, PyAny> = resolve_static_relation_context(
        py,
        Some(keywords(
            py,
            &[
                ("project", project.clone()),
                ("dialect", Dialect::of(&adapter)?.into_any()),
                ("scope", scope.clone()),
                (
                    "deferral",
                    new_record_with(
                        py,
                        RecordId::DeferralInputs,
                        &[
                            ("deferred_locations", deferred),
                            (
                                "defer_sources_to",
                                native_field(&options, "defer_sources_to")?,
                            ),
                            (
                                "source_deferral_enabled",
                                native_field(&options, "source_deferral_enabled")?,
                            ),
                        ],
                    )?,
                ),
                (
                    "project_config",
                    native_field(&discovered, "project_config")?,
                ),
                ("local_config", native_field(&discovered, "local_config")?),
                (
                    "relation_keys",
                    request.optional("relation_keys")?.unwrap_or_else(none),
                ),
            ],
        )?),
    )?;
    new_record_with(
        py,
        RecordId::StaticCommandContext,
        &[
            ("project", project),
            ("scope", scope),
            ("relations", relations),
            (
                "connection_config",
                native_field(&compiled, "connection_config")?,
            ),
            (
                "compile_seconds",
                native_field(&compiled, "compile_seconds")?,
            ),
        ],
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(compile_static_command_context, module)?)?;
    Ok(())
}
