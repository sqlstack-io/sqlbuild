//! A live plan's warehouse reads: the adapter and planning connection, read only when the planner asks.

use std::sync::Arc;
use std::time::Instant;

use pyo3::exceptions::{PyException, PyRuntimeError};
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyBool, PyCFunction, PyDict, PyFloat, PyString, PyTuple};
use pyo3::{IntoPyObject, pyfunction, wrap_pyfunction};
use sqlbuild_host::entries::models::AdapterHost;

use crate::bindings::_helpers::boundary::python_calls::{call_with, imported, keywords, within};
use crate::bindings::_helpers::fingerprints::storage_io::read_latest_fingerprints;
use crate::bindings::_helpers::freshness::state_io::{StateRead, read_latest};
use crate::bindings::_helpers::native_host::consumption_io::ConsumptionConnection;
use crate::bindings::_helpers::native_host::consumption_state::Location;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::observations::source_capture::capture_source_columns;
use crate::bindings::_helpers::planner_phases::objects::planner_error;
use crate::bindings::_helpers::planner_warehouse::constants::RUN_BOUNDED;
use crate::bindings::_helpers::planner_warehouse::reads::{
    CursorProgress, EligibleCursorQuery, FingerprintRead, PlannerReads, Read, WarehouseReads,
};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::state_rows::migration_io::{read_events, stored_old_name_columns};
use crate::bindings::_helpers::state_rows::models::{EventTableLayout, StateAccess};
use sqlbuild_state::consumption::models::ConsumptionRow;

const ACTIVE_CATALOG: AdapterHost = AdapterHost::ActiveInspectionCatalog;
const INSPECTION_CATALOG: AdapterHost = AdapterHost::OpenInspectionCatalog;
const RELATIONS_MAIN_LIST: AdapterHost = AdapterHost::ListRelationsForInspection;
const RELATIONS_MAIN_COLUMNS: AdapterHost = AdapterHost::GetColumnsForInspection;
const RECORD_QUERY: AdapterHost = AdapterHost::RecordInspectionQuery;
const RUN_RECORDED_QUERY: AdapterHost = AdapterHost::RunRecordedInspectionQuery;
const QUERY_RECORD: AdapterHost = AdapterHost::InspectionQueryRecord;
/// The adapter method that runs one statement on a connection.
const EXECUTE: &str = "execute";

/// The reads of one live plan: its adapter and planning connection, read only when asked.
pub(crate) struct LiveReads {
    adapter: Py<PyAny>,
    connection: Py<PyAny>,
    /// Runs SQL on the connection: the adapter's `execute` unless a caller observes statements.
    execute: Option<Py<PyAny>>,
    consumption: Option<ConsumptionConnection>,
}

fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

impl LiveReads {
    /// Reads through `connection`; consumption history connects with `connection_config` when given.
    pub(crate) fn new(
        adapter: &Bound<'_, PyAny>,
        connection: &Bound<'_, PyAny>,
        connection_config: Option<&Bound<'_, PyAny>>,
    ) -> Self {
        Self {
            adapter: adapter.clone().unbind(),
            connection: connection.clone().unbind(),
            execute: None,
            consumption: connection_config
                .filter(|config| !config.is_none())
                .map(|config| ConsumptionConnection::new(adapter, config)),
        }
    }

    /// Live reads as the handle a plan holds.
    pub(crate) fn handle<'py>(
        adapter: &Bound<'py, PyAny>,
        connection: &Bound<'py, PyAny>,
        connection_config: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PlannerReads>> {
        Bound::new(
            adapter.py(),
            PlannerReads::new(Arc::new(Self::new(adapter, connection, connection_config))),
        )
    }

    fn adapter<'py>(&self, py: Python<'py>) -> &Bound<'py, PyAny> {
        self.adapter.bind(py)
    }

    fn connection<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        self.connection.bind(py).clone()
    }

    fn method<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        if name == EXECUTE
            && let Some(execute) = &self.execute
        {
            return Ok(execute.bind(py).clone());
        }
        native_field(self.adapter(py), name)
    }

    /// The inspection catalog open for this connection, or `None`.
    fn active_catalog<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        call_with(
            &host::entry(py, ACTIVE_CATALOG)?,
            &[
                ("adapter", self.adapter(py).clone()),
                ("connection", self.connection(py)),
            ],
        )
    }

    fn state_access<'py>(&self, py: Python<'py>) -> PyResult<StateAccess<'py>> {
        Ok(StateAccess {
            connection: self.connection(py),
            execute: self.method(py, "execute")?,
            render_qualified_name: self.method(py, "render_qualified_name")?,
        })
    }

    fn consumption(&self) -> PyResult<&ConsumptionConnection> {
        self.consumption.as_ref().ok_or_else(|| {
            PyRuntimeError::new_err("live reads without a connection config read no consumption")
        })
    }
}

