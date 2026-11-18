//! Cursor windows rendered for models that SQL tests evaluate from their real SQL.

use std::cmp::Ordering;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_cursor::algebra::main::compare::compare;
use sqlbuild_cursor::algebra::main::floor_to_grain::floor_to_grain;
use sqlbuild_cursor::algebra::models::{Grain, Scalar};

use crate::compiled_views::main::compiled_models::name_object::name_object;
use crate::compiled_views::main::compiled_models::query_sql::query_sql;
use crate::sql_test_glue::constants::CURSOR_ALGEBRA_EXCEPTIONS;
use crate::sql_test_glue::constants::CURSOR_END_KEY;
use crate::sql_test_glue::constants::CURSOR_START_KEY;
use crate::sql_test_glue::constants::DEFAULT_INTEGER_END;
use crate::sql_test_glue::constants::DEFAULT_INTEGER_START;
use crate::sql_test_glue::constants::DEFAULT_TIMESTAMP_END;
use crate::sql_test_glue::constants::DEFAULT_TIMESTAMP_START;
use crate::sql_test_glue::constants::INTEGER_CURSOR_TYPE;
use crate::sql_test_glue::constants::TIMESTAMP_CURSOR_TYPE;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::attachments::main::compile_input_error::compile_input_error;
use sqlbuild_python_front::attachments::main::resolve_cursor_intrinsics::resolve_cursor_intrinsics;
use sqlbuild_python_front::cursor::main::scalars::parse_value::parse_value;
use sqlbuild_python_front::cursor::main::scalars::render_value::render_value;
use sqlbuild_python_front::cursor::models::CursorSession;
use sqlbuild_python_front::model_config::main::config::config_text_object::config_text_object;
use sqlbuild_python_front::model_config::main::config::model_values::model_values;

/// One model's cursor name, type and timestamp grain.
struct ModelCursor<'py> {
    name: String,
    cursor_type: Bound<'py, PyAny>,
    type_name: String,
    grain: Option<(String, Grain)>,
}

/// One declared bound with the defaults and key used when it is absent or invalid.
struct BoundRequest<'a, 'py> {
    raw: Option<Bound<'py, PyAny>>,
    default: &'a str,
    key: &'a str,
}

/// Whether SQL calls `__cursor_start()` or `__cursor_end()`.
pub(crate) fn uses_cursor_intrinsics(py: Python<'_>, sql: &str) -> PyResult<bool> {
    Ok(resolve_cursor_intrinsics(py, sql, None, None)?.1)
}

/// Whether a SQL test header declares its own cursor window.
pub(crate) fn declares_cursor_window(test: &Bound<'_, PyAny>) -> PyResult<bool> {
    let block = native_field(test, "test_block")?;
    Ok(!native_field(&block, "cursor_start")?.is_none()
        || !native_field(&block, "cursor_end")?.is_none())
}

/// SQL with intrinsics rendered from the test's declared window, or the wide default window.
pub(crate) fn render_test_cursor_intrinsics<'py>(
    sql: Bound<'py, PyAny>,
    model: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
    test: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = sql.py();
    let text: String = sql.extract()?;
    if !uses_cursor_intrinsics(py, &text)? {
        return Ok(sql);
    }
    let cursor = model_cursor(model)?;
    let (start, end) = test_window(&cursor, test)?;
    let literal = |value: &Bound<'py, PyAny>| -> PyResult<String> {
        call_with(
            &native_field(adapter, "render_cursor_bound_literal")?,
            &[
                ("value", PyString::new(py, &render_value(value)?).into_any()),
                ("cursor_type", cursor.cursor_type.clone()),
            ],
        )?
        .extract()
    };
    let (start_sql, end_sql) = (literal(&start)?, literal(&end)?);
    let (rendered, _) = resolve_cursor_intrinsics(py, &text, Some(&start_sql), Some(&end_sql))?;
    Ok(PyString::new(py, &rendered).into_any())
}

/// Chain models using intrinsics, after validating the test's declared window against each.
pub(crate) fn declared_window_cursor_models<'py>(
    test: &Bound<'py, PyAny>,
    chain_models: Vec<Bound<'py, PyAny>>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py = test.py();
    let mut cursor_models: Vec<Bound<'py, PyAny>> = Vec::new();
    for model in chain_models {
        if uses_cursor_intrinsics(py, &query_sql(&model)?.extract::<String>()?)? {
            cursor_models.push(model);
        }
    }
    if cursor_models.is_empty() {
        return Err(compile_input_error(
            py,
            format!(
                "{} declares cursor_start or cursor_end, but no model it evaluates uses __cursor_start() or __cursor_end()",
                test_label(test)?
            ),
        )?);
    }
    for model in &cursor_models {
        test_window(&model_cursor(model)?, Some(test))?;
    }
    Ok(cursor_models)
}

