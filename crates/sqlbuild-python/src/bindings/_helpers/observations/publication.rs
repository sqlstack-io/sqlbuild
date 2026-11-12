//! Record what a build observed, cursor `MAX` values and described source columns, for plan previews.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use pyo3::IntoPyObject;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_analysis::planner_warehouse::main::cursor_query_groups::cursor_query_groups;
use sqlbuild_analysis::planner_warehouse::models::CursorTagRequest;
use sqlbuild_state::observations::constants::{CURSOR_MAX_KIND, SOURCE_COLUMNS_KIND};

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::compiled_views::{model_reads, project_reads};
use crate::bindings::_helpers::consumption::relation_reads::relation;
use crate::bindings::_helpers::native_host::consumption_state::{ConsumptionConnection, Location};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::native_host::observations_state::{
    ObservationAppend, append_observations,
};
use crate::bindings::_helpers::observations::source_capture::take_source_capture;
use crate::bindings::_helpers::planner_phases::objects::config_str;
use crate::bindings::_helpers::planner_warehouse::constants::RUN_BOUNDED;
use crate::bindings::_helpers::planner_warehouse::cursors::{adapter_cursor_sql, bound_task};
use crate::bindings::_helpers::records::fields::field as native_field;

/// Dependency kind of a source in a model's `dep_rows`.
const SOURCE_DEPENDENCY: &str = "source";
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

/// One `source_columns` row per described source, in the state schema of each model reading it.
fn source_column_rows(
    project: &Bound<'_, PyAny>,
    described: &BTreeMap<String, Vec<String>>,
    run_id: &str,
) -> PyResult<BTreeMap<Location, Vec<ObservationAppend>>> {
    let mut readers: BTreeMap<Location, BTreeSet<String>> = BTreeMap::new();
    if described.is_empty() {
        return Ok(BTreeMap::new());
    }
    for model in project_reads::models(project)?.try_iter()? {
        let model: Bound<'_, PyAny> = model?;
        let destination: Bound<'_, PyAny> = model_reads::destination(&model)?;
        if native_field(&destination, "schema")?.is_none() {
            continue;
        }
        let sources: Vec<String> = model_reads::dep_rows(&model)?
            .into_iter()
            .filter(|(kind, name)| kind == SOURCE_DEPENDENCY && described.contains_key(name))
            .map(|(_, name)| name)
            .collect();
        if !sources.is_empty() {
            readers
                .entry(Location::of(&relation(&destination)?))
                .or_default()
                .extend(sources);
        }
    }
    let mut rows: BTreeMap<Location, Vec<ObservationAppend>> = BTreeMap::new();
    for (location, sources) in readers {
        for source in sources {
            let row: ObservationAppend = source_columns_row(source, described, run_id)?;
            rows.entry(location.clone()).or_default().push(row);
        }
    }
    Ok(rows)
}

/// The `source_columns` observation of one described source, its column names as a JSON array.
fn source_columns_row(
    source: String,
    described: &BTreeMap<String, Vec<String>>,
    run_id: &str,
) -> PyResult<ObservationAppend> {
    let value: String = serde_json::to_string(&described[&source])
        .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
    Ok(ObservationAppend {
        kind: SOURCE_COLUMNS_KIND,
        node: source,
        cursor_type: None,
        value,
        run_id: run_id.to_owned(),
    })
}

/// Read each cursor `MAX`, then append it with `rows`, by state location; one warning per failure.
fn observe<'py>(
    reads: &[CursorRead],
    mut rows: BTreeMap<Location, Vec<ObservationAppend>>,
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
                "could not record observations in schema {}: {error}; plan previews show them as unknown",
                location.schema
            ));
        }
    }
    Ok(warnings)
}

/// Record successful incremental models' cursor `MAX` and the source columns planning described; failures only warn.
pub(crate) fn append_build_observations(
    (project, successful_models): (&Bound<'_, PyAny>, &[String]),
    (adapter, connection_config): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    run_id: &str,
) -> Vec<String> {
    let described: BTreeMap<String, Vec<String>> = take_source_capture().into_iter().collect();
    let recorded: PyResult<Vec<String>> = (|| {
        let reads: Vec<CursorRead> = cursor_reads(project, adapter, successful_models)?;
        let sources: BTreeMap<Location, Vec<ObservationAppend>> =
            source_column_rows(project, &described, run_id)?;
        if reads.is_empty() && sources.is_empty() {
            return Ok(Vec::new());
        }
        let py: Python<'_> = adapter.py();
        let connection = ConsumptionConnection::new(adapter, connection_config);
        let observed: PyResult<Vec<String>> = connection
            .connected(py)
            .and_then(|handle| observe(&reads, sources, (adapter, &handle), run_id));
        let closed: PyResult<()> = connection.close(py);
        let warnings: Vec<String> = observed?;
        closed?;
        Ok(warnings)
    })();
    recorded.unwrap_or_else(|error| {
        vec![format!(
            "could not record observations: {error}; plan previews show them as unknown"
        )]
    })
}
