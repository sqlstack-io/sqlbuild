//! Writes of one run's rows to the runs table of every schema it writes, over at most a few connections.

use crate::native_host::_helpers::runs_state::CREATE_TABLE_METHOD;
use crate::native_host::_helpers::runs_state::WAREHOUSE_USER_METHOD;
use crate::native_host::_helpers::runs_state::row_cells;
use crate::native_host::constants::RUN_RECORD_SIGNAL_POLL_INTERVAL;
use crate::native_host::models::{RunIdentity, RunTable};
use pyo3::PyErr;
use pyo3::exceptions::{PyException, PyRuntimeError};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_core::constants::RUNS_TABLE_NAME;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::native_host::main::write_console::write_console;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::native_host::main::execute::execute;
use sqlbuild_python_front::native_host::main::literal::literal;
use sqlbuild_state::runs::constants::RUN_COLUMN_TYPES;
use sqlbuild_state::runs::models::RunNodeCounts;
use sqlbuild_state::state_tables::main::values_insert_sql::values_insert_sql;
use std::sync::atomic::{AtomicBool, Ordering};

/// A row's phase and, for `finished`, its status and node counts.
pub(crate) type RunRow<'a> = (&'a str, Option<(&'a str, &'a RunNodeCounts)>);

/// How a row reaches a schema's runs table.
#[derive(Clone, Copy, Debug)]
pub(crate) enum RowWrite {
    /// Set the schema and table up, then insert: the started row, once per run.
    SetUpFirst,
    /// Insert, and set the table up and insert again only when an ordinary error stops the insert.
    InsertFirst,
}

/// What one connection's share of schemas came to, by table index, and whether it closed.
pub(crate) struct ShareResult {
    pub(crate) written: Vec<(usize, PyResult<()>)>,
    pub(crate) closed: PyResult<()>,
}

/// What the writers of one row came to, and whether the run was interrupted while they ran.
pub(crate) struct SharesOutcome {
    pub(crate) shared: Vec<ShareResult>,
    /// The first signal, such as Ctrl+C, caught while waiting for the writers.
    pub(crate) signal: Option<PyErr>,
    /// Whether the writers were told to stop: a caught signal or an interrupt inside a writer.
    pub(crate) stopped: bool,
}

/// Write `row` to each share's tables on its own thread and connection; Ctrl+C stops and cancels them.
pub(crate) fn write_shares<'py>(
    (adapter, connection_config): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (tables, identity): (&[RunTable], &RunIdentity),
    shares: &[Vec<usize>],
    (row, mode): (RunRow<'_>, RowWrite),
) -> PyResult<SharesOutcome> {
    let py: Python<'py> = adapter.py();
    let context: Py<PyAny> = imported(py, "contextvars", "copy_context")?
        .call0()?
        .unbind();
    let unbound: (Py<PyAny>, Py<PyAny>) =
        (adapter.clone().unbind(), connection_config.clone().unbind());
    let stop: AtomicBool = AtomicBool::new(false);
    Ok(std::thread::scope(|scope| {
        let writers: Vec<_> = shares
            .iter()
            .map(|share| {
                let (adapter, connection_config) = (&unbound.0, &unbound.1);
                let (context, stop) = (&context, &stop);
                scope.spawn(move || {
                    Python::attach(|py| {
                        RunWriter {
                            adapter: adapter.bind(py),
                            connection_config: connection_config.bind(py),
                            identity,
                            stop,
                        }
                        .write_share_in(
                            context.bind(py),
                            share,
                            tables,
                            (row, mode),
                        )
                    })
                })
            })
            .collect();
        let mut signal: Option<PyErr> = None;
        while !writers.iter().all(|writer| writer.is_finished()) {
            if let Err(caught) = py.check_signals() {
                stop.store(true, Ordering::SeqCst);
                let _ = host_entry(py, ExecutorHost::CancelRunningStatements)
                    .and_then(|cancel| cancel.call0());
                signal.get_or_insert(caught);
            }
            py.detach(|| std::thread::sleep(RUN_RECORD_SIGNAL_POLL_INTERVAL));
        }
        let shared: Vec<ShareResult> = writers
            .into_iter()
            .zip(shares)
            .map(|(writer, share)| {
                py.detach(|| writer.join())
                    .unwrap_or_else(|_| panicked_share(share))
            })
            .collect();
        SharesOutcome {
            shared,
            signal,
            stopped: stop.load(Ordering::SeqCst),
        }
    }))
}

