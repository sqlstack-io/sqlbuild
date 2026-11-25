//! The warehouse statements of a `sqb test` or `sqb scenario test` run, with their compile time.

use std::time::Instant;

use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTypeMethods};
use sqlbuild_core::cost_allocation::main::statement_timings_sql::statement_timings_sql;
use sqlbuild_core::json::main::with_section::with_section;
use sqlbuild_core::progress::main::format_statement_summary::format_statement_summary;
use sqlbuild_core::progress::main::statement_summary_json::statement_summary_json;
use sqlbuild_core::progress::models::{StatementSummary, StatementTiming, StatementTimings};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_python_base::build_progress::main::progress::current_projector::current_projector;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;

use sqlbuild_python_front::cost_context::main::cost_aware::cost_aware;

/// The JSON document section the report adds.
const STATEMENTS_SECTION: &str = "statements";
/// Query identifiers per query-history read.
const QUERY_ID_BATCH: usize = 1_000;
/// Extra query-history window before the run started, for statements issued while planning.
const LOOKBACK_MARGIN_SECONDS: u64 = 600;
/// Connection and statement limits of the history read, so it cannot stall a finished run.
const CONNECTION_LIMITS: [(&str, u64); 2] = [("login_timeout", 5), ("network_timeout", 15)];
const STATEMENT_TIMEOUT_SECONDS: &str = "10";

/// The adapter and connection a run's statements went to, and when the run started.
pub(crate) struct StatementSource<'a, 'py> {
    pub(crate) adapter_name: &'a str,
    pub(crate) adapter: &'a Bound<'py, PyAny>,
    pub(crate) connection_config: &'a Bound<'py, PyAny>,
    pub(crate) started: Instant,
}

/// The statements this invocation finished, and what the warehouse reports about their time.
pub(crate) struct StatementReport {
    summary: StatementSummary,
    timing: StatementTiming,
}

impl StatementReport {
    /// The report of a `sqb test` invocation's adapter and connection.
    pub(crate) fn of_invocation(
        invocation: &Bound<'_, PyAny>,
        started: Instant,
        local: &StatementSummary,
    ) -> PyResult<Option<Self>> {
        Self::read_excluding(
            invocation.py(),
            &StatementSource {
                adapter_name: &invocation.getattr("adapter_name")?.extract::<String>()?,
                adapter: &invocation.getattr("adapter")?,
                connection_config: &invocation.getattr("connection_config")?,
                started,
            },
            local,
        )
    }

    /// The report of the installed projector without the statements of a local DuckDB batch.
    pub(crate) fn read_excluding(
        py: Python<'_>,
        source: &StatementSource<'_, '_>,
        local: &StatementSummary,
    ) -> PyResult<Option<Self>> {
        let Some(projector) = current_projector(py)? else {
            return Ok(None);
        };
        let mut summary: StatementSummary = projector.get().statement_summary()?;
        for (kind, count) in &local.by_kind {
            if let Some(total) = summary.by_kind.get_mut(kind) {
                *total = total.saturating_sub(*count);
            }
        }
        summary.by_kind.retain(|_, count| *count > 0);
        summary
            .query_ids
            .retain(|query_id| !local.query_ids.contains(query_id));
        let timing: StatementTiming = statement_timing(py, source, &summary.query_ids)?;
        Ok(Some(Self { summary, timing }))
    }

    /// The summary line, without a trailing newline; nothing when no statements ran.
    pub(crate) fn line(&self) -> Option<String> {
        format_statement_summary(&self.summary, &self.timing)
    }

    /// `payload` with the `statements` section added.
    pub(crate) fn with_section(&self, payload: &str) -> PyResult<String> {
        with_section(
            payload,
            STATEMENTS_SECTION,
            statement_summary_json(&self.summary, &self.timing),
        )
        .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))
    }
}

/// Compile and execution time from Snowflake query history; other adapters report none.
fn statement_timing(
    py: Python<'_>,
    source: &StatementSource<'_, '_>,
    query_ids: &[String],
) -> PyResult<StatementTiming> {
    let not_reported = || StatementTiming::NotReported(source.adapter_name.to_owned());
    if !cost_aware(source.adapter)? {
        return Ok(not_reported());
    }
    let capability: Bound<'_, PyAny> = source.adapter.call_method0("cost_capability")?;
    let query_history: Bound<'_, PyAny> =
        host_entry(py, ExecutorHost::CostCapability)?.getattr("SNOWFLAKE_QUERY_HISTORY")?;
    if !capability.eq(query_history)? {
        return Ok(not_reported());
    }
    if query_ids.is_empty() {
        return Ok(StatementTiming::Reported(StatementTimings::default()));
    }
    Ok(match read_timings(source, query_ids) {
        Ok(timings) => StatementTiming::Reported(timings),
        Err(error) if error.is_instance_of::<PyException>(py) => StatementTiming::Failed(format!(
            "query history read failed: {}",
            error.get_type(py).name()?
        )),
        Err(error) => return Err(error),
    })
}

fn read_timings(
    source: &StatementSource<'_, '_>,
    query_ids: &[String],
) -> PyResult<StatementTimings> {
    let py: Python<'_> = source.adapter.py();
    let config: Bound<'_, PyDict> = source.connection_config.downcast::<PyDict>()?.copy()?;
    let database: Option<String> = match config.get_item("database")? {
        Some(database) if !database.is_none() => Some(database.str()?.extract()?),
        _ => None,
    };
    for (name, limit) in CONNECTION_LIMITS {
        config.set_item(name, limit)?;
    }
    let parameters: Bound<'_, PyDict> = PyDict::new(py);
    parameters.set_item("STATEMENT_TIMEOUT_IN_SECONDS", STATEMENT_TIMEOUT_SECONDS)?;
    let options: Bound<'_, PyDict> = PyDict::new(py);
    options.set_item("statement_params", parameters)?;
    let lookback_seconds: u64 = source.started.elapsed().as_secs() + LOOKBACK_MARGIN_SECONDS;
    let connection: Bound<'_, PyAny> = source.adapter.call_method1("connect", (config,))?;
    let read = || -> PyResult<StatementTimings> {
        let mut timings: StatementTimings = StatementTimings::default();
        for batch in query_ids.chunks(QUERY_ID_BATCH) {
            let sql: String = statement_timings_sql(database.as_deref(), batch, lookback_seconds);
            let (count, compile_ms, execution_ms): (u64, f64, f64) = connection
                .call_method("execute", (sql,), Some(&options))?
                .call_method0("fetchone")?
                .extract()?;
            timings.timed_statements += count;
            timings.compile_ms += compile_ms;
            timings.execution_ms += execution_ms;
        }
        Ok(timings)
    };
    let timings: PyResult<StatementTimings> = read();
    let closed: PyResult<Bound<'_, PyAny>> = source.adapter.call_method1("close", (&connection,));
    let timings: StatementTimings = timings?;
    closed?;
    Ok(timings)
}
