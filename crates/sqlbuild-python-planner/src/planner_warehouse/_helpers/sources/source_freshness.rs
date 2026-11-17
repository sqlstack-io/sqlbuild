//! Direct planner source freshness observation and the warnings for unknown freshness.

use crate::freshness::main::build_direct_source_freshness_planning_result_function::build_direct_source_freshness_planning_result_function;
use crate::freshness::main::build_direct_source_freshness_propagation_result_function::build_direct_source_freshness_propagation_result_function;
use crate::planner_warehouse::_helpers::inspection::snapshot::destination_nodes;
use crate::planner_warehouse::_helpers::inspection::snapshot::lower;
use crate::planner_warehouse::constants::RUN_SCOPE;
use crate::planner_warehouse::models::PlannerReads;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyDict as KeywordDict;
use pyo3::types::{PyBool, PyDict, PyDictMethods};
use sqlbuild_analysis::planner_scope::constants::MODEL;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_base::native_host::types::Syntax;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::planner_phases::main::replace::replace;
use sqlbuild_python_project::planner_phases::main::text::text;
use sqlbuild_python_project::planner_phases::main::tuple::tuple;
use sqlbuild_python_project::planner_scope::main::scope_data::scope_data;
use std::collections::BTreeSet;

fn state_database<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    for node in destination_nodes(project)? {
        let database = native_field(&sqlbuild_python_project::resource_views::main::resource_reads::destination::destination(&node)?, "database")?;
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
        let schema = native_field(&sqlbuild_python_project::resource_views::main::resource_reads::destination::destination(&node)?, "schema")?;
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
            let schema = native_field(&sqlbuild_python_project::compiled_views::main::compiled_models::destination::destination(&model)?, "schema")?;
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
            &["project", "dialect", "reads", "scope", "relations"],
            &["freshness_state_schemas"],
        ),
    )?;
    build_planner_source_freshness_result(
        (&args.value("project")?, &args.value("scope")?),
        (
            &Dialect::passed(args.value("dialect")?)?,
            &PlannerReads::passed(args.value("reads")?)?,
        ),
        &args.value("relations")?,
        &args.value("freshness_state_schemas")?,
    )
}

/// The source freshness evaluation of one inspection scope against the warehouse.
pub(crate) fn build_planner_source_freshness_result<'py>(
    (project, scope): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (dialect, reads): (&Bound<'py, Dialect>, &Bound<'py, PlannerReads>),
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
        &host_entry(py, RUN_SCOPE)?.call1((native_field(project, "run_id")?,))?,
        |_| {
            call_with(
                build_direct_source_freshness_planning_result_function(py)?.as_any(),
                &[
                    ("dialect", dialect.clone().into_any()),
                    ("reads", reads.clone().into_any()),
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
                        dialect.syntax("render_qualified_name")?,
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
        build_direct_source_freshness_propagation_result_function(py)?.as_any(),
        &[
            ("source_freshness", result.clone()),
            ("scope", scope.clone()),
        ],
    )?;
    replace(&result, &[("propagation", propagation)])
}