/// The start and exclusive end of one model's window, validating declared bounds.
fn test_window<'py>(
    cursor: &ModelCursor<'py>,
    test: Option<&Bound<'py, PyAny>>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let block = test
        .map(|test| native_field(test, "test_block"))
        .transpose()?;
    let declared = |name: &str| -> PyResult<Option<Bound<'py, PyAny>>> {
        let Some(block) = &block else {
            return Ok(None);
        };
        let value = native_field(block, name)?;
        Ok((!value.is_none()).then_some(value))
    };
    let integer = cursor.type_name == INTEGER_CURSOR_TYPE;
    let start = bound(
        BoundRequest {
            raw: declared(CURSOR_START_KEY)?,
            default: if integer {
                DEFAULT_INTEGER_START
            } else {
                DEFAULT_TIMESTAMP_START
            },
            key: CURSOR_START_KEY,
        },
        cursor,
        test,
    )?;
    let end = bound(
        BoundRequest {
            raw: declared(CURSOR_END_KEY)?,
            default: if integer {
                DEFAULT_INTEGER_END
            } else {
                DEFAULT_TIMESTAMP_END
            },
            key: CURSOR_END_KEY,
        },
        cursor,
        test,
    )?;
    if let Some(test) = test
        && ordering(&start, &end)? != Ordering::Less
    {
        return Err(compile_input_error(
            test.py(),
            format!(
                "{}: cursor_start '{}' must be before the exclusive cursor_end '{}' for model '{}'",
                test_label(test)?,
                render_value(&start)?,
                render_value(&end)?,
                cursor.name
            ),
        )?);
    }
    Ok((start, end))
}

/// One parsed bound, or the parsed default when the test declares none.
fn bound<'py>(
    request: BoundRequest<'_, 'py>,
    cursor: &ModelCursor<'py>,
    test: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = cursor.cursor_type.py();
    let (Some(raw), Some(test)) = (request.raw, test) else {
        return parse_value(
            &PyString::new(py, request.default).into_any(),
            &cursor.cursor_type,
        );
    };
    let value = match parse_value(&raw, &cursor.cursor_type) {
        Ok(value) => value,
        Err(error)
            if error.value(py).is_instance(&imported(
                py,
                CURSOR_ALGEBRA_EXCEPTIONS,
                "CursorAlgebraError",
            )?)? =>
        {
            let expected = if cursor.type_name == INTEGER_CURSOR_TYPE {
                "an integer"
            } else {
                "an ISO timestamp or date"
            };
            return Err(compile_input_error(
                py,
                format!(
                    "{}: {} '{}' must be {expected} because model '{}' uses cursor_type {}",
                    test_label(test)?,
                    request.key,
                    raw.str()?,
                    cursor.name,
                    cursor.type_name
                ),
            )?);
        }
        Err(error) => return Err(error),
    };
    if let Some((grain_name, grain)) = &cursor.grain {
        let session = CursorSession::new(py)?;
        let scalar: Scalar = session.scalar(&value)?;
        let floored: Scalar = floor_to_grain(&scalar, Some(*grain));
        if session.checked(compare(&floored, &scalar, &session))? != Ordering::Equal {
            return Err(compile_input_error(
                py,
                format!(
                    "{}: {} '{}' is not aligned to cursor_grain {grain_name} of model '{}'",
                    test_label(test)?,
                    request.key,
                    raw.str()?,
                    cursor.name
                ),
            )?);
        }
    }
    Ok(value)
}

/// How two parsed cursor values order.
fn ordering(left: &Bound<'_, PyAny>, right: &Bound<'_, PyAny>) -> PyResult<Ordering> {
    let session = CursorSession::new(left.py())?;
    let (left, right) = (session.scalar(left)?, session.scalar(right)?);
    session.checked(compare(&left, &right, &session))
}

/// The cursor type and timestamp grain a model's config declares.
fn model_cursor<'py>(model: &Bound<'py, PyAny>) -> PyResult<ModelCursor<'py>> {
    let py = model.py();
    let values = model_values(model)?;
    let declared = config_text_object(py, &values, "cursor_type")?.filter(|value| {
        value
            .extract::<String>()
            .is_ok_and(|value| !value.is_empty())
    });
    let cursor_type = record_class(py, RecordId::CursorType)?.call1((
        declared.unwrap_or_else(|| PyString::new(py, TIMESTAMP_CURSOR_TYPE).into_any()),
    ))?;
    let type_name: String = native_field(&cursor_type, "value")?.extract()?;
    let grain = match config_text_object(py, &values, "cursor_grain")? {
        Some(grain) if type_name == TIMESTAMP_CURSOR_TYPE => {
            let grain = enum_from_value(py, RecordId::CursorGrain, grain)?;
            let session = CursorSession::new(py)?;
            session
                .grain(&grain)?
                .map(|known| PyResult::Ok((native_field(&grain, "value")?.extract()?, known)))
                .transpose()?
        }
        _ => None,
    };
    Ok(ModelCursor {
        name: name_object(model)?.extract()?,
        cursor_type,
        type_name,
        grain,
    })
}

/// `SQL test '<name>' in '<path>'`.
fn test_label(test: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(format!(
        "SQL test '{}' in '{}'",
        native_field(test, "name")?.str()?,
        native_field(&native_field(test, "test_file")?, "relative_path")?.str()?
    ))
}
