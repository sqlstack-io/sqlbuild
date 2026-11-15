//! Resolve scenario SQL through the shared native scanner and adapter callbacks.

use crate::bindings::_helpers::planner_phases::objects::object;
use crate::bindings::_helpers::planner_scenario::constants::{
    SCENARIO_PLAN_MISSING_RELATION_TARGET, SCENARIO_PLAN_UNRESOLVED_RELATION_MARKER,
};
use crate::bindings::_helpers::planner_scenario::{
    models::SqlContext,
    values::{error, string},
};
use crate::bindings::_helpers::references::source_relations::render_source_relation;
use crate::bindings::_helpers::sqltext::lexical_syntax::LexicalSyntaxInput;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python, pyfunction};
use pyo3::types::{PyDict, PyStringMethods};
use sqlbuild_analysis::planner_scenario::{
    errors::MarkerError, main::replace_markers::replace_markers,
};
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_sqltext::sql_scan::models::Unclosed;

pub(super) fn resolve_sql(
    context: &SqlContext<'_>,
    sql: &str,
    project_sources: bool,
) -> PyResult<String> {
    let py = context.adapter.py();
    let syntax: LexicalSyntaxInput = native_field(&context.syntax, "native_mapping")?.extract()?;
    let mut result = sql.to_owned();
    for (kind, field) in [
        (0, "model_locations"),
        (1, "seed_locations"),
        (
            2,
            if project_sources {
                "project_source_map"
            } else {
                "source_map"
            },
        ),
        (3, "dbt_ref_fixture_locations"),
    ] {
        if project_sources && kind != super::constants::SOURCE_MARKER {
            continue;
        }
        let locations = context.relations.getattr(field)?;
        result = replace_markers(&result, kind, Some(syntax.clone().into()), |name, marker| {
            let target = mapping_get(&locations, name)?;
            let target = if target.is_none() {target} else if kind == super::constants::SOURCE_MARKER {
                object(py, render_source_relation(&target, Some(&context.adapter.clone()).filter(|value| !value.is_none()))?)?
            } else {native_field(&target, "qualified_name")?};
            if target.is_none() {
                let marker = string(py, marker).repr()?.to_str()?.to_owned();
                return Err(error(py, &format!("Scenario SQL in {} references {marker}, which is not a relation in this scenario", context.label),
                    SCENARIO_PLAN_UNRESOLVED_RELATION_MARKER, Some("Reference a model, seed, source, or dbt ref the scenario builds or provides as a fixture."))?);
            }
            target.extract::<String>()
        }).map_err(|failure| match failure {
            MarkerError::Resolve(error) => error,
            MarkerError::Scan(unclosed) => {
                let construct = match unclosed {Unclosed::BlockComment => "block comment", Unclosed::Quote => "quoted string", Unclosed::Parenthesis => "parenthesis"};
                error(py, &format!("Scenario SQL in {} could not be scanned for relation markers: SQL reference resolution contains an unclosed {construct}", context.label),
                    SCENARIO_PLAN_UNRESOLVED_RELATION_MARKER, Some("Close every quoted string and block comment using the project dialect's quoting and comment rules.")).unwrap_or_else(|error| error)
            }
        })?;
    }
    Ok(result)
}

pub(super) fn resolve_model_dbt_refs(sql: &str, relations: &Bound<'_, PyAny>) -> PyResult<String> {
    let locations = native_field(relations, "dbt_ref_fixture_locations")?;
    replace_markers(sql, 3, None, |name, marker| {
        let target = mapping_get(&locations, name)?;
        let qualified = if target.is_none() {
            target
        } else {
            native_field(&target, "qualified_name")?
        };
        Ok(if qualified.is_none() {
            marker.to_owned()
        } else {
            qualified.extract()?
        })
    })
    .map_err(|failure| match failure {
        MarkerError::Resolve(error) => error,
        MarkerError::Scan(_) => {
            pyo3::exceptions::PyRuntimeError::new_err("unscanned marker replacement failed")
        }
    })
}

pub(super) fn source_label(scenario: &Bound<'_, PyAny>) -> PyResult<String> {
    let source = native_field(scenario, "source_path")?;
    if source.is_truthy()? {
        Ok(source.str()?.to_str()?.to_owned())
    } else {
        native_field(scenario, "name")?.extract()
    }
}

pub(super) fn required_target<'py>(
    relations: &Bound<'py, PyAny>,
    field: &str,
    name: &str,
    kind: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let target = mapping_get(&native_field(relations, field)?, name)?;
    if target.is_none() {
        return Err(error(
            relations.py(),
            &format!("Scenario relation plan is missing {kind} target '{name}'"),
            SCENARIO_PLAN_MISSING_RELATION_TARGET,
            Some("This is likely a SQLBuild bug. Please file an issue with the scenario name."),
        )?);
    }
    Ok(target)
}

#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(crate) fn resolve_scenario_check_sql(
    py: Python<'_>,
    kwargs: Option<Bound<'_, PyDict>>,
) -> PyResult<String> {
    let args = KeywordArguments::read(
        py,
        "resolve_scenario_check_sql",
        kwargs,
        (
            &[
                "sql",
                "relation_plan",
                "adapter",
                "source_label",
                "source_lexical_syntax",
            ],
            &[],
        ),
    )?;
    let context = SqlContext {
        adapter: args.value("adapter")?,
        relations: args.value("relation_plan")?,
        syntax: args.value("source_lexical_syntax")?,
        label: args.value("source_label")?.extract()?,
    };
    resolve_sql(&context, &args.value("sql")?.extract::<String>()?, false)
}
