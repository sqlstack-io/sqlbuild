//! Warehouse cursor column type checks.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyString};
use pyo3::{pyfunction, wrap_pyfunction};

use sqlbuild_analysis::planner_strategy::errors::TypeParseFailure;
use sqlbuild_analysis::planner_strategy::main::classify_cursor_sql_type::classify_cursor_sql_type as classify;
use sqlbuild_analysis::planner_strategy::main::cursor_type_warning::cursor_type_warning;
use sqlbuild_analysis::planner_strategy::models::{CursorType, CursorTypeCheck};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, call_with, imported};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::planner_changes::results::planner_enum;
use crate::bindings::_helpers::planner_strategy::values::{optional_string, plan_warning, text};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;

const PLANNER_LOGGER: &str = "sqlbuild.planner";

/// Log the parse error that left a warehouse type unclassified.
fn log_parse_failure(py: Python<'_>, failure: &TypeParseFailure) -> PyResult<()> {
    let text = |value: &str| PyString::new(py, value).into_any();
    call_with(
        &host::entry(py, ExecutorHost::LogDebugEvent)?,
        &[
            (
                "logger",
                imported(py, "logging", "getLogger")?.call1((PLANNER_LOGGER,))?,
            ),
            (
                "message",
                text("cursor type classification parse failed; falling back"),
            ),
            ("sqlbuild_warehouse_type", text(&failure.normalized_type)),
            ("sqlbuild_error", text(&failure.message)),
        ],
    )
    .map(|_| ())
}

/// The `CursorType` a supported cross-adapter SQL type holds, or `None`.
pub(crate) fn classify_cursor_sql_type<'py>(
    py: Python<'py>,
    sql_type: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let detected: Option<CursorType> = match classify(sql_type) {
        Ok(detected) => detected,
        Err(failure) => {
            log_parse_failure(py, &failure)?;
            None
        }
    };
    detected
        .map(|cursor_type| {
            let value: &str = match cursor_type {
                CursorType::Timestamp => "timestamp",
                CursorType::Integer => "integer",
            };
            planner_enum(py, RecordId::CursorType, value)
        })
        .transpose()
}

/// A `PlanWarning` when the warehouse cursor column type contradicts `cursor_type`.
#[pyfunction]
#[pyo3(signature = (**options))]
fn check_cursor_type_consistency<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "check_cursor_type_consistency",
        options,
        (
            &[
                "model_name",
                "cursor_column",
                "cursor_type",
                "warehouse_columns",
                "sql_analysis_enabled",
            ],
            &[],
        ),
    )?;
    let model_name: Bound<'py, PyAny> = arguments.value("model_name")?;
    let check = CursorTypeCheck {
        model_name: text(&model_name)?,
        cursor_column: optional_string(&arguments.value("cursor_column")?)?,
        cursor_type: optional_string(&arguments.value("cursor_type")?)?,
        warehouse_columns: warehouse_column_types(&arguments.value("warehouse_columns")?)?,
        sql_analysis_enabled: arguments.flag("sql_analysis_enabled")?,
    };
    cursor_type_warning_record(&model_name, &check)
}

/// The `(name, type)` pairs of warehouse `ColumnInfo`s.
pub(crate) fn warehouse_column_types(
    columns: &Bound<'_, PyAny>,
) -> PyResult<Vec<(String, String)>> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    for column in columns.try_iter()? {
        let column = column?;
        pairs.push((
            text(&native_field(&column, "name")?)?,
            text(&native_field(&column, "type")?)?,
        ));
    }
    Ok(pairs)
}

/// The `PlanWarning` of one cursor type check, logging an unclassifiable warehouse type.
pub(crate) fn cursor_type_warning_record<'py>(
    model_name: &Bound<'py, PyAny>,
    check: &CursorTypeCheck,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let (warning, parse_failure) = cursor_type_warning(check);
    if let Some(failure) = parse_failure {
        log_parse_failure(model_name.py(), &failure)?;
    }
    warning.map(|row| plan_warning(model_name, row)).transpose()
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(check_cursor_type_consistency, module)?)
}
