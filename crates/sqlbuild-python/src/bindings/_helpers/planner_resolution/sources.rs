//! Source contract validation and adapter-controlled projection rendering.

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::planner_resolution::models::CursorFiltering;
use crate::bindings::_helpers::planner_resolution::values::{
    cursor_wrap, filtering_of, method, string,
};
use crate::bindings::_helpers::planner_strategy::values::input_error;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::ids::ErrorId;
use crate::bindings::_helpers::references::source_relations::render_source_read;
use crate::bindings::_helpers::type_system::equality::types_equal;
use pyo3::IntoPyObject;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_sqltext::resolution::main::column_name_key::column_name_key;
use sqlbuild_sqltext::resolution::main::{
    reference_spans::reference_spans, source_alias::source_alias,
};

fn source_error(py: Python<'_>, message: &str, code: &str) -> PyErr {
    input_error(py, ErrorId::PlannerInput, message, Some(code))
}

fn rows(value: &Bound<'_, PyAny>) -> PyResult<Vec<(String, Option<String>)>> {
    value
        .try_iter()?
        .map(|row| {
            let row = row?;
            Ok((
                native_field(&row, "name")?.extract()?,
                native_field(&row, "type")?.extract()?,
            ))
        })
        .collect()
}

fn check_columns(
    entry: &Bound<'_, PyAny>,
    columns: &Bound<'_, PyAny>,
    adapter: &Bound<'_, PyAny>,
    relation: &str,
) -> PyResult<()> {
    let py = entry.py();
    let declared = rows(&native_field(entry, "columns")?)?;
    let available = rows(columns)?;
    let available: std::collections::HashMap<String, Option<String>> = available
        .into_iter()
        .map(|(name, kind)| (column_name_key(&name), kind))
        .collect();
    let missing = declared
        .iter()
        .filter(|(name, _)| !available.contains_key(&column_name_key(name)))
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(source_error(
            py,
            &format!(
                "source {relation} declares columns not found in warehouse: {}",
                missing.join(", ")
            ),
            "S401",
        ));
    }
    if native_field(entry, "contract")?
        .extract::<Option<String>>()?
        .as_deref()
        != Some("enforced")
    {
        return Ok(());
    }
    for (name, kind) in declared {
        let Some(kind) = kind else {
            continue;
        };
        let Some(Some(actual)) = available.get(&column_name_key(&name)) else {
            continue;
        };
        if actual.is_empty() {
            continue;
        }
        let equal: bool = types_equal(
            &native_field(adapter, "sql_analysis_dialect_name")?,
            actual,
            &kind,
        )?;
        if !equal {
            return Err(source_error(
                py,
                &format!(
                    "source {relation} column '{name}' has type {actual} but contract declares {kind}"
                ),
                "S404",
            ));
        }
    }
    Ok(())
}

fn cast_projection<'py>(
    adapter: &Bound<'py, PyAny>,
    name: &str,
    declared: &str,
    kind: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py = adapter.py();
    method(
        adapter,
        "render_source_expression_cast",
        &[
            (
                "expression",
                adapter.call_method1("render_identifier", (name,))?,
            ),
            ("target_type", string(py, kind)),
            (
                "alias",
                adapter.call_method1("render_identifier", (declared,))?,
            ),
        ],
    )
}

fn cast_source(
    entry: &Bound<'_, PyAny>,
    columns: &Bound<'_, PyAny>,
    adapter: &Bound<'_, PyAny>,
    relation: String,
) -> PyResult<String> {
    let declared = rows(&native_field(entry, "columns")?)?;
    if !declared.iter().any(|(_, kind)| kind.is_some()) {
        return Ok(relation);
    }
    let py = entry.py();
    let expression = !native_field(entry, "expression")?.is_none();
    let source_name: String = native_field(entry, "name")?.extract()?;
    if columns.is_none() {
        if expression {
            return Err(source_error(
                py,
                &format!(
                    "source expression '{source_name}' type enforcement requires query output column metadata"
                ),
                "S402",
            ));
        }
        return Ok(relation);
    }
    let available = rows(columns)?;
    let plan = sqlbuild_sqltext::resolution::main::source_cast_plan::source_cast_plan(
        &declared, &available, expression,
    );
    let names: Vec<&str> = available.iter().map(|(name, _)| name.as_str()).collect();
    if expression {
        let name_map: std::collections::HashMap<String, &str> = names
            .iter()
            .map(|name| (column_name_key(name), *name))
            .collect();
        let missing = declared
            .iter()
            .filter(|(name, _)| !name_map.contains_key(&column_name_key(name)))
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            let available = if names.is_empty() {
                "<none>".to_owned()
            } else {
                names.join(", ")
            };
            return Err(source_error(
                py,
                &format!(
                    "source expression '{source_name}' declares columns not found in query output: {}. Available query output columns: {available}",
                    missing.join(", ")
                ),
                "S403",
            ));
        }
        let mut projections: Vec<Bound<'_, PyAny>> = Vec::new();
        for projection in &plan.projections {
            projections.push(if let Some((declared, kind)) = &projection.cast {
                cast_projection(adapter, &projection.name, declared, kind)?
            } else {
                adapter.call_method1("render_identifier", (&projection.name,))?
            });
        }
        return method(
            adapter,
            "render_source_expression_cast_subquery",
            &[
                ("source_relation", string(py, &relation)),
                ("projections", PyTuple::new(py, projections)?.into_any()),
            ],
        )?
        .extract();
    }
    if plan.projections.is_empty() {
        return Ok(relation);
    }
    let mut projections: Vec<Bound<'_, PyAny>> = Vec::new();
    for projection in &plan.projections {
        if let Some((name, kind)) = &projection.cast {
            projections.push(cast_projection(adapter, &projection.name, name, kind)?);
        }
    }
    method(
        adapter,
        "_render_source_relation_cast_subquery_with_columns",
        &[
            ("source_relation", string(py, &relation)),
            (
                "cast_projections",
                PyTuple::new(py, projections)?.into_any(),
            ),
            (
                "cast_column_names",
                PyTuple::new(
                    py,
                    plan.projections.iter().map(|projection| &projection.name),
                )?
                .into_any(),
            ),
            (
                "warehouse_column_names",
                PyTuple::new(py, names)?.into_any(),
            ),
            (
                "all_columns_cast",
                plan.all_columns_cast
                    .into_pyobject(py)?
                    .to_owned()
                    .into_any(),
            ),
        ],
    )?
    .extract()
}

