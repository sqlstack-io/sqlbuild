//! Reference lookup and adapter callbacks for planner SQL.

use sqlbuild_host::entries::models::AdapterHost;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::native_host::models::Dialect;

use crate::planner_resolution::_helpers::references::values::cursor_wrap;
use crate::planner_resolution::_helpers::references::values::filtering_of;
use crate::planner_resolution::_helpers::references::values::method;
use crate::planner_resolution::_helpers::references::values::string;
use crate::planner_resolution::models::CursorFiltering;
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_front::sqltext::models::SqlCodeScanner;
use sqlbuild_sqltext::resolution::main::over_clause_open::over_clause_open;
use sqlbuild_sqltext::resolution::main::{ref_alias::ref_alias, reference_spans::reference_spans};

#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(crate) fn resolve_ref_references(
    py: Python<'_>,
    kwargs: Option<Bound<'_, PyDict>>,
) -> PyResult<String> {
    let args = KeywordArguments::read(
        py,
        "resolve_ref_references",
        kwargs,
        (
            &[
                "query_sql",
                "model_locations",
                "seed_locations",
                "cursor_bounds",
                "cursor_filter_inputs",
                "dialect",
                "cursor_type",
                "lower_bound_inclusive",
            ],
            &[],
        ),
    )?;
    resolve_refs(
        &args.value("query_sql")?.extract::<String>()?,
        (
            &args.value("model_locations")?,
            &args.value("seed_locations")?,
        ),
        &Dialect::passed(args.value("dialect")?)?,
        &filtering_of(&args)?,
    )
}

/// `__ref` and `__seed` references replaced with their relations, cursor inputs filtered.
pub(crate) fn resolve_refs(
    original: &str,
    (model_locations, seed_locations): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    dialect: &Bound<'_, Dialect>,
    filtering: &CursorFiltering<'_>,
) -> PyResult<String> {
    let py = dialect.py();
    let mut sql = original.to_owned();
    for (kind, locations) in [("ref", model_locations), ("seed", seed_locations)] {
        let mut output = String::new();
        let mut last = 0;
        for span in reference_spans(&sql, kind) {
            output.push_str(&sql[last..span.start]);
            let target = mapping_get(locations, &span.name)?;
            if target.is_none() {
                output.push_str(&sql[span.start..span.end]);
            } else {
                let relation: String = call_with(
                    &host_entry(py, AdapterHost::ResolveRelationLocationQualifiedName)?,
                    &[
                        ("adapter", dialect.clone().into_any()),
                        ("location", target),
                    ],
                )?
                .extract()?;
                let char_end = sql[..span.end].chars().count();
                let original_end = original
                    .char_indices()
                    .nth(char_end)
                    .map_or(original.len(), |(offset, _)| offset);
                output.push_str(&cursor_wrap(
                    dialect,
                    filtering,
                    relation,
                    (&span.name, !ref_alias(original, original_end)),
                )?);
            }
            last = span.end;
        }
        output.push_str(&sql[last..]);
        sql = output;
    }
    Ok(sql)
}

#[pyfunction]
#[pyo3(signature = (*, query_sql, external_sql_reference_resolver=None))]
pub(crate) fn resolve_dbt_ref_references(
    query_sql: &str,
    external_sql_reference_resolver: Option<&Bound<'_, PyAny>>,
) -> PyResult<String> {
    let Some(resolver) = external_sql_reference_resolver.filter(|value| !value.is_none()) else {
        return Ok(query_sql.to_owned());
    };
    let py = resolver.py();
    let mut output = String::new();
    let mut last = 0;
    for span in reference_spans(query_sql, "dbt_ref") {
        output.push_str(&query_sql[last..span.start]);
        let relation: Option<String> = method(
            resolver,
            "resolve_reference",
            &[
                ("ref_kind", string(py, "dbt_ref")),
                ("ref_package", span.package.into_pyobject(py)?.into_any()),
                ("ref_name", string(py, &span.name)),
            ],
        )?
        .extract()?;
        output.push_str(
            relation
                .as_deref()
                .unwrap_or(&query_sql[span.start..span.end]),
        );
        last = span.end;
    }
    output.push_str(&query_sql[last..]);
    Ok(output)
}

