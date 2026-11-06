//! Assemble scenario execution plans through existing planner and adapter APIs.

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::compiled_views::records::compiled_model_record;
use crate::bindings::_helpers::graph::keys::KeyCache;
use crate::bindings::_helpers::model_config::typed_config::{config_value, model_values};
use crate::bindings::_helpers::planner_entries::model_entries::plan_model;
use crate::bindings::_helpers::planner_entries::models::ModelPlanContext;
use crate::bindings::_helpers::planner_identity::graph::{execution_deps, execution_order};
use crate::bindings::_helpers::planner_phases::objects::object;
use crate::bindings::_helpers::planner_resolution::locations::build_function_locations;
use crate::bindings::_helpers::planner_scenario::constants::{
    CURSOR_INTRINSIC_NAMES, CURSOR_TYPE_KEY, INCREMENTAL_MODE_KEY, INTEGER_CURSOR_TYPE,
    MICROBATCH_INCREMENTAL_MODE, SCENARIO_EXPECTED_COLUMNS_HELP, SCENARIO_PLAN_EXPECTED_COLUMNS,
    SCENARIO_PLAN_INVALID_FIXTURE,
};
use crate::bindings::_helpers::planner_scenario::helper_scope::scenario_helper_scope;
use crate::bindings::_helpers::planner_scenario::{
    entries::{function_entry, seed_entries},
    fixtures::{extract_fixtures, fixture_plans},
    models::SqlContext,
    sql::{required_target, resolve_model_dbt_refs, resolve_sql, source_label},
    values::{error, string},
};
use crate::bindings::_helpers::planner_strategy::fixture_completion::complete_fixtures as complete_relation_fixtures;
use crate::bindings::_helpers::planner_strategy::fixture_context::planning_context;
use crate::bindings::_helpers::records::construct::{enum_member, new_record, new_record_with};
use crate::bindings::_helpers::records::fields::by_attribute;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::fields::replaced_keywords;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::sql_test_glue::constants::{
    DEFAULT_INTEGER_END, DEFAULT_INTEGER_START, DEFAULT_TIMESTAMP_END, DEFAULT_TIMESTAMP_START,
};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python, pyfunction};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyFrozenSet, PyTuple};
use sqlbuild_analysis::compiler::main::scenario_expected_column_message::scenario_expected_column_message;
use sqlbuild_analysis::compiler::main::scenario_expected_columns::scenario_expected_columns;
use sqlbuild_analysis::compiler::main::scenario_reader_sql::scenario_reader_sql;
use sqlbuild_discovery::constants::{SCENARIO_EXPECTED_COLUMNS_ALL, SCENARIO_EXPECTED_COLUMNS_KEY};

