//! Snowflake query-history cost collection for one run, attributed through the statement ledger.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use pyo3::exceptions::{PyTimeoutError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyCFunction, PyDict, PyList, PyListMethods, PyString, PyStringMethods, PyTuple};
use pyo3::{IntoPyObjectExt, pyfunction};
use sqlbuild_analysis::snowflake_cost::main::metadata_only_statement::metadata_only_statement;
use sqlbuild_analysis::snowflake_cost::main::sqlbuild_query_tag::sqlbuild_query_tag;
use sqlbuild_analysis::snowflake_cost::models::QueryTag;
use sqlbuild_core::cost_allocation::constants::QUERY_HISTORY_RESULT_LIMIT;
use sqlbuild_core::cost_allocation::main::query_history_sql::query_history_sql;
use sqlbuild_core::cost_allocation::models::{QueryHistoryWindow, RunAllocation};
use sqlbuild_core::cost_command::constants::{
    INCOMPLETE_HISTORY_LIMITATION, MISSING_METADATA_LIMITATION,
};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::cost_telemetry::_helpers::allocation::SummaryState;
use crate::cost_telemetry::_helpers::allocation::allocate_observations;
use crate::cost_telemetry::_helpers::allocation::priced;
use crate::cost_telemetry::_helpers::allocation::summary_object;
use crate::cost_telemetry::_helpers::health::consume_ledger_failure;
use crate::cost_telemetry::_helpers::ledger::read_ledger;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::cursor::constants::DATETIME_MODULE;
use sqlbuild_python_front::native_host::main::elapsed_seconds::elapsed_seconds;
use sqlbuild_python_front::native_host::main::seconds_since::seconds_since;

/// Default budget of one collection, which the at-exit collection keeps short.
const DEADLINE_SECONDS: f64 = 30.0;
/// Statement timeout of one query-history read.
const QUERY_TIMEOUT_SECONDS: &str = "10";
/// Narrowest slice still split in half after a statement timeout.
const MIN_TIMEOUT_SPLIT_SECONDS: i64 = 60;
const STATEMENT_TIMEOUT_CODE: &str = "000630";
const RUNNING_STATUS: &str = "RUNNING";
const HOOK_RESOURCE_TYPE: &str = "hook";
const ROW_WIDTH: usize = 14;
/// Seconds after which a finished query still without a warehouse size, such as DDL, used none.
const METADATA_SETTLE_SECONDS: f64 = 120.0;
const NO_WAREHOUSE_COMPUTE: &str = "no_warehouse_compute";
const MISSING_WAREHOUSE_METADATA: &str = "missing_warehouse_metadata";
const INCOMPLETE_HISTORY: &str = "incomplete_history";
const DEADLINE_MESSAGE: &str = "Snowflake cost collection deadline exceeded";
const VISIBLE_QUERY_LIMITATION: &str =
    "Concurrency sharing includes only queries visible to the executing Snowflake role.";
const RESULT_LIMIT_LIMITATION: &str = "Snowflake query-history result limit was reached.";
const LEDGER_FAILURE_PREFIX: &str = "Statement-ledger persistence failed (";
const PENDING_STATUS: &str = "PENDING";
const PARTIAL_STATUS: &str = "PARTIAL";
const COMPLETE_STATUS: &str = "COMPLETE";
const UNAVAILABLE_STATUS: &str = "UNAVAILABLE";

/// Connection, window inputs and deadline of every read; text is kept only for classified IDs.
struct History<'py> {
    connection: Bound<'py, PyAny>,
    database: Option<String>,
    text_run_id: Option<String>,
    classification_query_ids: HashSet<String>,
    deadline: f64,
    result_limit: usize,
}

/// One query-history snapshot: observations, observed IDs, skip reasons and saturation.
struct Observed<'py> {
    observations: Vec<Bound<'py, PyAny>>,
    observed: HashSet<String>,
    skipped: Vec<(String, &'static str)>,
    saturated: bool,
}

fn monotonic(py: Python<'_>) -> PyResult<f64> {
    imported(py, "time", "monotonic")?.call0()?.extract()
}

fn timedelta<'py>(py: Python<'py>, (unit, amount): (&str, i64)) -> PyResult<Bound<'py, PyAny>> {
    let options: Bound<'py, PyDict> = PyDict::new(py);
    options.set_item(unit, amount)?;
    imported(py, DATETIME_MODULE, "timedelta")?.call((), Some(&options))
}

fn check_deadline(history: &History<'_>, py: Python<'_>) -> PyResult<()> {
    if monotonic(py)? >= history.deadline {
        return Err(PyTimeoutError::new_err(DEADLINE_MESSAGE));
    }
    Ok(())
}

