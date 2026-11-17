//! Public records of live warehouse reads.

use crate::freshness::main::read_latest::read_latest;
use crate::freshness::models::StateRead;
use crate::live_reads::_helpers::bounded::eligible_task;
use crate::live_reads::_helpers::bounded::read_cursor_query;
use crate::live_reads::_helpers::bounded::run_bounded;
use crate::live_reads::_helpers::bounded::text;
use crate::live_reads::_helpers::cursor_batches::run_cursor_batches;
use crate::live_reads::constants::ACTIVE_CATALOG;
use crate::live_reads::constants::EXECUTE;
use crate::live_reads::constants::INSPECTION_CATALOG;
use crate::live_reads::constants::RELATIONS_MAIN_COLUMNS;
use crate::live_reads::constants::RELATIONS_MAIN_LIST;
use crate::live_reads::main::bound_task::bound_task;
use crate::observations::main::capture_source_columns::capture_source_columns;
use crate::planner_warehouse::models::{
    AdapterRead, CursorStatements, EligibleCursorQuery, FingerprintRead, PlannerReads, Read,
    StateHistoryRead,
};
use crate::planner_warehouse::types::CursorProgress;
use crate::planner_warehouse::types::WarehouseReads;
use pyo3::IntoPyObject;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyBool;
use pyo3::types::PyTuple;
use sqlbuild_host::entries::models::AdapterHost;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::native_host::models::ConsumptionConnection;
use sqlbuild_python_front::native_host::models::Location;
use sqlbuild_python_front::state_rows::main::migrations::read_events::read_events;
use sqlbuild_python_front::state_rows::main::migrations::stored_old_name_columns::stored_old_name_columns;
use sqlbuild_python_front::state_rows::models::StateAccess;
use sqlbuild_python_project::fingerprints::main::read_latest_fingerprints::read_latest_fingerprints;
use sqlbuild_state::consumption::models::ConsumptionRow;
use std::sync::Arc;

/// The reads of one live plan: its adapter and planning connection, read only when asked.
#[derive(Debug)]
pub struct LiveReads {
    adapter: Py<PyAny>,
    connection: Py<PyAny>,
    /// Runs SQL on the connection: the adapter's `execute` unless a caller observes statements.
    pub(crate) execute: Option<Py<PyAny>>,
    consumption: Option<ConsumptionConnection>,
}

