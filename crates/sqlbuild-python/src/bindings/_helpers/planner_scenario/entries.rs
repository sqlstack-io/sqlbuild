//! Existing seed and function plan dataclass construction.

use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::compiled_views::seed_reads;
use crate::bindings::_helpers::native_host::dialect::Syntax;
use crate::bindings::_helpers::planner_identity::metadata::build_compiled_function_fingerprint_sql;
use crate::bindings::_helpers::planner_resolution::orchestration::resolve_function_sql;
use crate::bindings::_helpers::planner_scenario::constants::SCENARIO_PLAN_UNKNOWN_SEED;
use crate::bindings::_helpers::planner_scenario::{
    models::SqlContext, sql::required_target, values::error,
};
use crate::bindings::_helpers::planner_strategy::model_entries::extract_seed_columns;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::by_attribute;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyString;
use pyo3::types::{PyDictMethods, PyTuple};

pub(super) fn seed_entries<'py>(
    project: &Bound<'py, PyAny>,
    graph: &Bound<'py, PyAny>,
    relations: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py = project.py();
    let seeds = by_attribute(&project_reads::seeds(project)?, "name")?;
    let fixtures = native_field(graph, "seed_fixture_names")?;
    let mut result: Vec<Bound<'py, PyAny>> = Vec::new();
    for name in native_field(graph, "seed_names")?.try_iter()? {
        let name = name?;
        if fixtures.contains(&name)? {
            continue;
        }
        let Some(seed) = seeds.get_item(&name)? else {
            let scenario: String = native_field(graph, "name")?.extract()?;
            let name: String = name.extract()?;
            return Err(error(
                py,
                &format!("Scenario '{scenario}' requires unknown seed '{name}'"),
                SCENARIO_PLAN_UNKNOWN_SEED,
                None,
            )?);
        };
        let destination = required_target(
            relations,
            "seed_locations",
            &name.extract::<String>()?,
            "seed",
        )?;
        result.push(new_record_with(
            py,
            RecordId::SeedPlanEntry,
            &[
                ("key", seed_reads::key(&seed)?),
                ("name", seed_reads::name(&seed)?),
                ("destination", destination),
                (
                    "file_path",
                    native_field(&seed_reads::seed_file(&seed)?, "file_path")?,
                ),
                ("columns", extract_seed_columns(&seed)?.into_any()),
                (
                    "csv_settings",
                    native_field(&seed_reads::schema_entry(&seed)?, "csv_settings")?,
                ),
            ],
        )?);
    }
    PyTuple::new(py, result)
}

pub(super) fn function_entry<'py>(
    function: &Bound<'py, PyAny>,
    context: &SqlContext<'py>,
    locations: &Bound<'py, PyAny>,
    warehouse_columns: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = function.py();
    let body = resolve_function_sql(
        py,
        Some(keywords(
            py,
            &[
                ("dialect", context.dialect.clone().into_any()),
                ("function", function.clone()),
                (
                    "model_locations",
                    native_field(&context.relations, "model_locations")?,
                ),
                (
                    "seed_locations",
                    native_field(&context.relations, "seed_locations")?,
                ),
                ("function_locations", locations.clone()),
                (
                    "source_map",
                    native_field(&context.relations, "source_map")?,
                ),
                ("source_warehouse_columns", warehouse_columns.clone()),
                (
                    "star_exclude_keyword",
                    context.dialect.syntax("star_exclude_keyword")?.call0()?,
                ),
            ],
        )?),
    )?;
    let mut fields: Vec<(&str, Bound<'py, PyAny>)> = Vec::new();
    for name in [
        "key",
        "name",
        "relative_path",
        "destination",
        "arguments",
        "returns",
    ] {
        fields.push((name, native_field(function, name)?));
    }
    fields.push(("body_sql", body));
    fields.push((
        "fingerprint_query_sql",
        PyString::new(py, &build_compiled_function_fingerprint_sql(function)?).into_any(),
    ));
    for name in [
        "fingerprint_destination",
        "return_columns",
        "language",
        "source_file_path",
        "runtime_version",
        "entry_point",
        "packages",
    ] {
        fields.push((name, native_field(function, name)?));
    }
    new_record_with(py, RecordId::FunctionPlanEntry, &fields)
}