/// Whether a read's statement timeout is recovered from by reading smaller windows, so its failed statement is recorded as handled.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TimeoutRetry {
    Split,
    None,
}

/// Run `execute`, marking a statement timeout inside it as handled when the caller retries it.
fn with_timeout_retry<'py, T>(
    py: Python<'py>,
    retry: TimeoutRetry,
    execute: impl FnOnce() -> PyResult<T>,
) -> PyResult<T> {
    if retry == TimeoutRetry::None {
        return execute();
    }
    let is_timeout = PyCFunction::new_closure(
        py,
        None,
        None,
        |arguments: &Bound<'_, PyTuple>, _: Option<&Bound<'_, PyDict>>| -> PyResult<bool> {
            Ok(arguments
                .get_item(0)?
                .str()?
                .to_string_lossy()
                .contains(STATEMENT_TIMEOUT_CODE))
        },
    )?;
    let scope: Bound<'py, PyAny> = host_entry(py, ExecutorHost::StatementLifecycle)?
        .call_method1("handled_failure_scope", (is_timeout,))?;
    within(&scope, |_| execute())
}

fn history_rows<'py>(
    history: &History<'py>,
    (start, end): (&Bound<'py, PyAny>, Option<&Bound<'py, PyAny>>),
    retry: TimeoutRetry,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = start.py();
    check_deadline(history, py)?;
    let start_text: String = start.call_method0("isoformat")?.extract()?;
    let end_text: Option<String> = match end {
        Some(end) => Some(end.call_method0("isoformat")?.extract()?),
        None => None,
    };
    let sql: String = query_history_sql(&QueryHistoryWindow {
        database: history.database.as_deref(),
        start: &start_text,
        end: end_text.as_deref(),
        text_run_id: history.text_run_id.as_deref(),
        result_limit: history.result_limit,
    });
    let parameters: Bound<'py, PyDict> = PyDict::new(py);
    parameters.set_item("STATEMENT_TIMEOUT_IN_SECONDS", QUERY_TIMEOUT_SECONDS)?;
    let options: Bound<'py, PyDict> = PyDict::new(py);
    options.set_item("statement_params", parameters)?;
    let cursor: Bound<'py, PyAny> = with_timeout_retry(py, retry, || {
        history
            .connection
            .call_method("execute", (sql,), Some(&options))
    })?;
    check_deadline(history, py)?;
    let rows: Vec<Bound<'py, PyAny>> = cursor
        .call_method0("fetchall")?
        .try_iter()?
        .map(|row| {
            Ok(PyTuple::new(py, row?.try_iter()?.collect::<PyResult<Vec<_>>>()?)?.into_any())
        })
        .collect::<PyResult<_>>()?;
    check_deadline(history, py)?;
    Ok(rows)
}

fn strings(values: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    values.try_iter()?.map(|value| value?.extract()).collect()
}

fn query_id(row: &Bound<'_, PyAny>) -> PyResult<String> {
    row.get_item(0)?.str()?.extract()
}

/// Rows keyed by query ID, later rows replacing earlier ones, in query-ID order.
fn merged_rows<'py>(
    rows: impl Iterator<Item = Bound<'py, PyAny>>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut by_query_id: BTreeMap<String, Bound<'py, PyAny>> = BTreeMap::new();
    for row in rows {
        by_query_id.insert(query_id(&row)?, row);
    }
    Ok(by_query_id.into_values().collect())
}

/// Whether a query-history read failed on Snowflake's statement timeout.
pub(crate) fn statement_timeout(py: Python<'_>, error: &PyErr) -> bool {
    error
        .value(py)
        .str()
        .is_ok_and(|text| text.to_string_lossy().contains(STATEMENT_TIMEOUT_CODE))
}

/// End-time-bounded rows, halving the window while a read reaches the result limit or times out.
fn bounded_rows<'py>(
    history: &History<'py>,
    (start, end): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<(Vec<Bound<'py, PyAny>>, bool)> {
    let py: Python<'py> = start.py();
    let width: Bound<'py, PyAny> = end.sub(start)?;
    let retry: TimeoutRetry = if width.gt(timedelta(py, ("seconds", MIN_TIMEOUT_SPLIT_SECONDS))?)? {
        TimeoutRetry::Split
    } else {
        TimeoutRetry::None
    };
    let rows: Vec<Bound<'py, PyAny>> = match history_rows(history, (start, Some(end)), retry) {
        Ok(rows) => rows,
        Err(error) if retry == TimeoutRetry::Split && statement_timeout(py, &error) => {
            return split_rows(history, (start, end), &width);
        }
        Err(error) => return Err(error),
    };
    if rows.len() < history.result_limit {
        return Ok((rows, false));
    }
    if width.le(timedelta(py, ("milliseconds", 1))?)? {
        return Ok((rows, true));
    }
    split_rows(history, (start, end), &width)
}

