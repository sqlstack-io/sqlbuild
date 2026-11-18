//! Existing seed and function plan dataclass construction.

use crate::planner_identity::main::build_compiled_function_fingerprint_sql::build_compiled_function_fingerprint_sql;
use crate::planner_resolution::main::references::resolve_function_sql::resolve_function_sql;
use crate::planner_scenario::_helpers::sql::required_target;
use crate::planner_scenario::_helpers::values::error;
use crate::planner_scenario::constants::SCENARIO_PLAN_UNKNOWN_SEED;
use crate::planner_scenario::models::SqlContext;
use crate::planner_strategy::main::model_entries::extract_seed_columns::extract_seed_columns;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyString;
use pyo3::types::{PyDictMethods, PyTuple};
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::native_host::types::Syntax;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::by_attribute::by_attribute;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::resource_views::main::seed_reads::key::key;
use sqlbuild_python_project::resource_views::main::seed_reads::schema_entry::schema_entry;
use sqlbuild_python_project::resource_views::main::seed_reads::seed_file::seed_file;

pub(crate) fn seed_entries<'py>(
    project: &Bound<'py, PyAny>,
    graph: &Bound<'py, PyAny>,
    relations: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py = project.py();
    let seeds = by_attribute(
        &sqlbuild_python_project::compiled_views::main::project::seeds::seeds(project)?,
        "name",
    )?;
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
                ("key", key(&seed)?),
                (
                    "name",
                    sqlbuild_python_project::resource_views::main::seed_reads::name::name(&seed)?,
                ),
                ("destination", destination),
                ("file_path", native_field(&seed_file(&seed)?, "file_path")?),
                ("columns", extract_seed_columns(&seed)?.into_any()),
                (
                    "csv_settings",
                    native_field(&schema_entry(&seed)?, "csv_settings")?,
                ),
            ],
        )?);
    }
    PyTuple::new(py, result)
}

pub(crate) fn function_entry<'py>(
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
