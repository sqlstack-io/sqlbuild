//! Backend-neutral execution history entry points and model validation.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python, pyfunction,
    wrap_pyfunction,
};
use pyo3::types::PyTuple;
use sqlbuild_state::execution_history::main::project_runs::project_runs;
use sqlbuild_state::execution_history::main::validate_filter::validate_filter;
use sqlbuild_state::execution_history::main::validate_page::validate_page;
use sqlbuild_state::execution_history::main::validate_page_limit::validate_page_limit;
use sqlbuild_state::execution_history::main::validate_stored_event::validate_stored_event;
use sqlbuild_state::execution_history::models::FilterSpec;

use crate::bindings::_helpers::execution_history::appends::{event_content, event_id};
use crate::bindings::_helpers::execution_history::conversion::{
    checked, event_filter_spec, exact_int, projection_facts, run_filter_spec, run_records,
    run_rows, text_of, utc_zone,
};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Return the required stable ID from a known or opaque lifecycle event.
#[pyfunction]
fn canonical_event_id(event: &Bound<'_, PyAny>) -> PyResult<String> {
    event_id(event)
}

/// Return deterministic content for lifecycle event idempotency comparison.
#[pyfunction]
fn canonical_event_content(event: &Bound<'_, PyAny>) -> PyResult<String> {
    event_content(event)
}

/// Apply durable event facts in storage order to immutable run projections.
#[pyfunction]
#[pyo3(signature = (*, stored_events, current_runs = None))]
fn project_execution_runs<'py>(
    py: Python<'py>,
    stored_events: &Bound<'py, PyAny>,
    current_runs: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let facts = projection_facts(stored_events)?;
    let current = match current_runs {
        Some(current_runs) => run_rows(current_runs)?,
        None => Vec::new(),
    };
    run_records(py, &checked(py, project_runs(facts, current))?)
}

/// Validate a positive page limit no greater than the public maximum.
#[pyfunction]
fn validate_history_page_limit(limit: &Bound<'_, PyAny>) -> Result<(), PyErr> {
    checked(limit.py(), validate_page_limit(exact_int(limit)))
}

/// Append facts durably before publishing their rebuildable run projection.
#[pyfunction]
#[pyo3(signature = (*, event_log, run_storage, events))]
fn append_and_project<'py>(
    event_log: &Bound<'py, PyAny>,
    run_storage: &Bound<'py, PyAny>,
    events: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let stored: Bound<'py, PyAny> = event_log.call_method1("append_events", (events,))?;
    run_storage.call_method1("project", (&stored,))?;
    Ok(stored)
}

/// Validate a `StoredEvent`'s position, cursor and UTC receipt time.
#[pyfunction]
fn validate_stored_history_event(record: &Bound<'_, PyAny>) -> Result<(), PyErr> {
    let py: Python<'_> = record.py();
    let storage_order: i64 = native_field(record, "storage_order")?.extract()?;
    let cursor: String = text_of(&native_field(record, "cursor")?)?.unwrap_or_default();
    let received_at_is_utc: bool = utc_zone(&native_field(record, "received_at")?)?;
    checked(
        py,
        validate_stored_event(storage_order, &cursor, received_at_is_utc),
    )
}

/// Validate an `EventFilter`.
#[pyfunction]
fn validate_history_event_filter(event_filter: &Bound<'_, PyAny>) -> Result<(), PyErr> {
    checked(
        event_filter.py(),
        validate_filter(FilterSpec::Event(&event_filter_spec(event_filter)?)),
    )
}

/// Validate a `RunFilter`.
#[pyfunction]
fn validate_history_run_filter(run_filter: &Bound<'_, PyAny>) -> Result<(), PyErr> {
    checked(
        run_filter.py(),
        validate_filter(FilterSpec::Run(&run_filter_spec(run_filter)?)),
    )
}

/// Validate an `EventPage` or `RunPage` next cursor against its records.
#[pyfunction]
#[pyo3(signature = (*, page, is_event_page))]
fn validate_history_page(page: &Bound<'_, PyAny>, is_event_page: bool) -> Result<(), PyErr> {
    let py: Python<'_> = page.py();
    let records: Bound<'_, PyAny> = native_field(page, "records")?;
    let has_records: bool = records.is_truthy()?;
    let next_cursor: Bound<'_, PyAny> = native_field(page, "next_cursor")?;
    let next_text: Option<String> = text_of(&next_cursor)?;
    let last_text: Option<String> = if is_event_page && has_records {
        text_of(&native_field(&records.get_item(-1)?, "cursor")?)?
    } else {
        None
    };
    let next: Option<&str> = if next_cursor.is_none() {
        None
    } else {
        Some(next_text.as_deref().unwrap_or_default())
    };
    checked(
        py,
        validate_page(is_event_page, has_records, (next, last_text.as_deref())),
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(canonical_event_id, module)?)?;
    module.add_function(wrap_pyfunction!(canonical_event_content, module)?)?;
    module.add_function(wrap_pyfunction!(project_execution_runs, module)?)?;
    module.add_function(wrap_pyfunction!(validate_history_page_limit, module)?)?;
    module.add_function(wrap_pyfunction!(append_and_project, module)?)?;
    module.add_function(wrap_pyfunction!(validate_stored_history_event, module)?)?;
    module.add_function(wrap_pyfunction!(validate_history_event_filter, module)?)?;
    module.add_function(wrap_pyfunction!(validate_history_run_filter, module)?)?;
    module.add_function(wrap_pyfunction!(validate_history_page, module)?)
}