/// Warn for each failed write unless the run was stopped; return an interrupt to escape.
pub(crate) fn warn_on_errors(
    py: Python<'_>,
    (tables, identity): (&[RunTable], &RunIdentity),
    (phase, stopped): (&str, bool),
    shared: Vec<ShareResult>,
) -> Option<PyErr> {
    let mut written: Vec<(usize, PyResult<()>)> = Vec::with_capacity(tables.len());
    let mut escaped: Option<PyErr> = None;
    for share in shared {
        match share.closed {
            Err(error) if !error.is_instance_of::<PyException>(py) => {
                escaped.get_or_insert(error);
            }
            Err(error) if !stopped => {
                let _ = write_console(
                    py,
                    "stderr",
                    &format!("Warning: could not close a {RUNS_TABLE_NAME} connection: {error}\n"),
                );
            }
            _ => {}
        }
        written.extend(share.written);
    }
    written.sort_by_key(|(index, _)| *index);
    for (index, result) in written {
        match result {
            Err(error) if !error.is_instance_of::<PyException>(py) => {
                escaped.get_or_insert(error);
            }
            Err(error) if !stopped => {
                let _ = write_console(
                    py,
                    "stderr",
                    &format!(
                        "Warning: could not record that run {} {phase} in {}: {error}\n",
                        identity.run_id, tables[index].table
                    ),
                );
            }
            _ => {}
        }
    }
    escaped
}

/// Set the caller's context variables, from `context`, in this writer thread's own context.
fn adopt_context(context: &Bound<'_, PyAny>) -> PyResult<()> {
    for item in context.call_method0("items")?.try_iter()? {
        let (variable, value): (Bound<'_, PyAny>, Bound<'_, PyAny>) = item?.extract()?;
        variable.call_method1("set", (value,))?;
    }
    Ok(())
}

/// What a share came to when its writer thread panicked: every one of its tables failed.
fn panicked_share(share: &[usize]) -> ShareResult {
    let failed = |index: &usize| {
        (
            *index,
            Err(PyRuntimeError::new_err("the run record writer panicked")),
        )
    };
    ShareResult {
        written: share.iter().map(failed).collect(),
        closed: Ok(()),
    }
}

/// Writes one run's rows over one connection, on the thread that owns it.
struct RunWriter<'a, 'py> {
    adapter: &'a Bound<'py, PyAny>,
    connection_config: &'a Bound<'py, PyAny>,
    identity: &'a RunIdentity,
    /// Set once the run is interrupted: no further table is written.
    stop: &'a AtomicBool,
}