fn resolve_function_calls(
    query_sql: &str,
    locations: &Bound<'_, PyAny>,
    dialect: &Bound<'_, Dialect>,
    table: bool,
) -> PyResult<String> {
    let py = dialect.py();
    let kind = if table { "table_fn" } else { "udf" };
    let spans = reference_spans(query_sql, kind);
    if spans.is_empty() {
        return Ok(query_sql.to_owned());
    }
    let scan = SqlCodeScanner::new(query_sql, None, "SQL reference resolution".to_owned());
    let mut output = String::new();
    let mut last = 0;
    let mut code_index = 0;
    for span in spans {
        if table {
            let start = query_sql[..span.start].chars().count();
            while code_index < start {
                let end: Option<usize> = scan.non_code_end(py, code_index)?;
                code_index = end.unwrap_or(code_index + 1);
            }
            if code_index > start {
                continue;
            }
            code_index = query_sql[..span.end].chars().count();
        }
        output.push_str(query_sql.get(last..span.start).unwrap_or_default());
        let target = mapping_get(locations, &span.name)?;
        let qualified = if target.is_none() {
            py.None().into_bound(py)
        } else {
            native_field(&target, "qualified_name")?
        };
        let suffix =
            span.end + query_sql[span.end..].len() - query_sql[span.end..].trim_start().len();
        if qualified.is_none() || !query_sql[suffix..].starts_with('(') {
            output.push_str(&query_sql[span.start..span.end]);
            last = span.end;
            continue;
        }
        let context = if table {
            "SQL table function call"
        } else {
            "SQL UDF call"
        };
        let scanner = SqlCodeScanner::new(query_sql, None, context.to_owned());
        let end: usize = scanner.matching_paren(py, query_sql[..suffix].chars().count())?;
        let end = query_sql
            .char_indices()
            .nth(end + 1)
            .map_or(query_sql.len(), |(index, _)| index);
        let end = if table {
            over_clause_end(query_sql, end, &scanner, py)?
        } else {
            end
        };
        let hook = if table {
            "render_table_function_call"
        } else {
            "render_udf_call"
        };
        let rendered: String = method(
            dialect,
            hook,
            &[
                ("target", qualified),
                ("call_suffix_sql", string(py, &query_sql[suffix..end])),
            ],
        )?
        .extract()?;
        output.push_str(&rendered);
        last = end;
    }
    if table {
        let length = query_sql.chars().count();
        while code_index < length {
            let end: Option<usize> = scan.non_code_end(py, code_index)?;
            code_index = end.unwrap_or(code_index + 1);
        }
    }
    output.push_str(&query_sql[last..]);
    Ok(output)
}

/// The end of an `OVER (...)` clause right after a table function call, or `end` without one.
fn over_clause_end(
    query_sql: &str,
    end: usize,
    scanner: &SqlCodeScanner,
    py: Python<'_>,
) -> PyResult<usize> {
    let Some(open) = over_clause_open(query_sql, end) else {
        return Ok(end);
    };
    let close: usize = scanner.matching_paren(py, query_sql[..open].chars().count())?;
    Ok(query_sql
        .char_indices()
        .nth(close + 1)
        .map_or(query_sql.len(), |(index, _)| index))
}

#[pyfunction]
#[pyo3(signature = (*, query_sql, function_locations, dialect))]
pub(crate) fn resolve_udf_references(
    query_sql: &str,
    function_locations: &Bound<'_, PyAny>,
    dialect: &Bound<'_, Dialect>,
) -> PyResult<String> {
    resolve_function_calls(query_sql, function_locations, dialect, false)
}

#[pyfunction]
#[pyo3(signature = (*, query_sql, function_locations, dialect))]
pub(crate) fn resolve_table_function_references(
    query_sql: &str,
    function_locations: &Bound<'_, PyAny>,
    dialect: &Bound<'_, Dialect>,
) -> PyResult<String> {
    resolve_function_calls(query_sql, function_locations, dialect, true)
}