/// Rows of both halves of a bounded window, merged by query ID.
fn split_rows<'py>(
    history: &History<'py>,
    (start, end): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    width: &Bound<'py, PyAny>,
) -> PyResult<(Vec<Bound<'py, PyAny>>, bool)> {
    let midpoint: Bound<'py, PyAny> = start.add(width.div(2)?)?;
    let (left, left_saturated) = bounded_rows(history, (start, &midpoint))?;
    let (right, right_saturated) = bounded_rows(history, (&midpoint, end))?;
    Ok((
        merged_rows(left.into_iter().chain(right))?,
        left_saturated || right_saturated,
    ))
}

fn running(end_time: &Bound<'_, PyAny>, status: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(end_time.is_none() || status.str()?.extract::<String>()?.to_uppercase() == RUNNING_STATUS)
}

fn unpacked<'py>(row: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let fields: Vec<Bound<'py, PyAny>> = row.try_iter()?.collect::<PyResult<_>>()?;
    match fields.len() {
        ROW_WIDTH => Ok(fields),
        width if width > ROW_WIDTH => Err(PyValueError::new_err(format!(
            "too many values to unpack (expected {ROW_WIDTH})"
        ))),
        width => Err(PyValueError::new_err(format!(
            "not enough values to unpack (expected {ROW_WIDTH}, got {width})"
        ))),
    }
}

fn python_int<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    let value: Bound<'py, PyAny> = if value.is_truthy()? {
        value.clone()
    } else {
        0_i64.into_bound_py_any(py)?
    };
    py.get_type::<pyo3::types::PyInt>().call1((value,))
}

fn optional_str<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if value.is_none() {
        return Ok(value.clone());
    }
    Ok(value.str()?.into_any())
}

/// A history row as an observation, or the reason it carries no attributable compute.
fn row_observation<'py>(
    row: &Bound<'py, PyAny>,
    (require_complete, classify): (bool, bool),
) -> PyResult<Result<Bound<'py, PyAny>, &'static str>> {
    let py: Python<'py> = row.py();
    let fields: Vec<Bound<'py, PyAny>> = unpacked(row)?;
    let [
        query_id,
        query_tag,
        warehouse_name,
        warehouse_size,
        warehouse_type,
        cluster_number,
        start_time,
        end_time,
        execution_ms,
        bytes_scanned,
        execution_status,
        query_type,
        query_text,
        observed_at,
    ] = <[Bound<'py, PyAny>; ROW_WIDTH]>::try_from(fields)
        .map_err(|_| PyValueError::new_err("query history row width changed"))?;
    let mut milliseconds: Bound<'py, PyAny> = python_int(&execution_ms)?;
    if milliseconds.lt(0)? {
        milliseconds = 0_i64.into_bound_py_any(py)?;
    }
    let query_type: String = if query_type.is_truthy()? {
        query_type.str()?.extract()?
    } else {
        String::new()
    };
    let query_text: Option<String> = if classify && query_text.is_instance_of::<PyString>() {
        Some(query_text.extract()?)
    } else {
        None
    };
    if metadata_only_statement(&query_type, query_text.as_deref()) {
        return Ok(Err(NO_WAREHOUSE_COMPUTE));
    }
    let running: bool = running(&end_time, &execution_status)?;
    if require_complete && running {
        return Ok(Err(INCOMPLETE_HISTORY));
    }
    if milliseconds.eq(0)? || warehouse_name.is_none() || warehouse_size.is_none() {
        let settled: bool =
            !running && elapsed_seconds(&end_time, &observed_at)? >= METADATA_SETTLE_SECONDS;
        return Ok(Err(if settled {
            NO_WAREHOUSE_COMPUTE
        } else {
            MISSING_WAREHOUSE_METADATA
        }));
    }
    let tag: Option<QueryTag> = if query_tag.is_instance_of::<PyString>() {
        sqlbuild_query_tag(&query_tag.extract::<String>()?)
    } else {
        None
    };
    let effective_end: Bound<'py, PyAny> = if running { observed_at } else { end_time };
    let options: Bound<'py, PyDict> = PyDict::new(py);
    options.set_item("milliseconds", &milliseconds)?;
    let busy_start: Bound<'py, PyAny> =
        effective_end.sub(imported(py, DATETIME_MODULE, "timedelta")?.call((), Some(&options))?)?;
    let active_start: Bound<'py, PyAny> = if busy_start.gt(&start_time)? {
        busy_start
    } else {
        start_time
    };
    if effective_end.le(&active_start)? {
        return Ok(Err(NO_WAREHOUSE_COMPUTE));
    }
    let tag: QueryTag = tag.unwrap_or_default();
    let observation: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::QueryCostObservation,
        &[
            ("query_id", query_id.str()?.into_any()),
            ("warehouse_name", warehouse_name.str()?.into_any()),
            ("warehouse_size", warehouse_size.str()?.into_any()),
            ("warehouse_type", optional_str(&warehouse_type)?),
            (
                "cluster_number",
                if cluster_number.is_none() {
                    cluster_number
                } else {
                    py.get_type::<pyo3::types::PyInt>()
                        .call1((cluster_number,))?
                },
            ),
            ("started_at", active_start),
            ("completed_at", effective_end),
            ("execution_ms", milliseconds),
            ("bytes_scanned", python_int(&bytes_scanned)?),
            ("execution_status", execution_status.str()?.into_any()),
            ("run_id", tag.run_id.into_bound_py_any(py)?),
            ("resource_type", tag.resource_type.into_bound_py_any(py)?),
            ("resource_name", tag.resource_name.into_bound_py_any(py)?),
        ],
    )?;
    Ok(Ok(observation))
}

