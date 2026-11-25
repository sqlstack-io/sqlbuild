//! Model, function and SQL-hook orchestration through adapter-owned rendering.

use crate::planner_phases::constants::UNBOUNDED_CURSOR_MARK;
use crate::planner_resolution::_helpers::cursor::cursor_model::{
    compute_model_cursor_bounds, unbounded,
};
use crate::planner_resolution::constants::CURSOR_START;
use crate::planner_resolution::main::references::model_hook_entries::model_hook_entries;
use crate::planner_resolution::models::Resolution;
use crate::planner_resolution::models::{CursorFiltering, ModelSqlRequest};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyString, PyTuple};
use pyo3::{PyErr, pyfunction};
use sqlbuild_model_config::config_values::models::ConfigNode;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_base::native_host::types::Syntax;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_front::attachments::main::resolve_cursor_intrinsics::resolve_cursor_intrinsics;
use sqlbuild_python_front::cursor::main::scalars::render_value::render_value;
use sqlbuild_python_front::cursor::main::scalars::sentinel_to_token::sentinel_to_token;
use sqlbuild_python_front::cursor::main::scalars::try_parse::try_parse;
use sqlbuild_python_front::model_config::main::config::config_cursor_bound::config_cursor_bound;
use sqlbuild_python_front::model_config::main::config::config_node_of::config_node_of;
use sqlbuild_python_front::model_config::main::config::config_text_is::config_text_is;
use sqlbuild_python_front::model_config::main::config::config_text_object::config_text_object;
use sqlbuild_python_front::model_config::main::config::model_values::model_values;
use sqlbuild_python_project::compile_attachment::main::resolve_cursor_input_roles::resolve_cursor_input_roles;
use sqlbuild_python_project::compiled_views::main::compiled_models::model_name::model_name;
use sqlbuild_python_project::compiled_views::main::compiled_models::query_sql::query_sql;

const APPEND_STRATEGY: &str = "append";

#[pyfunction(name = "resolve_model_sql")]
#[pyo3(signature = (**kwargs))]
pub(crate) fn resolve_model_sql_of<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "resolve_model_sql",
        kwargs,
        (
            &[
                "dialect",
                "model",
                "snapshot",
                "context",
                "backfill",
                "full_refresh",
                "cursor_overrides",
            ],
            &[
                "external_sql_reference_resolver",
                "replaces_relation",
                "full_rebuild_cause",
            ],
        ),
    )?;
    let context = args.value("context")?;
    let resolution = Resolution::of_context(
        Dialect::passed(args.value("dialect")?)?,
        &context,
        args.value("external_sql_reference_resolver")?,
    )?;
    let overrides = args.value("cursor_overrides")?;
    let cause = args.value("full_rebuild_cause")?;
    let request = ModelSqlRequest {
        model: args.value("model")?,
        snapshot: args.value("snapshot")?,
        backfill: args.value("backfill")?,
        full_refresh: args.flag("full_refresh")?,
        cursor_overrides: (
            native_field(&overrides, "start_cursor_override")?,
            native_field(&overrides, "end_cursor_override")?,
        ),
        replaces_relation: args.flag("replaces_relation")?,
        full_rebuild_cause: if cause.is_truthy()? {
            Some(cause.extract()?)
        } else {
            None
        },
    };
    let sql: String = resolve_model_sql(&resolution, &context, &request)?;
    Ok(PyString::new(py, &sql).into_any())
}

/// One model's planned SQL: references resolved, cursor inputs filtered and intrinsics bound.
pub(crate) fn resolve_model_sql<'py>(
    resolution: &Resolution<'py>,
    context: &Bound<'py, PyAny>,
    request: &ModelSqlRequest<'py>,
) -> PyResult<String> {
    let model = &request.model;
    let py = model.py();
    let cursor_type = config_text_object(py, &model_values(model)?, "cursor_type")?
        .unwrap_or_else(|| py.None().into_bound(py));
    let bounds = compute_model_cursor_bounds(request, context)?;
    let roles = resolve_cursor_input_roles(model)?;
    let filtering = CursorFiltering {
        bounds: bounds.clone(),
        filter_inputs: native_field(&roles, "filter_inputs")?,
        cursor_type: cursor_type.clone(),
        lower_bound_inclusive: append_cursor_inclusive(model)?,
    };
    let mut sql = resolution.resolve(&query_sql(model)?.extract::<String>()?, &filtering)?;
    let (_, uses_intrinsics) = resolve_cursor_intrinsics(py, &sql, None, None)?;
    let name: String = model_name(model)?;
    if uses_intrinsics {
        let cause = match &request.full_rebuild_cause {
            Some(cause) => Some(cause.clone()),
            None if request.full_refresh => Some("--full-refresh".to_owned()),
            None => None,
        };
        let bounds = match bounds {
            bounds if !bounds.is_none() => bounds,
            _ if cause.is_none()
                && native_field(&request.snapshot, "cursor_inputs_at_run")?.is_truthy()?
                && native_field(&roles, "watermark_inputs")?.is_truthy()? =>
            {
                unbounded(py)?
            }
            _ if cause.is_some() => {
                sql = full_build_bounds(&sql, model, &cursor_type, &resolution.dialect)?;
                resolution.assert_resolved(&sql, &format!("Model '{name}'"))?;
                return Ok(sql);
            }
            _ => return Err(unresolved_bounds_error(py, &name)?),
        };
        sql = intrinsic_bounds(&sql, &bounds, &cursor_type, &resolution.dialect)?;
    }
    resolution.assert_resolved(&sql, &format!("Model '{name}'"))?;
    Ok(sql)
}