#[pyfunction]
#[pyo3(signature = (*, adapter, source_entry))]
pub(crate) fn render_source_read_relation(
    adapter: &Bound<'_, PyAny>,
    source_entry: &Bound<'_, PyAny>,
) -> PyResult<String> {
    render_source_read(source_entry, adapter)
}

fn resolve_one(
    (source_map, warehouse_columns): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    adapter: &Bound<'_, PyAny>,
    filtering: &CursorFiltering<'_>,
    (name, original): (&str, &str),
) -> PyResult<String> {
    let entry = mapping_get(source_map, name)?;
    if entry.is_none() {
        return Ok(original.to_owned());
    }
    let adapter = adapter.clone();
    let mut relation = render_source_read_relation(&adapter, &entry)?;
    let columns = mapping_get(warehouse_columns, name)?;
    let expression = !native_field(&entry, "expression")?.is_none();
    if columns.is_truthy()?
        && (!expression
            || native_field(&entry, "contract")?
                .extract::<Option<String>>()?
                .as_deref()
                == Some("enforced"))
    {
        let label = if expression {
            format!("source expression '{name}'")
        } else {
            relation.clone()
        };
        check_columns(&entry, &columns, &adapter, &label)?;
    }
    if native_field(&entry, "type_enforcement")?.is_truthy()? {
        relation = cast_source(&entry, &columns, &adapter, relation)?;
    }
    cursor_wrap(&adapter, filtering, relation, (name, false))
}

#[pyfunction]
#[pyo3(signature = (**kwargs))]
fn resolve_source_references(
    py: Python<'_>,
    kwargs: Option<Bound<'_, PyDict>>,
) -> PyResult<String> {
    let args = KeywordArguments::read(
        py,
        "resolve_source_references",
        kwargs,
        (
            &[
                "query_sql",
                "source_map",
                "source_warehouse_columns",
                "star_exclude_keyword",
                "cursor_bounds",
                "cursor_filter_inputs",
                "adapter",
                "cursor_type",
                "lower_bound_inclusive",
            ],
            &[],
        ),
    )?;
    resolve_sources(
        &args.value("query_sql")?.extract::<String>()?,
        (
            &args.value("source_map")?,
            &args.value("source_warehouse_columns")?,
        ),
        &args.value("adapter")?,
        &filtering_of(&args)?,
    )
}

/// `__source` references replaced with their read relations, cursor inputs filtered; `lookups` holds the source map and warehouse columns.
pub(crate) fn resolve_sources(
    sql: &str,
    lookups: (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    adapter: &Bound<'_, PyAny>,
    filtering: &CursorFiltering<'_>,
) -> PyResult<String> {
    let aliases = adapter
        .call_method0("requires_derived_table_aliases")?
        .is_truthy()?;
    let mut output = String::new();
    let mut last = 0;
    for span in reference_spans(sql, "source") {
        let mut relation = resolve_one(
            lookups,
            adapter,
            filtering,
            (&span.name, &sql[span.start..span.end]),
        )?;
        output.push_str(sql.get(last..span.start).unwrap_or_default());
        last = span.end;
        if aliases && relation.trim_start().starts_with('(') {
            let (end, alias) = source_alias(sql, span.end, &span.name);
            relation.push_str(&format!(" AS {alias}"));
            last = end;
        }
        output.push_str(&relation);
    }
    output.push_str(&sql[last..]);
    Ok(output)
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_source_references, module)?)?;
    module.add_function(wrap_pyfunction!(render_source_read_relation, module)?)?;
    Ok(())
}