/// Run one cursor statement; a failure leaves only that relation unavailable.
fn read_cursor_query<'py>(
    py: Python<'py>,
    sql: &str,
    (connection, execute): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let started = Instant::now();
    let outcome = call_with(
        execute,
        &[("connection", connection.clone()), ("sql", text(py, sql))],
    )
    .and_then(|result| result.call_method0("fetchall"));
    let elapsed = started.elapsed().as_secs_f64();
    let (row, error, count) = match outcome {
        Ok(rows) => {
            let count = rows.len()?;
            let row = if count > 0 {
                rows.get_item(0)?
            } else {
                py.None().into_bound(py)
            };
            (
                row,
                py.None().into_bound(py),
                count.into_pyobject(py)?.into_any(),
            )
        }
        Err(error) if error.is_instance_of::<PyException>(py) => {
            let value = error.value(py).clone().into_any();
            (py.None().into_bound(py), value, py.None().into_bound(py))
        }
        Err(error) => return Err(error),
    };
    let error_text = if error.is_none() {
        py.None().into_bound(py)
    } else {
        error.str()?.into_any()
    };
    let record = call_with(
        &host::entry(py, QUERY_RECORD)?,
        &[
            ("sql", text(py, sql)),
            ("elapsed_seconds", PyFloat::new(py, elapsed).into_any()),
            ("row_count", count),
            ("error", error_text),
        ],
    )?;
    call_with(&host::entry(py, RECORD_QUERY)?, &[("record", record)])?;
    Ok(PyTuple::new(py, [row, PyFloat::new(py, elapsed).into_any(), error])?.into_any())
}

/// A bounded-inspection task running one cursor statement.
pub(crate) fn bound_task<'py>(
    py: Python<'py>,
    sql: String,
    (connection, execute): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let connection: Py<PyAny> = connection.clone().unbind();
    let execute: Py<PyAny> = execute.clone().unbind();
    Ok(PyCFunction::new_closure(
        py,
        None,
        None,
        move |args: &Bound<'_, PyTuple>,
              _kwargs: Option<&Bound<'_, PyDict>>|
              -> PyResult<Py<PyAny>> {
            let py = args.py();
            Ok(read_cursor_query(py, &sql, (connection.bind(py), execute.bind(py)))?.unbind())
        },
    )?
    .into_any())
}

/// A bounded-inspection task reading one model's highest eligible target cursor.
fn eligible_task<'py>(
    py: Python<'py>,
    query: EligibleCursorQuery,
    (connection, execute): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let connection: Py<PyAny> = connection.clone().unbind();
    let execute: Py<PyAny> = execute.clone().unbind();
    Ok(PyCFunction::new_closure(
        py,
        None,
        None,
        move |args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>| -> PyResult<Py<PyAny>> {
            let py = args.py();
            let run_connection = connection.clone_ref(py);
            let run_execute = execute.clone_ref(py);
            let run_sql = query.sql.clone();
            let run = PyCFunction::new_closure(
                py,
                None,
                None,
                move |args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>| -> PyResult<Py<PyAny>> {
                    let py = args.py();
                    Ok(run_execute
                        .bind(py)
                        .call((), Some(&keywords(py, &[
                            ("connection", run_connection.bind(py).clone()),
                            ("sql", text(py, &run_sql)),
                        ])?))?
                        .call_method0("fetchall")?
                        .unbind())
                },
            )?;
            let rows = call_with(
                &host::entry(py, RUN_RECORDED_QUERY)?,
                &[("sql", text(py, &query.sql)), ("run", run.into_any())],
            );
            match rows {
                Ok(rows) => Ok(rows.unbind()),
                Err(error) if error.is_instance_of::<PyException>(py) => {
                    let wrapped: PyErr = planner_error(
                        py,
                        &format!(
                            "model '{}': failed to query highest eligible target cursor for {}.{}: {}",
                            query.model_name,
                            query.target,
                            query.cursor_column,
                            error.value(py).str()?
                        ),
                        (None, None),
                    );
                    wrapped.set_cause(py, Some(error));
                    Err(wrapped)
                }
                Err(error) => Err(error),
            }
        },
    )?
    .into_any())
}

