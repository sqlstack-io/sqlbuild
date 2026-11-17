//! Backend-neutral execution history entry points and model validation.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python, pyfunction};
use pyo3::types::PyTuple;
use sqlbuild_python_base::boundary::main::optional_text::optional_text;
use sqlbuild_state::execution_history::main::project_runs::project_runs;
use sqlbuild_state::execution_history::main::validate_filter::validate_filter;
use sqlbuild_state::execution_history::main::validate_page::validate_page;
use sqlbuild_state::execution_history::main::validate_page_limit::validate_page_limit;
use sqlbuild_state::execution_history::main::validate_stored_event::validate_stored_event;
use sqlbuild_state::execution_history::models::FilterSpec;

use crate::execution_history::_helpers::appends::event_content;
use crate::execution_history::_helpers::appends::event_id;
use crate::execution_history::_helpers::conversion::checked;
use crate::execution_history::_helpers::conversion::event_filter_spec;
use crate::execution_history::_helpers::conversion::exact_int;
use crate::execution_history::_helpers::conversion::projection_facts;
use crate::execution_history::_helpers::conversion::run_filter_spec;
use crate::execution_history::_helpers::conversion::run_records;
use crate::execution_history::_helpers::conversion::run_rows;
use crate::execution_history::_helpers::conversion::utc_zone;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Return the required stable ID from a known or opaque lifecycle event.
#[pyfunction]
pub(crate) fn canonical_event_id(event: &Bound<'_, PyAny>) -> PyResult<String> {
    event_id(event)
}

/// Return deterministic content for lifecycle event idempotency comparison.
#[pyfunction]
pub(crate) fn canonical_event_content(event: &Bound<'_, PyAny>) -> PyResult<String> {
    event_content(event)
}

/// Apply durable event facts in storage order to immutable run projections.
#[pyfunction]
#[pyo3(signature = (*, stored_events, current_runs = None))]
pub(crate) fn project_execution_runs<'py>(
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
pub(crate) fn validate_history_page_limit(limit: &Bound<'_, PyAny>) -> Result<(), PyErr> {
    checked(limit.py(), validate_page_limit(exact_int(limit)))
}

/// Append facts durably before publishing their rebuildable run projection.
#[pyfunction]
#[pyo3(signature = (*, event_log, run_storage, events))]
pub(crate) fn append_and_project<'py>(
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
pub(crate) fn validate_stored_history_event(record: &Bound<'_, PyAny>) -> Result<(), PyErr> {
    let py: Python<'_> = record.py();
    let storage_order: i64 = native_field(record, "storage_order")?.extract()?;
    let cursor: String = optional_text(&native_field(record, "cursor")?)?.unwrap_or_default();
    let received_at_is_utc: bool = utc_zone(&native_field(record, "received_at")?)?;
    checked(
        py,
        validate_stored_event(storage_order, &cursor, received_at_is_utc),
    )
}

/// Validate an `EventFilter`.
#[pyfunction]
pub(crate) fn validate_history_event_filter(event_filter: &Bound<'_, PyAny>) -> Result<(), PyErr> {
    checked(
        event_filter.py(),
        validate_filter(FilterSpec::Event(&event_filter_spec(event_filter)?)),
    )
}

/// Validate a `RunFilter`.
#[pyfunction]
pub(crate) fn validate_history_run_filter(run_filter: &Bound<'_, PyAny>) -> Result<(), PyErr> {
    checked(
        run_filter.py(),
        validate_filter(FilterSpec::Run(&run_filter_spec(run_filter)?)),
    )
}

/// Validate an `EventPage` or `RunPage` next cursor against its records.
#[pyfunction]
#[pyo3(signature = (*, page, is_event_page))]
pub(crate) fn validate_history_page(
    page: &Bound<'_, PyAny>,
    is_event_page: bool,
) -> Result<(), PyErr> {
    let py: Python<'_> = page.py();
    let records: Bound<'_, PyAny> = native_field(page, "records")?;
    let has_records: bool = records.is_truthy()?;
    let next_cursor: Bound<'_, PyAny> = native_field(page, "next_cursor")?;
    let next_text: Option<String> = optional_text(&next_cursor)?;
    let last_text: Option<String> = if is_event_page && has_records {
        optional_text(&native_field(&records.get_item(-1)?, "cursor")?)?
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