/// Open-ended rows from `start`, which include running queries, or bounded windows if saturated.
fn open_rows<'py>(
    history: &History<'py>,
    (start, window_end): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<(Vec<Bound<'py, PyAny>>, bool)> {
    let py: Python<'py> = start.py();
    let open_rows: Vec<Bound<'py, PyAny>> =
        match history_rows(history, (start, None), TimeoutRetry::Split) {
            Ok(rows) => rows,
            Err(error) if statement_timeout(py, &error) => {
                return bounded_rows(history, (start, window_end));
            }
            Err(error) => return Err(error),
        };
    if open_rows.len() < history.result_limit {
        return Ok((open_rows, false));
    }
    let (bounded, bounded_saturated) = bounded_rows(history, (start, window_end))?;
    let mut still_running: Vec<Bound<'py, PyAny>> = Vec::new();
    for row in open_rows {
        if running(&row.get_item(7)?, &row.get_item(10)?)? {
            still_running.push(row);
        }
    }
    Ok((
        merged_rows(still_running.into_iter().chain(bounded))?,
        bounded_saturated,
    ))
}

/// Observations of the run window: an open read while it is open, else only the window itself.
fn query_observations<'py>(
    history: &History<'py>,
    (started_at, completed_at): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    expected: &HashSet<String>,
) -> PyResult<Observed<'py>> {
    let py: Python<'py> = started_at.py();
    let skew: Bound<'py, PyAny> = timedelta(py, ("minutes", 10))?;
    let window_start: Bound<'py, PyAny> = started_at.sub(&skew)?;
    let window_end: Bound<'py, PyAny> = completed_at.add(&skew)?;
    let (rows, saturated) = if seconds_since(&window_end)?.is_sign_positive() {
        bounded_rows(history, (&window_start, &window_end))?
    } else {
        open_rows(history, (&window_start, &window_end))?
    };
    let observations: Bound<'py, PyList> = PyList::empty(py);
    let mut observed: HashSet<String> = HashSet::new();
    let mut skipped: Vec<(String, &'static str)> = Vec::new();
    for row in rows {
        let id: String = query_id(&row)?;
        observed.insert(id.clone());
        let classify: bool = history.classification_query_ids.contains(&id);
        match row_observation(&row, (expected.contains(&id), classify))? {
            Ok(observation) => observations.append(observation)?,
            Err(reason) => match skipped.iter_mut().find(|(skipped_id, _)| *skipped_id == id) {
                Some(entry) => entry.1 = reason,
                None => skipped.push((id, reason)),
            },
        }
    }
    let key: Bound<'py, PyAny> =
        imported(py, "operator", "attrgetter")?.call1(("started_at", "query_id"))?;
    let options: Bound<'py, PyDict> = PyDict::new(py);
    options.set_item("key", key)?;
    observations.call_method("sort", (), Some(&options))?;
    Ok(Observed {
        observations: observations.iter().collect(),
        observed,
        skipped,
        saturated,
    })
}