impl LiveReads {
    /// Reads through `connection`; consumption history connects with `connection_config` when given.
    pub fn new(
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

    /// These reads as the handle a plan holds.
    pub fn handle(self, py: Python<'_>) -> PyResult<Bound<'_, PlannerReads>> {
        Bound::new(py, PlannerReads::new(Arc::new(self)))
    }

    /// Call a host entry with this adapter and connection, plus `arguments`.
    fn host_call<'py>(
        &self,
        py: Python<'py>,
        entry: AdapterHost,
        arguments: Vec<(&str, Bound<'py, PyAny>)>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let mut keywords = vec![
            ("adapter", self.adapter(py).clone()),
            ("connection", self.connection(py)),
        ];
        keywords.extend(arguments);
        call_with(&host_entry(py, entry)?, &keywords)
    }

    /// Call an adapter method on this connection, plus `arguments`; the warehouse answers it.
    fn known_call<'py>(
        &self,
        py: Python<'py>,
        method: &str,
        arguments: Vec<(&str, Bound<'py, PyAny>)>,
    ) -> PyResult<Read<Bound<'py, PyAny>>> {
        let mut keywords = vec![("connection", self.connection(py))];
        keywords.extend(arguments);
        Ok(Read::Known(call_with(
            &self.method(py, method)?,
            &keywords,
        )?))
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
        self.host_call(py, ACTIVE_CATALOG, Vec::new())
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

impl WarehouseReads for LiveReads {
    fn relations<'py>(
        &self,
        py: Python<'py>,
        database: &Bound<'py, PyAny>,
        (schemas, names): (Bound<'py, PyAny>, Bound<'py, PyAny>),
    ) -> PyResult<Bound<'py, PyAny>> {
        self.host_call(
            py,
            RELATIONS_MAIN_LIST,
            vec![
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
        self.host_call(py, RELATIONS_MAIN_COLUMNS, vec![("relations", relations)])
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
        let catalog: Bound<'py, PyAny> = self.host_call(py, INSPECTION_CATALOG, Vec::new())?;
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
        statements: &dyn Fn() -> PyResult<CursorStatements>,
        progress: &dyn CursorProgress<'py>,
    ) -> PyResult<Read<Vec<Bound<'py, PyAny>>>> {
        let CursorStatements { singles, batches } = statements()?;
        let connection: Bound<'py, PyAny> = self.connection(py);
        let execute: Bound<'py, PyAny> = self.method(py, "execute")?;
        let concurrency: i64 = self.inspection_concurrency(py)?;
        let total: usize = singles.len();
        let mut outcomes: Vec<Option<Bound<'py, PyAny>>> = vec![None; total];
        let pending: Vec<usize> = if batches.is_empty() || total <= 1 {
            (0..total).collect()
        } else {
            progress.batched(
                batches.len(),
                (concurrency.max(1) as usize).min(batches.len()),
            )?;
            let sizes: Vec<usize> = batches.iter().map(|batch| batch.reads.len()).collect();
            let (answered, failed) = run_cursor_batches(
                py,
                &batches,
                (&connection, &execute),
                (concurrency, Some(progress.batch_callback(py, sizes)?)),
            )?;
            for (index, outcome) in answered {
                outcomes[index] = Some(outcome);
            }
            failed
        };
        if concurrency <= 1 || pending.len() <= 1 {
            for index in pending {
                progress.starting(index)?;
                let outcome =
                    read_cursor_query(py, &singles[index], (&connection, &execute), true)?;
                progress.finished(index, &outcome)?;
                outcomes[index] = Some(outcome);
            }
        } else {
            if batches.is_empty() {
                progress.concurrent((concurrency as usize).min(pending.len()))?;
            }
            let tasks: Vec<Bound<'py, PyAny>> = pending
                .iter()
                .map(|index| bound_task(py, singles[*index].clone(), (&connection, &execute)))
                .collect::<PyResult<_>>()?;
            let results = run_bounded(
                py,
                tasks,
                concurrency,
                Some(progress.completion_callback(py, pending.clone())?),
            )?;
            for (index, outcome) in pending.into_iter().zip(results) {
                outcomes[index] = Some(outcome);
            }
        }
        Ok(Read::Known(outcomes.into_iter().flatten().collect()))
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
        relations: &dyn Fn() -> PyResult<Vec<Bound<'py, PyAny>>>,
    ) -> PyResult<Read<Vec<Bound<'py, PyAny>>>> {
        let relations: Vec<Bound<'py, PyAny>> = relations()?;
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

    fn state_history<'py>(
        &self,
        py: Python<'py>,
        read: StateHistoryRead<'_, 'py>,
    ) -> PyResult<Read<Bound<'py, PyAny>>> {
        Ok(Read::Known(match read {
            StateHistoryRead::SourceFreshness {
                location,
                table_exists,
                source_names,
            } => {
                let read: StateRead<'py> = StateRead {
                    connection: self.connection(py),
                    execute: self.method(py, "execute")?,
                    render_qualified_name: self.method(py, "render_qualified_name")?,
                    render_read_latest_sql: self
                        .method(py, "render_read_latest_source_freshness_sql")?,
                };
                read_latest(&read, location, table_exists, source_names)?
            }
            StateHistoryRead::MigrationTables { database, filter } => {
                self.relations(py, database, filter)?
            }
            StateHistoryRead::MigrationEvents {
                layout,
                location,
                stored,
            } => read_events(&self.state_access(py)?, layout, location, stored)?.into_any(),
        }))
    }

    fn adapter_read<'py>(
        &self,
        py: Python<'py>,
        read: AdapterRead<'py>,
    ) -> PyResult<Read<Bound<'py, PyAny>>> {
        let (method, arguments) = match read {
            AdapterRead::TableFreshness(requests) => (
                "get_tables_freshness_metadata",
                vec![("requests", requests)],
            ),
            AdapterRead::SingleTableFreshness([database, schema, name]) => (
                "get_table_freshness_metadata",
                vec![("database", database), ("schema", schema), ("name", name)],
            ),
            AdapterRead::FreshnessQuery(sql) => (
                "query",
                vec![("sql", sql), ("limit", py.None().into_bound(py))],
            ),
            AdapterRead::ColumnRenameUnavailable(destination) => (
                "column_rename_unavailable_reason",
                vec![("destination", destination)],
            ),
        };
        self.known_call(py, method, arguments)
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
