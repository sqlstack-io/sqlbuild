//! The planner runtime and warehouse state: built from keyword arguments and read by planner phases.

use crate::planner_warehouse::models::MigrationFingerprintCache;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pymethods;
use pyo3::types::{PyDict, PyTuple};

use crate::planner_phases::models::PlannerRuntime;
use crate::planner_phases::models::PlannerWarehouseState;
use crate::planner_warehouse::models::PlannerReads;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::build_command::main::utc_now::utc_now;
use sqlbuild_python_base::native_host::models::Dialect;

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
                &["project", "dialect", "reads"],
                &[
                    "project_config",
                    "local_config",
                    "on_progress",
                    "invocation_time",
                    "node_results_location",
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
            dialect: Dialect::passed(args.value("dialect")?)?.unbind(),
            reads: PlannerReads::passed(args.value("reads")?)?.unbind(),
            project_config: args.value("project_config")?.unbind(),
            local_config: args.value("local_config")?.unbind(),
            on_progress: args.value("on_progress")?.unbind(),
            invocation_time: invocation_time.unbind(),
            node_results_location: args.value("node_results_location")?.extract()?,
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

/// The warehouse reads of a planner runtime passed as a Python object.
pub(crate) fn runtime_reads<'py>(
    runtime: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PlannerReads>> {
    Ok(runtime
        .cast::<PlannerRuntime>()?
        .get()
        .reads
        .bind(runtime.py())
        .clone())
}

/// The SQL dialect of a planner runtime passed as a Python object.
pub(crate) fn runtime_dialect<'py>(runtime: &Bound<'py, PyAny>) -> PyResult<Bound<'py, Dialect>> {
    Ok(runtime
        .cast::<PlannerRuntime>()?
        .get()
        .dialect
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
