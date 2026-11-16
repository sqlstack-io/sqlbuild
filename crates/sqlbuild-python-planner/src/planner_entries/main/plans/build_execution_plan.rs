//! Build the execution plan for one selection against live warehouse state.

use crate::planner_entries::_helpers::plans::execution_plan::INSPECTION_CATALOG;
use crate::planner_entries::_helpers::plans::execution_plan::planned_output;
use crate::planner_entries::_helpers::plans::execution_plan::scopes_and_cursor_overrides;
use crate::planner_entries::_helpers::plans::test_planning::{PlanningRequest, start_planning};
use crate::planner_phases::main::inputs::class_field::class_field;
use crate::planner_phases::models::PlannerRuntime;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::build_command::main::utc_now::utc_now;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::planner_scope::models::PlannerScopeResolution;

/// Build the execution plan for one selection against live warehouse state.
#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub fn build_execution_plan<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "build_execution_plan",
        kwargs,
        (
            &[
                "project",
                "adapter",
                "connection",
                "selection",
                "overrides",
                "deferral",
                "policies",
            ],
            &["on_progress", "project_config", "local_config"],
        ),
    )?;
    let project = args.value("project")?;
    let adapter = args.value("adapter")?;
    let connection = args.value("connection")?;
    let runtime = Bound::new(
        py,
        PlannerRuntime {
            project: project.clone().unbind(),
            adapter: adapter.clone().unbind(),
            connection: connection.clone().unbind(),
            project_config: args.value("project_config")?.unbind(),
            local_config: args.value("local_config")?.unbind(),
            on_progress: args.value("on_progress")?.unbind(),
            invocation_time: utc_now(py)?.unbind(),
        },
    )?
    .into_any();
    let (scopes, overrides) = scopes_and_cursor_overrides(&args)?;
    let inspection = class_field(&scopes, |s: &PlannerScopeResolution| &s.inspection_scope)?;
    let test_planning = start_planning(
        py,
        PlanningRequest {
            project: project.clone().unbind(),
            adapter: adapter.clone().unbind(),
            selected_keys: native_field(&inspection, "selected_keys")?.unbind(),
            sql_test_selection: native_field(&inspection, "sql_test_selection")?.unbind(),
        },
        native_field(&args.value("policies")?, "plan_sql_tests")?.is_truthy()?,
    )?;
    let planned = (|| {
        let catalog = call_with(
            &host_entry(py, INSPECTION_CATALOG)?,
            &[("adapter", adapter), ("connection", connection)],
        )?;
        within(&catalog, |_| {
            planned_output(&args, &runtime, (&scopes, &overrides), test_planning.get())
        })
    })();
    let joined = test_planning.get().result(py);
    let planned = planned?;
    joined?;
    Ok(planned)
}