/// The budget a query-history read exhausted, or `None` for any other failure.
fn budget_exhausted(py: Python<'_>, error: &PyErr, budget_seconds: f64) -> Option<String> {
    if error.is_instance_of::<PyTimeoutError>(py)
        && error
            .value(py)
            .str()
            .is_ok_and(|text| text.to_string_lossy() == DEADLINE_MESSAGE)
    {
        return Some(format!(
            "{budget_seconds:.0}-second collection deadline exceeded"
        ));
    }
    statement_timeout(py, error).then(|| {
        format!("query-history read reached its {QUERY_TIMEOUT_SECONDS}-second statement timeout")
    })
}

/// A pending summary for a run whose history did not return within the collection budget.
fn budget_pending(
    (run_id, cause): (&str, &str),
    statement_count: usize,
    failure: Option<&str>,
) -> Summary {
    let mut pending: Summary = with_ledger_failure(
        partial(&format!(
            "Snowflake query history did not return within the cost collection budget \
({cause}). Run `sqb cost {run_id}` to retry."
        )),
        failure,
    );
    if failure.is_none() {
        pending.state.status = PENDING_STATUS;
    }
    pending.state.counts = [statement_count, 0, statement_count, 0];
    pending
}

/// A collected summary: the allocation it prices, if any, and its derived fields.
struct Summary {
    allocation: Option<RunAllocation>,
    state: SummaryState,
}

/// Sorted unique limitations after adding `added`.
fn with_limitations(
    limitations: &[String],
    added: impl IntoIterator<Item = String>,
) -> Vec<String> {
    let limitations: BTreeSet<String> = limitations.iter().cloned().chain(added).collect();
    limitations.into_iter().collect()
}

fn partial(message: &str) -> Summary {
    Summary {
        allocation: None,
        state: SummaryState {
            status: PARTIAL_STATUS,
            message: Some(message.to_owned()),
            limitations: Vec::new(),
            counts: [0; 4],
        },
    }
}

fn with_ledger_failure(mut summary: Summary, failure: Option<&str>) -> Summary {
    let Some(failure) = failure else {
        return summary;
    };
    summary.state.status = PARTIAL_STATUS;
    summary.state.limitations = with_limitations(
        &summary.state.limitations,
        [format!("{LEDGER_FAILURE_PREFIX}{failure}).")],
    );
    if summary.state.message.is_none() {
        summary.state.message = Some("Cost telemetry was only partially persisted.".to_owned());
    }
    summary
}

/// Ledger expectations a summary is checked against.
struct Expected<'a> {
    statement_count: usize,
    query_ids: &'a HashSet<String>,
}

fn with_observation_gaps(
    mut summary: Summary,
    expected: &Expected<'_>,
    observed: &Observed<'_>,
) -> Summary {
    let relevant: Vec<&'static str> = observed
        .skipped
        .iter()
        .filter(|(id, _)| expected.query_ids.contains(id))
        .map(|(_, reason)| *reason)
        .collect();
    let missing_metadata: bool = relevant.contains(&MISSING_WAREHOUSE_METADATA);
    let incomplete: bool = relevant.contains(&INCOMPLETE_HISTORY);
    let mut added: Vec<String> = Vec::new();
    if missing_metadata {
        added.push(MISSING_METADATA_LIMITATION.to_owned());
    }
    if incomplete {
        added.push(INCOMPLETE_HISTORY_LIMITATION.to_owned());
    }
    summary.state.limitations = with_limitations(&summary.state.limitations, added);
    let query_count: usize = summary
        .allocation
        .as_ref()
        .map_or(0, |allocation| allocation.query_count);
    if missing_metadata || (incomplete && query_count > 0) {
        summary.state.status = PARTIAL_STATUS;
    }
    let visible: usize = expected.query_ids.intersection(&observed.observed).count();
    summary.state.counts = [
        expected.statement_count,
        visible,
        expected.statement_count - visible,
        relevant.len(),
    ];
    summary
}

/// Whether every expected statement is visible and finished, lacking at most settling metadata.
fn awaits_only_metadata(expected: &HashSet<String>, observed: &Observed<'_>) -> bool {
    expected.is_subset(&observed.observed)
        && !observed
            .skipped
            .iter()
            .any(|(id, reason)| expected.contains(id) && *reason == INCOMPLETE_HISTORY)
}

fn incomplete_observation(expected: &HashSet<String>, observed: &Observed<'_>) -> bool {
    observed.skipped.iter().any(|(id, reason)| {
        expected.contains(id)
            && (*reason == MISSING_WAREHOUSE_METADATA || *reason == INCOMPLETE_HISTORY)
    })
}