impl<'py> RunWriter<'_, 'py> {
    /// Write the share in a copy of `context`, the caller's, with every deadline suspended.
    fn write_share_in(
        &self,
        context: &Bound<'py, PyAny>,
        share: &[usize],
        tables: &[RunTable],
        row: (RunRow<'_>, RowWrite),
    ) -> ShareResult {
        let py: Python<'py> = self.adapter.py();
        let suspended: PyResult<Bound<'py, PyAny>> = adopt_context(context).and_then(|()| {
            let suspended = host_entry(py, ExecutorHost::ExecutionDeadlinesSuspended)?.call0()?;
            suspended.call_method0("__enter__")?;
            Ok(suspended)
        });
        let suspended: Bound<'py, PyAny> = match suspended {
            Ok(suspended) => suspended,
            Err(error) => return self.failed_share(share, &error),
        };
        let mut result: ShareResult = self.write_share(share, tables, row);
        let exited: PyResult<()> = suspended
            .call_method1("__exit__", (py.None(), py.None(), py.None()))
            .map(|_| ());
        result.closed = result.closed.and(exited);
        result
    }

    /// Every table of `share` failed with `error`; an interrupt also stops the other writers.
    fn failed_share(&self, share: &[usize], error: &PyErr) -> ShareResult {
        let py: Python<'py> = self.adapter.py();
        self.note_interrupt(error);
        ShareResult {
            written: share
                .iter()
                .map(|index| (*index, Err(error.clone_ref(py))))
                .collect(),
            closed: Ok(()),
        }
    }

    /// Stop the other writers when `error` is an interrupt or exit, not an ordinary failure.
    fn note_interrupt(&self, error: &PyErr) {
        if !error.is_instance_of::<PyException>(self.adapter.py()) {
            self.stop.store(true, Ordering::SeqCst);
        }
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// Write the row to each table of `share` over one connection, until the run is interrupted.
    fn write_share(
        &self,
        share: &[usize],
        tables: &[RunTable],
        (row, mode): (RunRow<'_>, RowWrite),
    ) -> ShareResult {
        if self.stopped() {
            return ShareResult {
                written: Vec::new(),
                closed: Ok(()),
            };
        }
        let connected: PyResult<Bound<'py, PyAny>> = native_field(self.adapter, "connect")
            .and_then(|connect| connect.call1((self.connection_config,)));
        let connection: Bound<'py, PyAny> = match connected {
            Ok(connection) => connection,
            Err(error) => return self.failed_share(share, &error),
        };
        let mut written: Vec<(usize, PyResult<()>)> = Vec::with_capacity(share.len());
        for index in share {
            if self.stopped() {
                break;
            }
            let result: PyResult<()> = self.append_row(&connection, &tables[*index], (row, mode));
            if let Err(error) = &result {
                self.note_interrupt(error);
            }
            written.push((*index, result));
        }
        let closed: PyResult<()> = native_field(self.adapter, "close")
            .and_then(|close| close.call1((&connection,)))
            .map(|_| ());
        ShareResult { written, closed }
    }

    /// Write one row to one table; a retry after an ordinary error reuses the same connection.
    fn append_row(
        &self,
        connection: &Bound<'py, PyAny>,
        table: &RunTable,
        (row, mode): (RunRow<'_>, RowWrite),
    ) -> PyResult<()> {
        match mode {
            RowWrite::SetUpFirst => self.append((connection, table), row, true),
            RowWrite::InsertFirst => {
                self.append((connection, table), row, false)
                    .or_else(|error| {
                        if error.is_instance_of::<PyException>(self.adapter.py()) && !self.stopped()
                        {
                            self.append((connection, table), row, true)
                        } else {
                            Err(error)
                        }
                    })
            }
        }
    }

    /// The schema is created only because the build writes to it; it never exists just for state.
    fn append(
        &self,
        (connection, table): (&Bound<'py, PyAny>, &RunTable),
        row: RunRow<'_>,
        ensure_table: bool,
    ) -> PyResult<()> {
        let py: Python<'py> = self.adapter.py();
        if ensure_table {
            let keywords = table.location.keywords(py);
            for statement in call_with(
                &native_field(self.adapter, "render_create_schema")?,
                &keywords,
            )?
            .try_iter()?
            {
                execute(self.adapter, connection, &statement?.extract::<String>()?)?;
            }
            let create: String =
                call_with(&native_field(self.adapter, CREATE_TABLE_METHOD)?, &keywords)?
                    .extract()?;
            if !create.is_empty() {
                execute(self.adapter, connection, &create)?;
            }
        }
        let recorded_at: String = call_with(
            &native_field(self.adapter, "render_current_utc_timestamp")?,
            &[],
        )?
        .extract()?;
        let warehouse_user: String =
            match call_with(&native_field(self.adapter, WAREHOUSE_USER_METHOD)?, &[])?
                .extract::<Option<String>>()?
            {
                Some(expression) => expression,
                None => literal(py, None)?,
            };
        let columns: Vec<&str> = RUN_COLUMN_TYPES.iter().map(|(column, _)| *column).collect();
        let cells: Vec<String> =
            row_cells(py, self.identity, row, (&warehouse_user, &recorded_at))?;
        execute(
            self.adapter,
            connection,
            &values_insert_sql(&table.table, &columns, &[cells]),
        )
        .map(|_| ())
    }
}
