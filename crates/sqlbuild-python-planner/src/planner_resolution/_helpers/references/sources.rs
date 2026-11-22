//! Source contract validation and adapter-controlled projection rendering.

use crate::planner_resolution::_helpers::references::values::cursor_wrap;
use crate::planner_resolution::_helpers::references::values::filtering_of;
use crate::planner_resolution::_helpers::references::values::method;
use crate::planner_resolution::_helpers::references::values::string;
use crate::planner_resolution::models::CursorFiltering;
use crate::planner_strategy::main::values::input_error::input_error;
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_base::native_host::types::Syntax;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::type_system::main::types_equal::types_equal;
use sqlbuild_python_project::references::main::render_source_read::render_source_read;
use sqlbuild_sqltext::resolution::main::column_name_key::column_name_key;
use sqlbuild_sqltext::resolution::main::{
    reference_spans::reference_spans, source_alias::source_alias,
};
use sqlbuild_sqltext::sql_scan::main::bounded_expansions::bounded_expansions;
use sqlbuild_sqltext::sql_scan::main::dialect_lexical_syntax::dialect_lexical_syntax;
use sqlbuild_sqltext::sql_scan::main::expansion_followed_by::expansion_followed_by;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

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
    dialect: &Bound<'_, Dialect>,
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
        let equal: bool =
            types_equal(&dialect.syntax("sql_analysis_dialect_name")?, actual, &kind)?;
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
    dialect: &Bound<'py, Dialect>,
    name: &str,
    declared: &str,
    kind: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py = dialect.py();
    method(
        dialect,
        "render_source_expression_cast",
        &[
            (
                "expression",
                dialect.syntax("render_identifier")?.call1((name,))?,
            ),
            ("target_type", string(py, kind)),
            (
                "alias",
                dialect.syntax("render_identifier")?.call1((declared,))?,
            ),
        ],
    )
}

fn cast_source(
    entry: &Bound<'_, PyAny>,
    columns: &Bound<'_, PyAny>,
    dialect: &Bound<'_, Dialect>,
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
                cast_projection(dialect, &projection.name, declared, kind)?
            } else {
                dialect
                    .syntax("render_identifier")?
                    .call1((&projection.name,))?
            });
        }
        return method(
            dialect,
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
            projections.push(cast_projection(dialect, &projection.name, name, kind)?);
        }
    }
    method(
        dialect,
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
#[pyo3(signature = (*, dialect, source_entry))]
pub(crate) fn render_source_read_relation(
    dialect: &Bound<'_, Dialect>,
    source_entry: &Bound<'_, PyAny>,
) -> PyResult<String> {
    render_source_read(source_entry, dialect)
}

fn resolve_one(
    (source_map, warehouse_columns): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    dialect: &Bound<'_, Dialect>,
    filtering: &CursorFiltering<'_>,
    (name, original): (&str, &str),
) -> PyResult<String> {
    let entry = mapping_get(source_map, name)?;
    if entry.is_none() {
        return Ok(original.to_owned());
    }
    let dialect = dialect.clone();
    let mut relation = render_source_read_relation(&dialect, &entry)?;
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
        check_columns(&entry, &columns, &dialect, &label)?;
    }
    if native_field(&entry, "type_enforcement")?.is_truthy()? {
        relation = cast_source(&entry, &columns, &dialect, relation)?;
    }
    cursor_wrap(&dialect, filtering, relation, (name, false))
}

#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(crate) fn resolve_source_references(
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
                "dialect",
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
        &Dialect::passed(args.value("dialect")?)?,
        &filtering_of(&args)?,
    )
}

/// `__source` references replaced with their read relations, cursor inputs filtered; `lookups` holds the source map and warehouse columns.
pub(crate) fn resolve_sources(
    sql: &str,
    lookups: (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    dialect: &Bound<'_, Dialect>,
    filtering: &CursorFiltering<'_>,
) -> PyResult<String> {
    let aliases = dialect
        .syntax("requires_derived_table_aliases")?
        .call0()?
        .is_truthy()?;
    let mut output = String::new();
    let mut last = 0;
    let spans = reference_spans(sql, "source");
    if spans.is_empty() {
        return Ok(sql.to_owned());
    }
    let dialect_name: Option<String> = dialect.syntax("sql_analysis_dialect_name")?.extract()?;
    let syntax: LexicalSyntax = dialect_lexical_syntax(dialect_name.as_deref().unwrap_or_default());
    let mut relations: Vec<(usize, usize, String)> = Vec::with_capacity(spans.len());
    for span in spans {
        let mut relation = resolve_one(
            lookups,
            dialect,
            filtering,
            (&span.name, &sql[span.start..span.end]),
        )?;
        let mut end = span.end;
        if aliases && relation.trim_start().starts_with('(') {
            let (aliased_end, alias) = source_alias(sql, span.end, &span.name);
            let clause = format!(" AS {alias}");
            relation = expansion_followed_by(&relation, &clause, Some(&syntax));
            relation.push_str(&clause);
            end = aliased_end;
        }
        relations.push((span.start, end, relation));
    }
    let sites: Vec<(usize, usize, &str)> = relations
        .iter()
        .map(|(start, end, relation)| (*start, *end, relation.as_str()))
        .collect();
    let bounded = bounded_expansions(sql, &sites, Some(&syntax));
    for expansion in bounded {
        relations[expansion.index].2 = expansion.text;
    }
    for (start, end, relation) in &relations {
        output.push_str(sql.get(last..*start).unwrap_or_default());
        output.push_str(relation);
        last = *end;
    }
    output.push_str(&sql[last..]);
    Ok(output)
}
