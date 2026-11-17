//! Selected-model inspection and untyped override conversion.

use crate::planner_resolution::_helpers::references::values::method;
use crate::planner_resolution::_helpers::references::values::string;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyErr;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_cursor::planner::main::override_kind::override_kind;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::model_config::main::config::config_text_is::config_text_is;
use sqlbuild_python_front::model_config::main::config::config_text_object::config_text_object;
use sqlbuild_python_front::model_config::main::config::model_values::model_values;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;

const MATERIALIZED_KEY: &str = "materialized";
const CURSOR_KEY: &str = "cursor";
const CURSOR_TYPE_KEY: &str = "cursor_type";
const INCREMENTAL: &str = "incremental";
const INTEGER_CURSOR: &str = "integer";
const TIMESTAMP_CURSOR: &str = "timestamp";

#[pyfunction(name = "type_cursor_overrides")]
#[pyo3(signature = (**kwargs))]
pub(crate) fn type_cursor_overrides_of<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "type_cursor_overrides",
        kwargs,
        (&["cursor_overrides"], &["selected_models"]),
    )?;
    let selected: Vec<Bound<'py, PyAny>> = if args.passed("selected_models")? {
        args.value("selected_models")?
            .try_iter()?
            .collect::<PyResult<_>>()?
    } else {
        Vec::new()
    };
    type_cursor_overrides(&args.value("cursor_overrides")?, &selected)
}

/// Untyped cursor overrides typed by the cursor type of the selected incremental models.
pub(crate) fn type_cursor_overrides<'py>(
    overrides: &Bound<'py, PyAny>,
    selected: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    let py = overrides.py();
    if overrides.is_none() || !native_field(overrides, "has_untyped")?.is_truthy()? {
        return Ok(overrides.clone());
    }
    let mut models: Vec<(String, String)> = Vec::new();
    for model in selected {
        let values = model_values(model)?;
        let incremental: bool = config_text_is(py, &values, MATERIALIZED_KEY, INCREMENTAL)?;
        if !incremental || config_text_object(py, &values, CURSOR_KEY)?.is_none() {
            continue;
        }
        for kind in [INTEGER_CURSOR, TIMESTAMP_CURSOR] {
            if config_text_is(py, &values, CURSOR_TYPE_KEY, kind)? {
                models.push((name_object(model)?.extract()?, kind.to_owned()));
            }
        }
    }
    let mut integer_literals = true;
    if models.is_empty() {
        for field in ["start", "end"] {
            let value = native_field(overrides, field)?;
            if value.is_none() {
                continue;
            }
            let stripped = value.call_method0("strip")?;
            let text: String = stripped.extract()?;
            let digits = text.strip_prefix(['+', '-']).unwrap_or(&text);
            if !string(py, digits).call_method0("isdecimal")?.is_truthy()? {
                integer_literals = false;
                break;
            }
        }
    }
    let kind = override_kind(&models, integer_literals).map_err(|message| {
        match error_class(py, ErrorId::PlannerInput).and_then(|class| call_with(&class, &[
            ("message", string(py, &message)), ("code", string(py, "S303")),
            ("help", string(py, "select models with one cursor type, or set each type explicitly with --start-cursor-ts/--end-cursor-ts and --start-cursor-int/--end-cursor-int")),
        ])) { Ok(error) => PyErr::from_value(error), Err(error) => error }
    })?;
    let kind = enum_from_value(py, RecordId::CursorType, kind)?;
    method(overrides, "typed_as", &[("cursor_type", kind)])
}