#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(crate) fn build_scenario_execution_plan<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "build_scenario_execution_plan",
        kwargs,
        (
            &[
                "scenario",
                "project",
                "adapter",
                "graph_plan",
                "relation_plan",
                "source_lexical_syntax",
            ],
            &[
                "snapshot",
                "source_warehouse_columns",
                "sql_analysis_enabled",
            ],
        ),
    )?;
    let scenario = args.value("scenario")?;
    let project = args.value("project")?;
    let graph = args.value("graph_plan")?;
    let context = SqlContext {
        adapter: args.value("adapter")?,
        relations: args.value("relation_plan")?,
        syntax: args.value("source_lexical_syntax")?,
        label: source_label(&scenario)?,
    };
    let snapshot = args.value("snapshot")?;
    let snapshot = if snapshot.is_truthy()? {
        snapshot
    } else {
        new_record(py, RecordId::WarehouseSnapshot, (), None)?
    };
    let columns = args.value("source_warehouse_columns")?;
    let columns = if columns.is_truthy()? {
        columns
    } else {
        PyDict::new(py).into_any()
    };
    let analysis = args.value("sql_analysis_enabled")?;
    let analysis = if analysis.is_none() {
        true
    } else {
        analysis.is_truthy()?
    };
    let models = by_attribute(&project_reads::models(&project)?, "name")?;
    let functions = by_attribute(&project_reads::functions(&project)?, "key")?;
    let locations = build_function_locations(&project_reads::functions(&project)?)?.into_any();
    let upstream = execution_deps(&project)?;
    let ordered = execution_order(py, &upstream, &[])?;
    let ordered = KeyCache::new(py)?.keys(&ordered)?;
    let mut model_keys: Vec<Bound<'py, PyAny>> = Vec::new();
    for key in &ordered {
        if native_field(key, "resource_type")?.eq("model")?
            && native_field(&graph, "model_names")?.contains(native_field(key, "name")?)?
        {
            model_keys.push(key.clone());
        }
    }
    let overrides = if analysis {
        complete_fixtures(&scenario, &project, (&context, &model_keys))?
    } else {
        py.None().into_bound(py)
    };
    let fixtures = fixture_plans(&scenario, &graph, &context, &overrides)?;
    let seeds = seed_entries(&project, &graph, &context.relations)?;
    let mut function_entries: Vec<Bound<'py, PyAny>> = Vec::new();
    for key in &ordered {
        if native_field(&graph, "function_deps")?.contains(key)?
            && let Some(function) = functions.get_item(key)?
        {
            function_entries.push(function_entry(&function, &context, &locations, &columns)?);
        }
    }
    let mut entries: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut warnings: Vec<Bound<'py, PyAny>> = Vec::new();
    for key in model_keys {
        let model = models
            .get_item(native_field(&key, "name")?)?
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("scenario model"))?;
        let destination = required_target(
            &context.relations,
            "model_locations",
            &model_reads::model_name(&model)?,
            "model",
        )?;
        let query = resolve_model_dbt_refs(
            &model_reads::query_sql(&model)?.extract::<String>()?,
            &context.relations,
        )?;
        let cursor_window = scenario_cursor_window(&model)?;
        let record = compiled_model_record(&model)?;
        let changes = PyDict::new(py);
        changes.set_item("destination", destination)?;
        changes.set_item("query_sql", query)?;
        let model = replaced_keywords(&record, &changes)?;
        let model_context = model_context(&project, &context, (&models, &locations, &columns))?;
        let result = object(
            py,
            plan_model(
                py,
                Some(keywords(
                    py,
                    &[
                        ("model", model),
                        ("snapshot", snapshot.clone()),
                        ("adapter", context.adapter.clone()),
                        ("context", model_context),
                        (
                            "sql_analysis_enabled",
                            PyBool::new(py, analysis).to_owned().into_any(),
                        ),
                        (
                            "query_change_tracking",
                            PyBool::new(py, false).to_owned().into_any(),
                        ),
                        ("full_refresh", PyBool::new(py, true).to_owned().into_any()),
                        ("cursor_overrides", cursor_window),
                    ],
                )?),
            )?,
        )?;
        entries.push(result.get_item(0)?);
        for warning in result.get_item(1)?.try_iter()? {
            warnings.push(warning?);
        }
    }
    let (expected, assertions) = expectations(&scenario, &context, &models)?;
    let plan = new_record_with(
        py,
        RecordId::ScenarioExecutionPlan,
        &[
            ("key", native_field(&scenario, "key")?),
            ("name", native_field(&scenario, "name")?),
            ("graph_plan", graph),
            ("relation_plan", context.relations),
            ("fixture_plans", fixtures.into_any()),
            ("seed_entries", seeds.into_any()),
            (
                "function_entries",
                PyTuple::new(py, function_entries)?.into_any(),
            ),
            ("model_entries", PyTuple::new(py, entries)?.into_any()),
            ("hook_functions", native_field(&project, "hook_functions")?),
            ("expected_expectations", expected),
            ("assertion_expectations", assertions),
        ],
    )?;
    Ok(PyTuple::new(py, [plan, PyTuple::new(py, warnings)?.into_any()])?.into_any())
}

