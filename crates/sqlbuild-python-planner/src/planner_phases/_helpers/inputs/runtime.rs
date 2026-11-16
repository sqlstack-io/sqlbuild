//! The planner runtime and warehouse state: built from keyword arguments and read by planner phases.

use crate::planner_warehouse::models::MigrationFingerprintCache;
use pyo3::prelude::{Bound, Py, PyAny, PyResult, Python};
use pyo3::pymethods;
use pyo3::types::{PyDict, PyTuple};

use crate::planner_phases::models::{PlannerRuntime, PlannerWarehouseState};
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::build_command::main::utc_now::utc_now;

#[pymethods]
impl PlannerRuntime {
    #[new]
    #[pyo3(signature = (**kwargs))]
    fn new(py: Python<'_>, kwargs: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let args = KeywordArguments::read(
            py,
            "PlannerRuntime",
            kwargs,
            (
                &["project", "adapter", "connection"],
                &[
                    "project_config",
                    "local_config",
                    "on_progress",
                    "invocation_time",
                ],
            ),
        )?;
        let invocation_time = if args.passed("invocation_time")? {
            args.value("invocation_time")?
        } else {
            utc_now(py)?
        };
        Ok(Self {
            project: args.value("project")?.unbind(),
            adapter: args.value("adapter")?.unbind(),
            connection: args.value("connection")?.unbind(),
            project_config: args.value("project_config")?.unbind(),
            local_config: args.value("local_config")?.unbind(),
            on_progress: args.value("on_progress")?.unbind(),
            invocation_time: invocation_time.unbind(),
        })
    }
}

/// One field of a planner runtime passed as a Python object.
pub(crate) fn runtime_field<'py>(
    runtime: &Bound<'py, PyAny>,
    field: fn(&PlannerRuntime) -> &Py<PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    Ok(field(runtime.cast::<PlannerRuntime>()?.get())
        .bind(runtime.py())
        .clone())
}

#[pymethods]
impl PlannerWarehouseState {
    #[new]
    #[pyo3(signature = (**kwargs))]
    fn new(py: Python<'_>, kwargs: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let args = KeywordArguments::read(
            py,
            "PlannerWarehouseState",
            kwargs,
            (
                &["snapshot", "inspection_relations"],
                &[
                    "migration_entries",
                    "migration_warnings",
                    "column_migration_entries",
                    "column_rename_hints",
                    "old_name_view_entries",
                    "old_name_views",
                    "migration_fingerprints",
                ],
            ),
        )?;
        let entries = |name: &str| -> PyResult<Py<PyAny>> {
            Ok(if args.passed(name)? {
                args.value(name)?.unbind()
            } else {
                PyTuple::empty(py).into_any().unbind()
            })
        };
        Ok(Self {
            snapshot: args.value("snapshot")?.unbind(),
            inspection_relations: args.value("inspection_relations")?.unbind(),
            migration_entries: entries("migration_entries")?,
            migration_warnings: entries("migration_warnings")?,
            column_migration_entries: entries("column_migration_entries")?,
            column_rename_hints: entries("column_rename_hints")?,
            old_name_view_entries: entries("old_name_view_entries")?,
            old_name_views: entries("old_name_views")?,
            migration_fingerprints: if args.passed("migration_fingerprints")? {
                args.value("migration_fingerprints")?.unbind()
            } else {
                Py::new(py, MigrationFingerprintCache::open(py, None)?)?.into_any()
            },
        })
    }
}

/// One field of a planner warehouse state passed as a Python object.
pub(crate) fn warehouse_field<'py>(
    warehouse: &Bound<'py, PyAny>,
    field: fn(&PlannerWarehouseState) -> &Py<PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    Ok(field(warehouse.cast::<PlannerWarehouseState>()?.get())
        .bind(warehouse.py())
        .clone())
}