/// `replace(observation, run_id=..., resource_type=..., resource_name=...)` from the ledger entry.
fn ledger_attributed<'py>(
    observation: &Bound<'py, PyAny>,
    ledger: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = observation.py();
    let entry: Option<Bound<'py, PyAny>> =
        pyo3::types::PyDictMethods::get_item(ledger, native_field(observation, "query_id")?)?;
    let field = |name: &str| -> PyResult<Bound<'py, PyAny>> {
        match &entry {
            Some(entry) => native_field(entry, name),
            None => Ok(py.None().into_bound(py)),
        }
    };
    let changes: Bound<'py, PyDict> = PyDict::new(py);
    changes.set_item("run_id", field("run_id")?)?;
    changes.set_item("resource_type", field("resource_type")?)?;
    changes.set_item("resource_name", field("resource_name")?)?;
    replaced_keywords(observation, &changes)
}

/// Collection inputs read from the keyword arguments.
struct Collection<'py> {
    run_id: Bound<'py, PyAny>,
    started_at: Bound<'py, PyAny>,
    completed_at: Bound<'py, PyAny>,
    usd_per_credit: Bound<'py, PyAny>,
    rate_source: Bound<'py, PyAny>,
    attempts: i64,
    retry_delay_seconds: f64,
    budget_seconds: f64,
}

fn collect<'py>(
    collection: &Collection<'py>,
    mut history: History<'py>,
    ledger_path: &Bound<'py, PyAny>,
) -> PyResult<Summary> {
    let py: Python<'py> = ledger_path.py();
    history.deadline = monotonic(py)? + collection.budget_seconds;
    let run_id: String = collection.run_id.extract()?;
    let ledger: Vec<Bound<'py, PyAny>> = read_ledger(ledger_path, &collection.run_id)?;
    let failure: Option<String> = consume_ledger_failure(py, &run_id)?;
    let by_query_id: Bound<'py, PyDict> = PyDict::new(py);
    for entry in &ledger {
        let id: Bound<'py, PyAny> = native_field(entry, "query_id")?;
        if !id.is_none() {
            by_query_id.set_item(id, entry)?;
        }
    }
    let mut expected_ids: HashSet<String> = HashSet::new();
    let mut classification: HashSet<String> = HashSet::new();
    for (id, entry) in pyo3::types::PyDictMethods::iter(&by_query_id) {
        let id: String = id.extract()?;
        if !native_field(&entry, "resource_type")?.eq(HOOK_RESOURCE_TYPE)? {
            classification.insert(id.clone());
        }
        expected_ids.insert(id);
    }
    history.text_run_id = (!classification.is_empty()).then(|| run_id.clone());
    history.classification_query_ids = classification;
    if !ledger.is_empty() && expected_ids.is_empty() {
        let mut summary: Summary = with_ledger_failure(
            partial("No Snowflake query IDs were available in the statement ledger."),
            failure.as_deref(),
        );
        summary.state.counts = [ledger.len(), 0, ledger.len(), 0];
        return Ok(summary);
    }
    let expected: Expected<'_> = Expected {
        statement_count: ledger.len(),
        query_ids: &expected_ids,
    };
    let mut summary: Option<Summary> = None;
    let mut observed: Observed<'py> = Observed {
        observations: Vec::new(),
        observed: HashSet::new(),
        skipped: Vec::new(),
        saturated: false,
    };
    for attempt in 0..collection.attempts {
        observed = match query_observations(
            &history,
            (&collection.started_at, &collection.completed_at),
            &expected_ids,
        ) {
            Ok(observed) => observed,
            Err(error) => {
                let Some(cause) = budget_exhausted(py, &error, collection.budget_seconds) else {
                    return Err(error);
                };
                if summary.is_some() {
                    break;
                }
                return Ok(budget_pending(
                    (&run_id, &cause),
                    ledger.len(),
                    failure.as_deref(),
                ));
            }
        };
        let attributed: Vec<Bound<'py, PyAny>> = observed
            .observations
            .iter()
            .map(|observation| ledger_attributed(observation, &by_query_id))
            .collect::<PyResult<_>>()?;
        let allocation: RunAllocation = allocate_observations(
            &attributed,
            (&run_id, &collection.usd_per_credit),
            observed.saturated,
        )?;
        let (allocation, mut state) = priced(allocation);
        state.limitations =
            with_limitations(&state.limitations, [VISIBLE_QUERY_LIMITATION.to_owned()]);
        let current: Summary = Summary { allocation, state };
        if expected_ids.is_subset(&observed.observed)
            && !incomplete_observation(&expected_ids, &observed)
        {
            let mut complete: Summary = with_observation_gaps(current, &expected, &observed);
            if complete.state.status == PENDING_STATUS {
                complete.state.status = COMPLETE_STATUS;
                complete.state.message =
                    Some("No cost-bearing Snowflake warehouse queries were observed.".to_owned());
            }
            return Ok(with_ledger_failure(complete, failure.as_deref()));
        }
        summary = Some(current);
        let last: bool =
            attempt + 1 >= collection.attempts || awaits_only_metadata(&expected_ids, &observed);
        if last || history.deadline - monotonic(py)? <= collection.retry_delay_seconds {
            break;
        }
        imported(py, "time", "sleep")?.call1((collection.retry_delay_seconds,))?;
    }
    let mut summary: Summary =
        summary.unwrap_or_else(|| partial("Snowflake query history could not be collected."));
    let missing: usize = expected_ids.difference(&observed.observed).count();
    let mut added: Vec<String> = Vec::new();
    if missing > 0 {
        added.push(format!(
            "{missing} statement ledger query ID(s) were not visible in query history."
        ));
    }
    if observed.saturated {
        added.push(RESULT_LIMIT_LIMITATION.to_owned());
    }
    summary.state.limitations = with_limitations(&summary.state.limitations, added);
    let visible: bool = expected_ids
        .intersection(&observed.observed)
        .next()
        .is_some();
    let now: Bound<'py, PyAny> = imported(py, DATETIME_MODULE, "datetime")?
        .call_method1("now", (imported(py, DATETIME_MODULE, "UTC")?,))?;
    let expired: bool = now
        .sub(&collection.completed_at)?
        .ge(timedelta(py, ("days", 7))?)?;
    if expired && !visible {
        summary.state.status = UNAVAILABLE_STATUS;
        summary.state.message =
            Some("Snowflake query history is no longer available for this run.".to_owned());
    } else if !visible && !observed.saturated {
        summary.state.status = PENDING_STATUS;
        summary.state.message = Some(
            "Snowflake query history is not complete yet. Run `sqb cost latest` to refresh."
                .to_owned(),
        );
    } else {
        summary.state.status = PARTIAL_STATUS;
        if summary.state.message.is_none() {
            summary.state.message =
                Some("Snowflake query history was only partially available.".to_owned());
        }
    }
    let summary: Summary = with_observation_gaps(summary, &expected, &observed);
    Ok(with_ledger_failure(summary, failure.as_deref()))
}

