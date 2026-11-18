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
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::write_console::write_console;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::native_host::main::execute::execute;
use sqlbuild_python_front::native_host::main::literal::literal;
use sqlbuild_state::runs::constants::RUN_COLUMN_TYPES;
use sqlbuild_state::runs::models::RunNodeCounts;
use sqlbuild_state::state_tables::main::values_insert_sql::values_insert_sql;

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

/// Write `row` to each share's tables over one connection per share, each on its own thread.
///
/// The writers run outside the calling thread's context, so a build's expired deadline does not
/// stop its finished row. A signal such as Ctrl+C that arrives meanwhile is returned once every
/// writer is done.
pub(crate) fn write_shares<'py>(
    (adapter, connection_config): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (tables, identity): (&[RunTable], &RunIdentity),
    shares: &[Vec<usize>],
    (row, mode): (RunRow<'_>, RowWrite),
) -> (Vec<ShareResult>, Option<PyErr>) {
    let py: Python<'py> = adapter.py();
    let unbound: (Py<PyAny>, Py<PyAny>) =
        (adapter.clone().unbind(), connection_config.clone().unbind());
    std::thread::scope(|scope| {
        let writers: Vec<_> = shares
            .iter()
            .map(|share| {
                let (adapter, connection_config) = (&unbound.0, &unbound.1);
                scope.spawn(move || {
                    Python::attach(|py| {
                        RunWriter {
                            adapter: adapter.bind(py),
                            connection_config: connection_config.bind(py),
                            identity,
                        }
                        .write_share(share, tables, (row, mode))
                    })
                })
            })
            .collect();
        let mut signal: Option<PyErr> = None;
        while !writers.iter().all(|writer| writer.is_finished()) {
            if signal.is_none()
                && let Err(error) = py.check_signals()
            {
                signal = Some(error);
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
        (shared, signal)
    })
}

/// Warn for each table whose write failed; an interrupt or exit is not a failed write and escapes.
pub(crate) fn warn_on_errors(
    py: Python<'_>,
    (tables, identity): (&[RunTable], &RunIdentity),
    phase: &str,
    shared: Vec<ShareResult>,
) -> PyResult<()> {
    let mut written: Vec<(usize, PyResult<()>)> = Vec::with_capacity(tables.len());
    let mut escaped: Option<PyErr> = None;
    for share in shared {
        match share.closed {
            Err(error) if error.is_instance_of::<PyException>(py) => {
                let _ = write_console(
                    py,
                    "stderr",
                    &format!("Warning: could not close a {RUNS_TABLE_NAME} connection: {error}\n"),
                );
            }
            Err(error) => {
                escaped.get_or_insert(error);
            }
            Ok(()) => {}
        }
        written.extend(share.written);
    }
    written.sort_by_key(|(index, _)| *index);
    for (index, result) in written {
        match result {
            Err(error) if error.is_instance_of::<PyException>(py) => {
                let _ = write_console(
                    py,
                    "stderr",
                    &format!(
                        "Warning: could not record that run {} {phase} in {}: {error}\n",
                        identity.run_id, tables[index].table
                    ),
                );
            }
            Err(error) => {
                escaped.get_or_insert(error);
            }
            Ok(()) => {}
        }
    }
    escaped.map_or(Ok(()), Err)
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
}

impl<'py> RunWriter<'_, 'py> {
    /// Write the row to each table of `share` over one connection; a failure stays with its table.
    ///
    /// Connect for these rows only, so loaders can still lock file warehouses during the build.
    fn write_share(
        &self,
        share: &[usize],
        tables: &[RunTable],
        (row, mode): (RunRow<'_>, RowWrite),
    ) -> ShareResult {
        let py: Python<'py> = self.adapter.py();
        let connected: PyResult<Bound<'py, PyAny>> = native_field(self.adapter, "connect")
            .and_then(|connect| connect.call1((self.connection_config,)));
        let connection: Bound<'py, PyAny> = match connected {
            Ok(connection) => connection,
            Err(error) => {
                return ShareResult {
                    written: share
                        .iter()
                        .map(|index| (*index, Err(error.clone_ref(py))))
                        .collect(),
                    closed: Ok(()),
                };
            }
        };
        let mut written: Vec<(usize, PyResult<()>)> = Vec::with_capacity(share.len());
        for index in share {
            let result: PyResult<()> = self.append_row(&connection, &tables[*index], (row, mode));
            let interrupted: bool =
                matches!(&result, Err(error) if !error.is_instance_of::<PyException>(py));
            written.push((*index, result));
            if interrupted {
                break;
            }
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
                        if error.is_instance_of::<PyException>(self.adapter.py()) {
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