fn complete_fixtures<'py>(
    scenario: &Bound<'py, PyAny>,
    project: &Bound<'py, PyAny>,
    inputs: (&SqlContext<'py>, &[Bound<'py, PyAny>]),
) -> PyResult<Bound<'py, PyAny>> {
    let (context, keys) = inputs;
    let py = scenario.py();
    let mut groups: Vec<Bound<'py, PyTuple>> = Vec::new();
    for (kind, prefix) in [
        ("SOURCE", "__source__"),
        ("MODEL", "__ref__"),
        ("SEED", "__seed__"),
    ] {
        let kind = enum_member(py, RecordId::CompiledResourceType, kind)?;
        groups.push(PyTuple::new(
            py,
            [kind, extract_fixtures(scenario, prefix)?.into_any()],
        )?);
    }
    let names = keys
        .iter()
        .map(|key| native_field(key, "name"))
        .collect::<PyResult<Vec<_>>>()?;
    let names = names
        .iter()
        .map(|name| name.extract())
        .collect::<PyResult<Vec<String>>>()?;
    let planning = planning_context(project, None)?;
    let completion = complete_relation_fixtures(
        &planning,
        &context.adapter,
        (&names, PyTuple::new(py, groups)?.as_any()),
    )?;
    if !completion.diagnostics.is_empty() {
        let mut details: Vec<String> = Vec::new();
        for diagnostic in &completion.diagnostics {
            details.push(format!(
                "- {}",
                native_field(diagnostic, "message")?.extract::<String>()?
            ));
        }
        let name: String = native_field(scenario, "name")?.extract()?;
        return Err(error(
            py,
            &format!(
                "Scenario '{name}' has invalid relation fixtures in {}:\n{}",
                context.label,
                details.join("\n")
            ),
            SCENARIO_PLAN_INVALID_FIXTURE,
            None,
        )?);
    }
    Ok(completion.sql_by_key.into_any())
}

fn model_context<'py>(
    project: &Bound<'py, PyAny>,
    context: &SqlContext<'py>,
    inputs: (&Bound<'py, PyDict>, &Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let (models, locations, columns) = inputs;
    let py = project.py();
    let none = || py.None();
    Ok(Bound::new(
        py,
        ModelPlanContext {
            model_locations: native_field(&context.relations, "model_locations")?.unbind(),
            models_by_name: models.clone().into_any().into(),
            functions_by_name: by_attribute(&project_reads::functions(project)?, "name")?
                .into_any()
                .unbind(),
            seed_locations: native_field(&context.relations, "seed_locations")?.unbind(),
            function_locations: locations.clone().unbind(),
            source_map: native_field(&context.relations, "source_map")?.unbind(),
            source_warehouse_columns: columns.clone().unbind(),
            star_exclude_keyword: context
                .adapter
                .call_method0("star_exclude_keyword")?
                .unbind(),
            runtime_cursor_producer_names: PyFrozenSet::empty(py)?.into_any().unbind(),
            future_cursor_config: none(),
            start_cursor_config: none(),
            invocation_time: none(),
        },
    )?
    .into_any())
}

/// Whether the scenario requires its expected CTEs to list every model output column.
fn requires_all_columns(scenario: &Bound<'_, PyAny>) -> PyResult<bool> {
    let header: Bound<'_, PyAny> =
        native_field(&native_field(scenario, "scenario_file")?, "header_values")?;
    let value: Bound<'_, PyAny> = mapping_get(&header, SCENARIO_EXPECTED_COLUMNS_KEY)?;
    Ok(!value.is_none() && value.extract::<String>()? == SCENARIO_EXPECTED_COLUMNS_ALL)
}