/// Collect visible history and attribute exact ledger query IDs to one run.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn collect_snowflake_cost<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "collect_snowflake_cost",
        options,
        (
            &[
                "connection",
                "run_id",
                "started_at",
                "completed_at",
                "statement_ledger_path",
                "usd_per_credit",
                "rate_source",
            ],
            &[
                "database",
                "attempts",
                "retry_delay_seconds",
                "result_limit",
                "budget_seconds",
            ],
        ),
    )?;
    let optional = |name: &str, default: Bound<'py, PyAny>| -> PyResult<Bound<'py, PyAny>> {
        Ok(if arguments.contains(name)? {
            arguments.value(name)?
        } else {
            default
        })
    };
    let collection: Collection<'py> = Collection {
        run_id: arguments.value("run_id")?,
        started_at: arguments.value("started_at")?,
        completed_at: arguments.value("completed_at")?,
        usd_per_credit: arguments.value("usd_per_credit")?,
        rate_source: arguments.value("rate_source")?,
        attempts: optional("attempts", 3_i64.into_bound_py_any(py)?)?.extract()?,
        retry_delay_seconds: optional("retry_delay_seconds", 1.0_f64.into_bound_py_any(py)?)?
            .extract()?,
        budget_seconds: optional("budget_seconds", py.None().into_bound(py))?
            .extract::<Option<f64>>()?
            .unwrap_or(DEADLINE_SECONDS),
    };
    let history: History<'py> = History {
        connection: arguments.value("connection")?,
        database: arguments.value("database")?.extract()?,
        text_run_id: None,
        classification_query_ids: HashSet::new(),
        deadline: 0.0,
        result_limit: optional(
            "result_limit",
            QUERY_HISTORY_RESULT_LIMIT.into_bound_py_any(py)?,
        )?
        .extract()?,
    };
    let summary: Summary = collect(
        &collection,
        history,
        &arguments.value("statement_ledger_path")?,
    )?;
    summary_object(
        summary.allocation.as_ref(),
        &summary.state,
        (&collection.usd_per_credit, &collection.rate_source),
    )
}