fn run_bounded<'py>(
    py: Python<'py>,
    tasks: Vec<Bound<'py, PyAny>>,
    concurrency: i64,
    on_complete: Option<Bound<'py, PyAny>>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut fields: Vec<(&str, Bound<'py, PyAny>)> = vec![
        ("tasks", PyTuple::new(py, tasks)?.into_any()),
        ("concurrency", concurrency.into_pyobject(py)?.into_any()),
    ];
    if let Some(on_complete) = on_complete {
        fields.push(("on_complete", on_complete));
    }
    call_with(&host::entry(py, RUN_BOUNDED)?, &fields)?
        .try_iter()?
        .collect()
}

impl WarehouseReads for LiveReads {
    fn relations<'py>(
        &self,
        py: Python<'py>,
        database: &Bound<'py, PyAny>,
        (schemas, names): (Bound<'py, PyAny>, Bound<'py, PyAny>),
    ) -> PyResult<Bound<'py, PyAny>> {
        call_with(
            &host::entry(py, RELATIONS_MAIN_LIST)?,
            &[
                ("adapter", self.adapter(py).clone()),
                ("connection", self.connection(py)),
                ("database", database.clone()),
                ("schemas", schemas),
                ("names", names),
            ],
        )
    }

    fn columns<'py>(
        &self,
        py: Python<'py>,
        relations: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        call_with(
            &host::entry(py, RELATIONS_MAIN_COLUMNS)?,
            &[
                ("adapter", self.adapter(py).clone()),
                ("connection", self.connection(py)),
                ("relations", relations),
            ],
        )
    }

    fn fingerprints<'py>(
        &self,
        py: Python<'py>,
        read: FingerprintRead<'_>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let node_names: Bound<'py, PyAny> = match &read.node_names {
            None => py.None().into_bound(py),
            Some(names) => PyTuple::new(py, names)?.into_any(),
        };
        read_latest_fingerprints(Some(&keywords(
            py,
            &[
                ("connection", self.connection(py)),
                ("execute", self.method(py, "execute")?),
                (
                    "table_exists",
                    PyBool::new(py, read.table_exists).to_owned().into_any(),
                ),
                ("database", read.database.into_pyobject(py)?.into_any()),
                ("schema", text(py, read.schema)),
                (
                    "render_qualified_name",
                    self.method(py, "render_qualified_name")?,
                ),
                (
                    "render_read_latest_sql",
                    self.method(py, "render_read_latest_fingerprints_sql")?,
                ),
                ("node_names", node_names),
                (
                    "filtered_node_types",
                    PyTuple::new(py, &read.filtered_node_types)?.into_any(),
                ),
            ],
        )?))
    }

    fn within_inspection<'py>(
        &self,
        py: Python<'py>,
        plan: &mut dyn FnMut() -> PyResult<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let catalog: Bound<'py, PyAny> = call_with(
            &host::entry(py, INSPECTION_CATALOG)?,
            &[
                ("adapter", self.adapter(py).clone()),
                ("connection", self.connection(py)),
            ],
        )?;
        within(&catalog, |_| plan())
    }

    fn inspection_concurrency(&self, py: Python<'_>) -> PyResult<i64> {
        let catalog = self.active_catalog(py)?;
        if catalog.is_none() {
            return Ok(1);
        }
        native_field(&catalog, "concurrency")?.extract()
    }

    fn prefetch_schema_listings<'py>(
        &self,
        py: Python<'py>,
        scopes: Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let catalog = self.active_catalog(py)?;
        if catalog.is_none() {
            return Ok(());
        }
        call_with(
            &native_field(&catalog, "prefetch_schema_listings")?,
            &[
                ("scopes", scopes),
                ("best_effort", PyBool::new(py, true).to_owned().into_any()),
            ],
        )?;
        Ok(())
    }

    fn lists_by_schema(&self, py: Python<'_>) -> PyResult<bool> {
        let catalog = self.active_catalog(py)?;
        Ok(!catalog.is_none() && native_field(&catalog, "schema_scoped")?.is_truthy()?)
    }

    fn cursor_bounds<'py>(
        &self,
        py: Python<'py>,
        statements: Vec<String>,
        progress: &dyn CursorProgress<'py>,
    ) -> PyResult<Read<Vec<Bound<'py, PyAny>>>> {
        let connection: Bound<'py, PyAny> = self.connection(py);
        let execute: Bound<'py, PyAny> = self.method(py, "execute")?;
        let concurrency: i64 = self.inspection_concurrency(py)?;
        let total: usize = statements.len();
        if concurrency <= 1 || total <= 1 {
            let mut outcomes: Vec<Bound<'py, PyAny>> = Vec::with_capacity(total);
            for (index, sql) in statements.iter().enumerate() {
                progress.starting(index)?;
                let outcome = read_cursor_query(py, sql, (&connection, &execute))?;
                progress.finished(index, &outcome)?;
                outcomes.push(outcome);
            }
            return Ok(Read::Known(outcomes));
        }
        progress.concurrent((concurrency as usize).min(total))?;
        let tasks: Vec<Bound<'py, PyAny>> = statements
            .into_iter()
            .map(|sql| bound_task(py, sql, (&connection, &execute)))
            .collect::<PyResult<_>>()?;
        Ok(Read::Known(run_bounded(
            py,
            tasks,
            concurrency,
            Some(progress.completion_callback(py)?),
        )?))
    }

    fn eligible_cursor_maxes<'py>(
        &self,
        py: Python<'py>,
        queries: Vec<EligibleCursorQuery>,
    ) -> PyResult<Read<Vec<Bound<'py, PyAny>>>> {
        let connection: Bound<'py, PyAny> = self.connection(py);
        let execute: Bound<'py, PyAny> = self.method(py, "execute")?;
        let tasks: Vec<Bound<'py, PyAny>> = queries
            .into_iter()
            .map(|query| eligible_task(py, query, (&connection, &execute)))
            .collect::<PyResult<_>>()?;
        Ok(Read::Known(run_bounded(
            py,
            tasks,
            self.inspection_concurrency(py)?,
            None,
        )?))
    }

    fn target_max(&self, model: &str) -> Read<String> {
        Read::Unknown(format!("cursor position of {model}").into())
    }

    fn note_window(&self, _model: &str, _start: &str) {}

    fn note_unknown(&self, _item: String) {}

    fn described_columns(
        &self,
        py: Python<'_>,
        source: &str,
        sql: &str,
    ) -> PyResult<Read<Vec<String>>> {
        let names = call_with(
            &self.method(py, "query_column_names")?,
            &[("connection", self.connection(py)), ("sql", text(py, sql))],
        )?;
        let described: Vec<String> = names
            .try_iter()?
            .map(|name| name?.extract())
            .collect::<PyResult<_>>()?;
        capture_source_columns(source, sql, &described);
        Ok(Read::Known(described))
    }

    fn read_access<'py>(
        &self,
        py: Python<'py>,
        relations: Vec<Bound<'py, PyAny>>,
    ) -> PyResult<Read<Vec<Bound<'py, PyAny>>>> {
        let probe: Bound<'py, PyAny> = self.method(py, "probe_relation_read")?;
        let partial: Bound<'py, PyAny> = imported(py, "functools", "partial")?;
        let tasks: Vec<Bound<'py, PyAny>> = relations
            .into_iter()
            .map(|relation| {
                partial.call(
                    (&probe,),
                    Some(&keywords(
                        py,
                        &[("connection", self.connection(py)), ("relation", relation)],
                    )?),
                )
            })
            .collect::<PyResult<_>>()?;
        let concurrency: i64 = imported(py, "builtins", "int")?
            .call1((self.method(py, "metadata_inspection_concurrency")?,))?
            .extract()?;
        Ok(Read::Known(run_bounded(
            py,
            tasks,
            concurrency.max(1),
            None,
        )?))
    }

    fn source_freshness_state<'py>(
        &self,
        py: Python<'py>,
        location: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
        (table_exists, source_names): (bool, Option<&[String]>),
    ) -> PyResult<Read<Bound<'py, PyAny>>> {
        let read: StateRead<'py> = StateRead {
            connection: self.connection(py),
            execute: self.method(py, "execute")?,
            render_qualified_name: self.method(py, "render_qualified_name")?,
            render_read_latest_sql: self.method(py, "render_read_latest_source_freshness_sql")?,
        };
        Ok(Read::Known(read_latest(
            &read,
            location,
            table_exists,
            source_names,
        )?))
    }

    fn table_freshness<'py>(
        &self,
        py: Python<'py>,
        requests: Bound<'py, PyAny>,
    ) -> PyResult<Read<Bound<'py, PyAny>>> {
        Ok(Read::Known(call_with(
            &self.method(py, "get_tables_freshness_metadata")?,
            &[("connection", self.connection(py)), ("requests", requests)],
        )?))
    }

    fn single_table_freshness<'py>(
        &self,
        py: Python<'py>,
        [database, schema, name]: [Bound<'py, PyAny>; 3],
    ) -> PyResult<Read<Bound<'py, PyAny>>> {
        Ok(Read::Known(call_with(
            &self.method(py, "get_table_freshness_metadata")?,
            &[
                ("connection", self.connection(py)),
                ("database", database),
                ("schema", schema),
                ("name", name),
            ],
        )?))
    }

    fn freshness_query<'py>(
        &self,
        py: Python<'py>,
        sql: Bound<'py, PyAny>,
    ) -> PyResult<Read<Bound<'py, PyAny>>> {
        Ok(Read::Known(call_with(
            &self.method(py, "query")?,
            &[
                ("connection", self.connection(py)),
                ("sql", sql),
                ("limit", py.None().into_bound(py)),
            ],
        )?))
    }

    fn table_types(&self) -> Read<()> {
        Read::Known(())
    }

    fn retention<'py>(&self, py: Python<'py>) -> PyResult<Read<Bound<'py, PyAny>>> {
        Ok(Read::Known(imported(py, "functools", "partial")?.call(
            (self.method(py, "inspect_retention")?,),
            Some(&keywords(py, &[("connection", self.connection(py))])?),
        )?))
    }

    fn column_rename_unavailable<'py>(
        &self,
        py: Python<'py>,
        destination: Bound<'py, PyAny>,
    ) -> PyResult<Read<Bound<'py, PyAny>>> {
        Ok(Read::Known(call_with(
            &self.method(py, "column_rename_unavailable_reason")?,
            &[
                ("connection", self.connection(py)),
                ("destination", destination),
            ],
        )?))
    }

    fn migration_state_tables<'py>(
        &self,
        py: Python<'py>,
        database: &Bound<'py, PyAny>,
        filter: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    ) -> PyResult<Read<Bound<'py, PyAny>>> {
        Ok(Read::Known(self.relations(py, database, filter)?))
    }

    fn recorded_migrations<'py>(
        &self,
        py: Python<'py>,
        layout: &EventTableLayout,
        location: (Bound<'py, PyAny>, Bound<'py, PyAny>),
        stored: Option<&[String]>,
    ) -> PyResult<Read<Bound<'py, PyTuple>>> {
        Ok(Read::Known(read_events(
            &self.state_access(py)?,
            layout,
            location,
            stored,
        )?))
    }

    fn old_name_view_columns<'py>(
        &self,
        py: Python<'py>,
        location: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    ) -> PyResult<Read<Vec<String>>> {
        Ok(Read::Known(stored_old_name_columns(
            self.adapter(py),
            &self.connection(py),
            location,
        )?))
    }

    fn deferred_relations<'py>(
        &self,
        py: Python<'py>,
        (database, schema): (Bound<'py, PyAny>, Bound<'py, PyAny>),
    ) -> PyResult<Bound<'py, PyAny>> {
        call_with(
            &self.method(py, "list_relations")?,
            &[
                ("connection", self.connection(py)),
                ("database", database),
                ("schemas", PyTuple::new(py, [schema])?.into_any()),
            ],
        )
    }

    fn recent_consumption(
        &self,
        py: Python<'_>,
        location: &Location,
    ) -> PyResult<Vec<ConsumptionRow>> {
        self.consumption()?.read_recent(py, location)
    }

    fn consumption_executions(
        &self,
        py: Python<'_>,
        location: &Location,
        execution_ids: &[&str],
    ) -> PyResult<Vec<ConsumptionRow>> {
        self.consumption()?
            .read_executions(py, location, execution_ids)
    }

    fn close_consumption(&self, py: Python<'_>) -> PyResult<()> {
        match &self.consumption {
            Some(consumption) => consumption.close(py),
            None => Ok(()),
        }
    }
}

/// Live warehouse reads for planner functions called from Python, optionally observing statements.
#[pyfunction]
#[pyo3(signature = (*, adapter, connection, connection_config = None, execute = None))]
fn live_reads<'py>(
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    connection_config: Option<Bound<'py, PyAny>>,
    execute: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PlannerReads>> {
    let mut reads = LiveReads::new(adapter, connection, connection_config.as_ref());
    reads.execute = execute
        .filter(|execute| !execute.is_none())
        .map(Bound::unbind);
    Bound::new(adapter.py(), PlannerReads::new(Arc::new(reads)))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PlannerReads>()?;
    module.add_function(wrap_pyfunction!(live_reads, module)?)?;
    Ok(())
}
