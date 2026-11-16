//! Warehouse cursor column type checks.

use crate::planner_strategy::main::values::text::text;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyString};

use sqlbuild_analysis::planner_strategy::errors::TypeParseFailure;
use sqlbuild_analysis::planner_strategy::main::cursor_type_warning::cursor_type_warning;
use sqlbuild_analysis::planner_strategy::models::CursorTypeCheck;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::planner_strategy::_helpers::values::{optional_string, plan_warning};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const PLANNER_LOGGER: &str = "sqlbuild.planner";

/// Log the Polyglot parse error that left a warehouse type unclassified.
pub(crate) fn log_parse_failure(py: Python<'_>, failure: &TypeParseFailure) -> PyResult<()> {
    let text = |value: &str| PyString::new(py, value).into_any();
    call_with(
        &host_entry(py, ExecutorHost::LogDebugEvent)?,
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

/// A `PlanWarning` when the warehouse cursor column type contradicts `cursor_type`.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn check_cursor_type_consistency<'py>(
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
