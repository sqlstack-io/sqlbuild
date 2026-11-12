//! Record each successful incremental model's cursor `MAX` after a build, for plan previews.

use std::collections::{BTreeMap, HashSet};

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_analysis::planner_warehouse::main::cursor_query_groups::cursor_query_groups;
use sqlbuild_analysis::planner_warehouse::models::CursorTagRequest;
use sqlbuild_state::observations::constants::CURSOR_MAX_KIND;

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::compiled_views::{model_reads, project_reads};
use crate::bindings::_helpers::consumption::relation_reads::relation;
use crate::bindings::_helpers::native_host::consumption_state::{ConsumptionConnection, Location};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::native_host::observations_state::{
    ObservationAppend, append_observations,
};
use crate::bindings::_helpers::planner_phases::objects::config_str;
use crate::bindings::_helpers::planner_warehouse::constants::RUN_BOUNDED;
use crate::bindings::_helpers::planner_warehouse::cursors::{adapter_cursor_sql, bound_task};
use crate::bindings::_helpers::records::fields::field as native_field;

/// Most concurrent cursor reads after a build, whatever the adapter allows.
const MAX_OBSERVATION_READS: i64 = 4;

/// One successful incremental model's cursor read: its name, state location, cursor type and SQL.
struct CursorRead {
    model: String,
    location: Location,
    cursor_type: String,
    sql: String,
}

/// The cursor read of every successful incremental model, rendered as the planner renders it.
fn cursor_reads(
    project: &Bound<'_, PyAny>,
    adapter: &Bound<'_, PyAny>,
    successful_models: &[String],
) -> PyResult<Vec<CursorRead>> {
    let succeeded: HashSet<&str> = successful_models.iter().map(String::as_str).collect();
    let mut reads: Vec<CursorRead> = Vec::new();
    for model in project_reads::models(project)?.try_iter()? {
        let model: Bound<'_, PyAny> = model?;
        let name: String = model_reads::name_object(&model)?.extract()?;
        if !succeeded.contains(name.as_str())
            || config_str(&model, "materialized")?.as_deref() != Some("incremental")
        {
            continue;
        }
        let Some(cursor_column) = config_str(&model, "cursor")? else {
            continue;
        };
        let destination: Bound<'_, PyAny> = model_reads::destination(&model)?;
        let qualified: Bound<'_, PyAny> = native_field(&destination, "qualified_name")?;
        if qualified.is_none() || native_field(&destination, "schema")?.is_none() {
            continue;
        }
        let Some(query) = cursor_query_groups(&[CursorTagRequest {
            relation: qualified.extract()?,
            cursor_column,
            cursor_type: config_str(&model, "cursor_type")?,
            min_tag: None,
            max_tag: name.clone(),
        }])
        .into_iter()
        .next() else {
            continue;
        };
        reads.push(CursorRead {
            location: Location::of(&relation(&destination)?),
            sql: adapter_cursor_sql(adapter, &query)?,
            cursor_type: query.cursor_type,
            model: name,
        });
    }
    Ok(reads)
}

/// Read each cursor `MAX` and append it, by state location; returns one warning per failure.
fn observe<'py>(
    reads: &[CursorRead],
    (adapter, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    run_id: &str,
) -> PyResult<Vec<String>> {
    let py: Python<'py> = adapter.py();
    let execute: Bound<'py, PyAny> = native_field(adapter, "execute")?;
    let tasks: Vec<Bound<'py, PyAny>> = reads
        .iter()
        .map(|read| bound_task(py, read.sql.clone(), (connection, &execute)))
        .collect::<PyResult<_>>()?;
    let concurrency: i64 = native_field(adapter, "metadata_inspection_concurrency")?
        .extract::<i64>()?
        .clamp(1, MAX_OBSERVATION_READS);
    let outcomes: Vec<Bound<'py, PyAny>> = call_with(
        &host::entry(py, RUN_BOUNDED)?,
        &[
            ("tasks", pyo3::types::PyTuple::new(py, tasks)?.into_any()),
            ("concurrency", concurrency.into_pyobject(py)?.into_any()),
        ],
    )?
    .try_iter()?
    .collect::<PyResult<_>>()?;
    let mut warnings: Vec<String> = Vec::new();
    let mut rows: BTreeMap<Location, Vec<ObservationAppend>> = BTreeMap::new();
    for (read, outcome) in reads.iter().zip(outcomes) {
        let error: Bound<'py, PyAny> = outcome.get_item(2)?;
        if !error.is_none() {
            warnings.push(format!(
                "could not read the cursor of model {}: {}; plan previews show it as unknown",
                read.model,
                error.str()?
            ));
            continue;
        }
        let row: Bound<'py, PyAny> = outcome.get_item(0)?;
        if row.is_none() || row.get_item(0)?.is_none() {
            continue;
        }
        rows.entry(read.location.clone())
            .or_default()
            .push(ObservationAppend {
                kind: CURSOR_MAX_KIND,
                node: read.model.clone(),
                cursor_type: Some(read.cursor_type.clone()),
                value: row.get_item(0)?.str()?.extract()?,
                run_id: run_id.to_owned(),
            });
    }
    for (location, rows) in rows {
        if let Err(error) = append_observations(adapter, connection, &location, &rows) {
            warnings.push(format!(
                "could not record cursor observations in schema {}: {error}; plan previews show those cursors as unknown",
                location.schema
            ));
        }
    }
    Ok(warnings)
}

/// Record the cursor `MAX` of each successful incremental model; failures only warn.
pub(crate) fn append_cursor_observations(
    (project, successful_models): (&Bound<'_, PyAny>, &[String]),
    (adapter, connection_config): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    run_id: &str,
) -> Vec<String> {
    let recorded: PyResult<Vec<String>> = (|| {
        let reads: Vec<CursorRead> = cursor_reads(project, adapter, successful_models)?;
        if reads.is_empty() {
            return Ok(Vec::new());
        }
        let py: Python<'_> = adapter.py();
        let connection = ConsumptionConnection::new(adapter, connection_config);
        let observed: PyResult<Vec<String>> = connection
            .connected(py)
            .and_then(|handle| observe(&reads, (adapter, &handle), run_id));
        let closed: PyResult<()> = connection.close(py);
        let warnings: Vec<String> = observed?;
        closed?;
        Ok(warnings)
    })();
    recorded.unwrap_or_else(|error| {
        vec![format!(
            "could not record cursor observations: {error}; plan previews show cursors as unknown"
        )]
    })
}