/// Effective append cursor lower-bound inclusivity.
fn append_cursor_inclusive(model: &Bound<'_, PyAny>) -> PyResult<bool> {
    let values = model_values(model)?;
    let py: Python<'_> = model.py();
    let append: bool = config_text_is(py, &values, "incremental_strategy", APPEND_STRATEGY)?;
    let inclusive: bool = match *config_node_of(&values, "append_cursor_inclusive")? {
        ConfigNode::Bool(flag) => flag,
        _ => true,
    };
    Ok(!append || inclusive)
}

/// `__cursor_start()` / `__cursor_end()` replaced with the adapter's typed bound literals.
fn intrinsic_bounds<'py>(
    sql: &str,
    bounds: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
    dialect: &Bound<'py, Dialect>,
) -> PyResult<String> {
    let py = dialect.py();
    let literal = |field: &str| -> PyResult<String> {
        call_with(
            &dialect.syntax("render_cursor_bound_literal")?,
            &[
                (
                    "value",
                    PyString::new(py, &sentinel_to_token(&native_field(bounds, field)?)?)
                        .into_any(),
                ),
                ("cursor_type", cursor_type.clone()),
            ],
        )?
        .extract()
    };
    let start_sql = literal("start")?;
    let end_sql = literal("end")?;
    Ok(resolve_cursor_intrinsics(py, sql, Some(&start_sql), Some(&end_sql))?.0)
}

fn unresolved_bounds_error(py: Python<'_>, name: &str) -> PyResult<PyErr> {
    let error = error_class(py, ErrorId::PlannerInput)?;
    let message =
        format!("Model '{name}' uses cursor intrinsics, but cursor bounds could not be resolved");
    Ok(PyErr::from_value(error.call1((message,))?))
}

/// A non-microbatch full build's intrinsics: the model's `cursor_start` floor or the cursor type's lowest value, to its highest.
fn full_build_bounds<'py>(
    sql: &str,
    model: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
    dialect: &Bound<'py, Dialect>,
) -> PyResult<String> {
    let py = model.py();
    let (lowest, highest): (String, String) = call_with(
        &dialect.syntax("cursor_type_range")?,
        &[("cursor_type", cursor_type.clone())],
    )?
    .extract()?;
    let literal = |value: &str| -> PyResult<String> {
        call_with(
            &dialect.syntax("render_cursor_bound_literal")?,
            &[
                ("value", PyString::new(py, value).into_any()),
                ("cursor_type", cursor_type.clone()),
            ],
        )?
        .extract()
    };
    let floor = match config_cursor_bound(py, &model_values(model)?, CURSOR_START)? {
        Some(floor) => try_parse(&floor, cursor_type)?,
        None => None,
    };
    let start = match floor {
        Some(floor) => literal(&render_value(&floor)?)?,
        None => format!("{} {UNBOUNDED_CURSOR_MARK}", literal(&lowest)?),
    };
    let end = format!("{} {UNBOUNDED_CURSOR_MARK}", literal(&highest)?);
    Ok(resolve_cursor_intrinsics(py, sql, Some(&start), Some(&end))?.0)
}

#[pyfunction]
#[pyo3(signature = (*, model, dialect, context, external_sql_reference_resolver))]
pub(crate) fn resolve_model_hook_entries<'py>(
    model: &Bound<'py, PyAny>,
    dialect: Bound<'py, Dialect>,
    context: Bound<'py, PyAny>,
    external_sql_reference_resolver: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let resolution = Resolution::of_context(dialect, &context, external_sql_reference_resolver)?;
    model_hook_entries(model, &resolution)
}
