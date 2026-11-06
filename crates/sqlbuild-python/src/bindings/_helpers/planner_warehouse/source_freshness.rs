//! Direct planner source freshness observation and the warnings for unknown freshness.

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::boundary::python_calls::{call_with, imported, within};
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::compiled_views::resource_reads;
use crate::bindings::_helpers::freshness::planning::{
    build_direct_source_freshness_planning_result, build_direct_source_freshness_propagation_result,
};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::planner_phases::objects::{native, replace, text, tuple};
use crate::bindings::_helpers::planner_scope::retained::scope_data;
use crate::bindings::_helpers::planner_warehouse::constants::RUN_SCOPE;
use crate::bindings::_helpers::planner_warehouse::snapshot::{destination_nodes, lower};
use crate::bindings::_helpers::records::construct::enum_member;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyDict as KeywordDict;
use pyo3::types::{PyBool, PyDict, PyDictMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::planner_scope::constants::MODEL;
use sqlbuild_analysis::planner_warehouse::main::freshness_unknown_message::freshness_unknown_message;
use std::collections::BTreeSet;

/// One warning per reason that left configured source freshness unknown.
pub(crate) fn source_freshness_unknown_warnings<'py>(
    source_freshness: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = source_freshness.py();
    let mut reasons: Vec<(String, Vec<String>)> = Vec::new();
    for unknown in native_field(source_freshness, "unknown_sources")?
        .call_method0("values")?
        .try_iter()?
    {
        let unknown = unknown?;
        let reason: String = native_field(&unknown, "reason")?.str()?.extract()?;
        if freshness_unknown_message(&reason, &[]).is_none() {
            continue;
        }
        let name: String = native_field(&unknown, "source_name")?.extract()?;
        match reasons.iter_mut().find(|(existing, _)| *existing == reason) {
            Some((_, names)) => names.push(name),
            None => reasons.push((reason, vec![name])),
        }
    }
    let mut warnings: Vec<_> = Vec::new();
    for (reason, names) in reasons {
        if let Some(message) = freshness_unknown_message(&reason, &names) {
            warnings.push(native(
                py,
                RecordId::PlanWarning,
                &[
                    ("model_name", py.None().into_bound(py)),
                    (
                        "severity",
                        enum_member(py, RecordId::WarningSeverity, "WARNING")?,
                    ),
                    ("message", text(py, &message)),
                ],
            )?);
        }
    }
    tuple(py, warnings)
}

fn state_database<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    for node in destination_nodes(project)? {
        let database = native_field(&resource_reads::destination(&node)?, "database")?;
        if !database.is_none() {
            return Ok(database);
        }
    }
    Ok(project.py().None().into_bound(project.py()))
}

fn state_schemas(
    project: &Bound<'_, PyAny>,
    scope: &Bound<'_, PyAny>,
) -> PyResult<BTreeSet<String>> {
    let mut schemas: BTreeSet<_> = BTreeSet::new();
    for node in destination_nodes(project)? {
        let schema = native_field(&resource_reads::destination(&node)?, "schema")?;
        if !schema.is_none() {
            schemas.insert(schema.extract()?);
        }
    }
    if schemas.is_empty() {
        let data = scope_data(scope)?;
        for (kind, name) in &data.selected {
            if kind != MODEL {
                continue;
            }
            let Some(model) = data.model(scope.py(), name) else {
                continue;
            };
            let schema = native_field(&model_reads::destination(&model)?, "schema")?;
            if !schema.is_none() {
                schemas.insert(schema.extract()?);
            }
        }
    }
    Ok(schemas)
}

#[pyfunction(name = "build_planner_source_freshness_result")]
#[pyo3(signature = (**options))]
pub(crate) fn build_planner_source_freshness_result_of<'py>(
    py: pyo3::Python<'py>,
    options: Option<Bound<'py, KeywordDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "build_planner_source_freshness_result",
        options,
        (
            &["project", "adapter", "connection", "scope", "relations"],
            &["freshness_state_schemas"],
        ),
    )?;
    build_planner_source_freshness_result(
        (&args.value("project")?, &args.value("scope")?),
        (&args.value("adapter")?, &args.value("connection")?),
        &args.value("relations")?,
        &args.value("freshness_state_schemas")?,
    )
}

/// The source freshness evaluation of one inspection scope against the warehouse.
pub(crate) fn build_planner_source_freshness_result<'py>(
    (project, scope): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (adapter, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    relations: &Bound<'py, PyAny>,
    freshness_state_schemas: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = project.py();
    let schemas = state_schemas(project, scope)?;
    let mut lowered_state: BTreeSet<String> = BTreeSet::new();
    if !freshness_state_schemas.is_none() {
        let existing = freshness_state_schemas;
        for schema in existing.try_iter()? {
            lowered_state.insert(schema?.extract()?);
        }
    }
    let exists = PyDict::new(py);
    for schema in &schemas {
        exists.set_item(
            schema,
            PyBool::new(py, lowered_state.contains(&lower(&text(py, schema))?)),
        )?;
    }
    let sources = tuple(
        py,
        native_field(relations, "source_read_map")?
            .call_method0("values")?
            .try_iter()?
            .collect::<PyResult<_>>()?,
    )?;
    let observed_at = imported(py, "datetime", "datetime")?
        .call_method1("now", (imported(py, "datetime", "UTC")?,))?;
    let result = within(
        &host::entry(py, RUN_SCOPE)?.call1((native_field(project, "run_id")?,))?,
        |_| {
            call_with(
                wrap_pyfunction!(build_direct_source_freshness_planning_result, py)?.as_any(),
                &[
                    ("adapter", adapter.clone()),
                    ("connection", connection.clone()),
                    ("sources", sources),
                    ("state_database", state_database(project)?),
                    (
                        "state_schemas",
                        tuple(py, schemas.iter().map(|schema| text(py, schema)).collect())?,
                    ),
                    ("observed_at", observed_at),
                    ("run_id", text(py, "planning")),
                    (
                        "render_qualified_name",
                        native_field(adapter, "render_qualified_name")?,
                    ),
                    ("state_table_exists_by_schema", exists.into_any()),
                    (
                        "filter_previous_to_sources",
                        PyBool::new(py, true).to_owned().into_any(),
                    ),
                ],
            )
        },
    )?;
    let propagation = call_with(
        wrap_pyfunction!(build_direct_source_freshness_propagation_result, py)?.as_any(),
        &[
            ("source_freshness", result.clone()),
            ("scope", scope.clone()),
        ],
    )?;
    replace(&result, &[("propagation", propagation)])
}
