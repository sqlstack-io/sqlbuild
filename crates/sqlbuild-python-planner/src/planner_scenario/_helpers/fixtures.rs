//! Materialize fixture SQL plans while preserving adapter and completion boundaries.

use crate::planner_scenario::_helpers::helper_scope::scenario_helper_scope;
use crate::planner_scenario::_helpers::sql::required_target;
use crate::planner_scenario::_helpers::sql::resolve_sql;
use crate::planner_scenario::_helpers::sql::source_label;
use crate::planner_scenario::_helpers::values::error;
use crate::planner_scenario::_helpers::values::string;
use crate::planner_scenario::constants::SCENARIO_PLAN_MISSING_FIXTURE_SQL;
use crate::planner_scenario::models::SqlContext;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python, pyfunction};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_analysis::compiler::main::scenario_reader_ctes::scenario_reader_ctes;
use sqlbuild_analysis::compiler::main::scenario_reader_sql::scenario_reader_sql;
use sqlbuild_analysis::compiler::models::ScenarioHelperScope;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::sqltext::models::LexicalSyntaxInput;
use sqlbuild_sqltext::sql_references::main::extract_sql_references::extract_sql_references;
use sqlbuild_sqltext::sql_references::models::ReferenceExtraction;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn extract_fixtures<'py>(
    scenario: &Bound<'py, PyAny>,
    prefix: &str,
) -> PyResult<Bound<'py, PyDict>> {
    sqlbuild_python_front::sql_test_glue::main::prefixed_ctes::prefixed_ctes(
        &native_field(scenario, "authored_ctes")?,
        prefix,
    )
}

pub(crate) fn fixture_plans<'py>(
    scenario: &Bound<'py, PyAny>,
    graph: &Bound<'py, PyAny>,
    context: &SqlContext<'py>,
    overrides: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py = scenario.py();
    let scope = scenario_helper_scope(scenario, context)?;
    let mut result: Vec<Bound<'py, PyAny>> = Vec::new();
    for (kind, resource, names, locations) in [
        (
            "source",
            "source",
            "source_fixture_names",
            "source_fixture_locations",
        ),
        ("ref", "model", "ref_fixture_names", "ref_fixture_locations"),
        (
            "seed",
            "seed",
            "seed_fixture_names",
            "seed_fixture_locations",
        ),
        (
            "dbt_ref",
            "dbt_ref",
            "dbt_ref_fixture_names",
            "dbt_ref_fixture_locations",
        ),
    ] {
        let fixtures = extract_fixtures(scenario, &format!("__{kind}__"))?;
        for name in native_field(graph, names)?.try_iter()? {
            let name: String = name?.extract()?;
            let target = required_target(&context.relations, locations, &name, kind)?;
            let Some(authored) = fixtures.get_item(&name)? else {
                return Err(error(
                    py,
                    &format!("Scenario is missing {kind} fixture SQL '{name}'"),
                    SCENARIO_PLAN_MISSING_FIXTURE_SQL,
                    None,
                )?);
            };
            let mut sql = authored.clone();
            if kind != crate::planner_scenario::constants::DBT_REF && !overrides.is_none() {
                sql = mapping_get_or(overrides, (resource, &name), sql)?;
            }
            let sql = resolve_sql(
                context,
                &scenario_reader_sql(&scope, &sql.extract::<String>()?, false),
                true,
            )?;
            let fingerprint: String =
                fingerprint_sql(scenario, (&scope, context), &authored.extract::<String>()?)?;
            result.push(new_record_with(
                py,
                RecordId::ScenarioFixturePlan,
                &[
                    (
                        "kind",
                        enum_from_value(py, RecordId::ScenarioArtifactKind, kind)?,
                    ),
                    ("logical_name", string(py, &name)),
                    ("destination", target),
                    ("sql", string(py, &sql)),
                    ("fingerprint_sql", string(py, &fingerprint)),
                ],
            )?);
        }
    }
    PyTuple::new(py, result)
}

/// The authored fixture, its helper CTEs, transitively, and each reference's fixture-or-relation target.
fn fingerprint_sql(
    scenario: &Bound<'_, PyAny>,
    (scope, context): (&ScenarioHelperScope, &SqlContext<'_>),
    authored: &str,
) -> PyResult<String> {
    let mut bodies: BTreeMap<String, String> = BTreeMap::new();
    for cte in native_field(scenario, "authored_ctes")?.try_iter()? {
        let cte = cte?;
        bodies.insert(
            native_field(&cte, "name")?.extract()?,
            native_field(&cte, "sql_body")?.extract()?,
        );
    }
    let mut text: String = authored.to_owned();
    let mut read: Vec<String> = vec![authored.to_owned()];
    for (name, _) in scenario_reader_ctes(scope, authored, false) {
        let body: &str = bodies.get(&name).map_or("", String::as_str);
        text.push_str(&format!("\n-- helper {name}\n{body}"));
        read.push(body.to_owned());
    }
    let syntax: LexicalSyntax = native_field(&context.syntax, "native_mapping")?
        .extract::<LexicalSyntaxInput>()?
        .into();
    let mut references: BTreeSet<String> = BTreeSet::new();
    for sql in &read {
        let ReferenceExtraction::Extracted(scan) = extract_sql_references(sql, &syntax) else {
            continue;
        };
        for reference in scan.references {
            let package: &str = reference.package.as_deref().unwrap_or("");
            let target: &str =
                if bodies.contains_key(&format!("__{}__{}", reference.kind, reference.name)) {
                    "fixture"
                } else {
                    "relation"
                };
            references.insert(format!(
                "{}:{package}:{} -> {target}",
                reference.kind, reference.name
            ));
        }
    }
    for reference in references {
        text.push_str(&format!("\n-- reads {reference}"));
    }
    Ok(text)
}

#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(crate) fn build_scenario_fixture_plans<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let args = KeywordArguments::read(
        py,
        "build_scenario_fixture_plans",
        kwargs,
        (
            &[
                "scenario",
                "graph_plan",
                "relation_plan",
                "adapter",
                "source_lexical_syntax",
            ],
            &["fixture_sql_overrides"],
        ),
    )?;
    let scenario = args.value("scenario")?;
    let context = SqlContext {
        adapter: args.value("adapter")?,
        relations: args.value("relation_plan")?,
        syntax: args.value("source_lexical_syntax")?,
        label: source_label(&scenario)?,
    };
    fixture_plans(
        &scenario,
        &args.value("graph_plan")?,
        &context,
        &args.value("fixture_sql_overrides")?,
    )
}