/// Query-history observations of one window, as cost collection reads them.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn snowflake_query_observations<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "snowflake_query_observations",
        options,
        (
            &[
                "connection",
                "started_at",
                "completed_at",
                "expected_query_ids",
                "classification_query_ids",
                "deadline",
            ],
            &["database", "result_limit", "run_id"],
        ),
    )?;
    let result_limit: Bound<'py, PyAny> = arguments.value("result_limit")?;
    let classification_query_ids: HashSet<String> =
        strings(&arguments.value("classification_query_ids")?)?
            .into_iter()
            .collect();
    let history: History<'py> = History {
        connection: arguments.value("connection")?,
        database: arguments.value("database")?.extract()?,
        text_run_id: if classification_query_ids.is_empty() {
            None
        } else {
            arguments.value("run_id")?.extract()?
        },
        classification_query_ids,
        deadline: arguments.value("deadline")?.extract()?,
        result_limit: if result_limit.is_none() {
            QUERY_HISTORY_RESULT_LIMIT
        } else {
            result_limit.extract()?
        },
    };
    let expected: HashSet<String> = strings(&arguments.value("expected_query_ids")?)?
        .into_iter()
        .collect();
    let observed: Observed<'py> = query_observations(
        &history,
        (
            &arguments.value("started_at")?,
            &arguments.value("completed_at")?,
        ),
        &expected,
    )?;
    let skipped: Bound<'py, PyDict> = PyDict::new(py);
    for (id, reason) in &observed.skipped {
        skipped.set_item(id, reason)?;
    }
    PyTuple::new(
        py,
        [
            PyTuple::new(py, &observed.observations)?.into_any(),
            pyo3::types::PyFrozenSet::new(py, &observed.observed)?.into_any(),
            skipped.into_any(),
            observed.saturated.into_bound_py_any(py)?,
        ],
    )
}

/// The query-history SQL of one window; an absent end reads every query ending after the start.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn render_snowflake_query_history_sql(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<String> {
    let arguments: KeywordArguments<'_> = KeywordArguments::read(
        py,
        "render_snowflake_query_history_sql",
        options,
        (
            &["started_at"],
            &["database", "completed_at", "result_limit", "run_id"],
        ),
    )?;
    let start: String = arguments
        .value("started_at")?
        .call_method0("isoformat")?
        .extract()?;
    let completed_at: Bound<'_, PyAny> = arguments.value("completed_at")?;
    let end: Option<String> = if completed_at.is_none() {
        None
    } else {
        Some(completed_at.call_method0("isoformat")?.extract()?)
    };
    let result_limit: Bound<'_, PyAny> = arguments.value("result_limit")?;
    let database: Option<String> = arguments.value("database")?.extract()?;
    let run_id: Option<String> = arguments.value("run_id")?.extract()?;
    Ok(query_history_sql(&QueryHistoryWindow {
        database: database.as_deref(),
        start: &start,
        end: end.as_deref(),
        text_run_id: run_id.as_deref(),
        result_limit: if result_limit.is_none() {
            QUERY_HISTORY_RESULT_LIMIT
        } else {
            result_limit.extract()?
        },
    }))
}

/// Whether Snowflake runs a statement of this type and text without warehouse compute.
#[pyfunction]
#[pyo3(signature = (*, query_type, query_text))]
pub(crate) fn snowflake_statement_is_metadata_only(
    query_type: &Bound<'_, PyAny>,
    query_text: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    let query_type: String = if query_type.is_truthy()? {
        query_type.str()?.extract()?
    } else {
        String::new()
    };
    let query_text: Option<String> = if query_text.is_instance_of::<PyString>() {
        Some(query_text.extract()?)
    } else {
        None
    };
    Ok(metadata_only_statement(&query_type, query_text.as_deref()))
}

/// The run attribution of a SQLBuild query tag, or `None` for any other tag.
#[pyfunction]
pub(crate) fn parse_sqlbuild_query_tag<'py>(
    value: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyDict>>> {
    let py: Python<'py> = value.py();
    if !value.is_instance_of::<PyString>() {
        return Ok(None);
    }
    let Some(tag) = sqlbuild_query_tag(&value.extract::<String>()?) else {
        return Ok(None);
    };
    let attribution: Bound<'py, PyDict> = PyDict::new(py);
    attribution.set_item("run_id", tag.run_id)?;
    attribution.set_item("resource_type", tag.resource_type)?;
    attribution.set_item("resource_name", tag.resource_name)?;
    Ok(Some(attribution))
}