/// The columns compared by name, after rejecting an expected CTE that cannot be compared.
fn compared_columns(
    scenario: &Bound<'_, PyAny>,
    context: &SqlContext<'_>,
    (model, model_name): (Option<Bound<'_, PyAny>>, &str),
    authored_sql: &str,
) -> PyResult<Option<Vec<String>>> {
    let py = scenario.py();
    let model_columns: Option<Vec<String>> = match model {
        Some(model) => model_reads::inferred_names(&model)?,
        None => None,
    };
    let dialect: Option<String> =
        native_field(&context.adapter, "sql_analysis_dialect_name")?.extract()?;
    let columns =
        scenario_expected_columns(authored_sql, dialect.as_deref(), model_columns.as_deref());
    let scenario_name: String = native_field(scenario, "name")?.extract()?;
    let all_columns: bool = requires_all_columns(scenario)?;
    let Some(message) =
        scenario_expected_column_message(&scenario_name, model_name, &columns, all_columns)
    else {
        return Ok(columns.listed);
    };
    Err(error(
        py,
        &format!("{message} in {}", context.label),
        SCENARIO_PLAN_EXPECTED_COLUMNS,
        Some(SCENARIO_EXPECTED_COLUMNS_HELP),
    )?)
}

fn expectations<'py>(
    scenario: &Bound<'py, PyAny>,
    context: &SqlContext<'py>,
    models: &Bound<'py, PyDict>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py = scenario.py();
    let scope = scenario_helper_scope(scenario, context)?;
    let mut expected: Vec<Bound<'py, PyAny>> = Vec::new();
    for cte in native_field(scenario, "expected_ctes")?.try_iter()? {
        let cte = cte?;
        let name: String = native_field(&cte, "name")?.extract()?;
        let name = name.strip_prefix("__expected__").unwrap_or(&name);
        let target = required_target(&context.relations, "model_locations", name, "model")?;
        let authored: String = native_field(&cte, "sql_body")?.extract()?;
        let compared =
            compared_columns(scenario, context, (models.get_item(name)?, name), &authored)?;
        let sql = resolve_sql(
            context,
            &scenario_reader_sql(&scope, &authored, true),
            false,
        )?;
        expected.push(new_record_with(
            py,
            RecordId::ScenarioExpectedExpectationPlan,
            &[
                ("model_name", string(py, name)),
                ("actual_destination", target),
                ("expected_sql", string(py, &sql)),
                (
                    "compared_columns",
                    match compared {
                        Some(columns) => PyTuple::new(py, columns)?.into_any(),
                        None => py.None().into_bound(py),
                    },
                ),
            ],
        )?);
    }
    let mut assertions: Vec<Bound<'py, PyAny>> = Vec::new();
    for cte in native_field(scenario, "assertion_ctes")?.try_iter()? {
        let cte = cte?;
        let name: String = native_field(&cte, "name")?.extract()?;
        let authored: String = native_field(&cte, "sql_body")?.extract()?;
        let sql = resolve_sql(
            context,
            &scenario_reader_sql(&scope, &authored, true),
            false,
        )?;
        assertions.push(new_record_with(
            py,
            RecordId::ScenarioAssertionExpectationPlan,
            &[
                (
                    "name",
                    string(py, name.strip_prefix("__assert__").unwrap_or(&name)),
                ),
                ("sql", string(py, &sql)),
            ],
        )?);
    }
    Ok((
        PyTuple::new(py, expected)?.into_any(),
        PyTuple::new(py, assertions)?.into_any(),
    ))
}

/// The SQL-test wide window for a non-microbatch model whose SQL reads the cursor intrinsics.
fn scenario_cursor_window<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = model.py();
    let query: String = model_reads::query_sql(model)?.extract()?;
    let values = model_values(model)?;
    let microbatch: bool =
        config_value(py, &values, INCREMENTAL_MODE_KEY)?.eq(MICROBATCH_INCREMENTAL_MODE)?;
    if microbatch
        || !CURSOR_INTRINSIC_NAMES
            .iter()
            .any(|name| query.contains(name))
    {
        return new_record(py, RecordId::CursorOverridePair, (), None);
    }
    let integer: bool = config_value(py, &values, CURSOR_TYPE_KEY)?.eq(INTEGER_CURSOR_TYPE)?;
    let (start, end) = if integer {
        (DEFAULT_INTEGER_START, DEFAULT_INTEGER_END)
    } else {
        (DEFAULT_TIMESTAMP_START, DEFAULT_TIMESTAMP_END)
    };
    new_record_with(
        py,
        RecordId::CursorOverridePair,
        &[
            ("start_cursor_override", string(py, start)),
            ("end_cursor_override", string(py, end)),
        ],
    )
}
