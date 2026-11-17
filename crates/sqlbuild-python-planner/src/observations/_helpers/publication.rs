//! Record what a build observed, cursor `MAX` values and described source columns, for plan previews.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_analysis::planner_warehouse::main::cursor_query_groups::cursor_query_groups;
use sqlbuild_analysis::planner_warehouse::models::CursorTagRequest;
use sqlbuild_state::observations::constants::{CURSOR_MAX_KIND, SOURCE_COLUMNS_KIND};

use crate::consumption::main::relation::relation;
use crate::live_reads::main::bound_task::bound_task;
use crate::planner_warehouse::constants::RUN_BOUNDED;
use crate::planner_warehouse::main::inspection::dialect_cursor_sql::dialect_cursor_sql;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::native_host::main::append_observations::append_observations;
use sqlbuild_python_front::native_host::models::Location;
use sqlbuild_python_front::native_host::models::ObservationAppend;
use sqlbuild_python_project::compiled_views::main::compiled_models::dep_rows::dep_rows;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;
use sqlbuild_python_project::compiled_views::main::project::compiled_models::compiled_models;
use sqlbuild_python_project::planner_phases::main::config_str::config_str;

/// Dependency kind of a source in a model's `dep_rows`.
const SOURCE_DEPENDENCY: &str = "source";
/// Most concurrent cursor reads after a build, whatever the adapter allows.
const MAX_OBSERVATION_READS: i64 = 4;

/// One successful incremental model's cursor read: its name, state location, cursor type and SQL.
pub(crate) struct CursorRead {
    model: String,
    location: Location,
    cursor_type: String,
    sql: String,
}

/// The cursor read of every successful incremental model, rendered as the planner renders it.
pub(crate) fn cursor_reads(
    project: &Bound<'_, PyAny>,
    adapter: &Bound<'_, PyAny>,
    successful_models: &[String],
) -> PyResult<Vec<CursorRead>> {
    let succeeded: HashSet<&str> = successful_models.iter().map(String::as_str).collect();
    let mut reads: Vec<CursorRead> = Vec::new();
    for model in compiled_models(project)?.try_iter()? {
        let model: Bound<'_, PyAny> = model?;
        let name: String = name_object(&model)?.extract()?;
        if !succeeded.contains(name.as_str())
            || config_str(&model, "materialized")?.as_deref() != Some("incremental")
        {
            continue;
        }
        let Some(cursor_column) = config_str(&model, "cursor")? else {
            continue;
        };
        let destination: Bound<'_, PyAny> = sqlbuild_python_project::compiled_views::main::compiled_models::destination::destination(&model)?;
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
            sql: dialect_cursor_sql(&Dialect::of(adapter)?, &query)?,
            cursor_type: query.cursor_type,
            model: name,
        });
    }
    Ok(reads)
}

/// One `source_columns` row per described source, in the state schema of each model reading it.
pub(crate) fn source_column_rows(
    project: &Bound<'_, PyAny>,
    described: &BTreeMap<String, String>,
    run_id: &str,
) -> PyResult<BTreeMap<Location, Vec<ObservationAppend>>> {
    let mut readers: BTreeMap<Location, BTreeSet<String>> = BTreeMap::new();
    if described.is_empty() {
        return Ok(BTreeMap::new());
    }
    for model in compiled_models(project)?.try_iter()? {
        let model: Bound<'_, PyAny> = model?;
        let destination: Bound<'_, PyAny> = sqlbuild_python_project::compiled_views::main::compiled_models::destination::destination(&model)?;
        if native_field(&destination, "schema")?.is_none() {
            continue;
        }
        let sources: Vec<String> = dep_rows(&model)?
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
            let row: ObservationAppend = ObservationAppend {
                kind: SOURCE_COLUMNS_KIND,
                value: described[&source].clone(),
                node: source,
                cursor_type: None,
                run_id: run_id.to_owned(),
            };
            rows.entry(location.clone()).or_default().push(row);
        }
    }
    Ok(rows)
}

/// Read each cursor `MAX`, then append it with `rows`, by state location; one warning per failure.
pub(crate) fn observe<'py>(
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
        &host_entry(py, RUN_BOUNDED)?,
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
