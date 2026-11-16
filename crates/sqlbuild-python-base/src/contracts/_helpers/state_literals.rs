//! Portable typed SQL literals for warehouse-state columns.

use crate::native_host::main::host_error::host_error;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyBool, PyDict, PyInt, PyString};
use pyo3::{pyfunction, wrap_pyfunction};

const DATETIME_MODULE: &str = "datetime";
const JSON_MODULE: &str = "json";
use sqlbuild_state::freshness::constants::STRING_STATE_VALUE_TYPE as STRING_TYPE;
const INTEGER_TYPE: &str = "integer";
const BOOLEAN_TYPE: &str = "boolean";
const TIMESTAMP_TYPE: &str = "timestamp";
use sqlbuild_host::entries::models::ExecutorHost;

use crate::records::main::fields::field::field as native_field;
use sqlbuild_state::freshness::constants::TEXT_TIMESTAMP_STATE_VALUE_TYPE as TEXT_TIMESTAMP_TYPE;
const DATE_TYPE: &str = "date";
/// The portable SQL type state timestamps cast to unless the warehouse names a finer one.
const SQL_TIMESTAMP_TYPE: &str = "TIMESTAMP";
const JSON_TYPE: &str = "json";

/// `value` as its state column's literal, timestamps cast to `timestamp_type` when given.
#[pyfunction]
#[pyo3(signature = (*, value, declared_type, timestamp_type = None))]
pub(crate) fn render_state_sql_literal(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
    declared_type: &str,
    timestamp_type: Option<&str>,
) -> PyResult<String> {
    if value.is_none() {
        return Ok(typed_null(declared_type).to_owned());
    }
    match declared_type {
        STRING_TYPE => {
            if !value.is_instance_of::<PyString>() {
                return Err(rendering_error(
                    py,
                    "state string columns require str values",
                ));
            }
            Ok(quoted(&value.extract::<String>()?))
        }
        INTEGER_TYPE => {
            if value.is_instance_of::<PyBool>() || !value.is_instance_of::<PyInt>() {
                return Err(rendering_error(
                    py,
                    "state integer columns require int values",
                ));
            }
            let rendered: String = value.call_method1("__format__", ("",))?.extract()?;
            Ok(format!("CAST({rendered} AS BIGINT)"))
        }
        BOOLEAN_TYPE => {
            if !value.is_instance_of::<PyBool>() {
                return Err(rendering_error(
                    py,
                    "state boolean columns require bool values",
                ));
            }
            Ok(if value.is_truthy()? { "TRUE" } else { "FALSE" }.to_owned())
        }
        TIMESTAMP_TYPE | TEXT_TIMESTAMP_TYPE => {
            timestamp_literal(value, declared_type, timestamp_type)
        }
        DATE_TYPE => {
            let datetime = py.import(DATETIME_MODULE)?;
            if value.is_instance(&native_field(&datetime, "datetime")?)?
                || !value.is_instance(&native_field(&datetime, "date")?)?
            {
                return Err(rendering_error(
                    py,
                    "state date columns require date values",
                ));
            }
            let iso: String = value.call_method0("isoformat")?.extract()?;
            Ok(format!("CAST({} AS DATE)", quoted(&iso)))
        }
        JSON_TYPE => json_literal(value),
        other => Err(PyValueError::new_err(format!(
            "unknown state SQL value type {other:?}"
        ))),
    }
}

fn timestamp_literal(
    value: &Bound<'_, PyAny>,
    declared_type: &str,
    timestamp_type: Option<&str>,
) -> PyResult<String> {
    let py: Python<'_> = value.py();
    let datetime = py.import(DATETIME_MODULE)?;
    if !value.is_instance(&native_field(&datetime, "datetime")?)? {
        return Err(rendering_error(
            py,
            "state timestamp columns require datetime values",
        ));
    }
    let utc: Bound<'_, PyAny> = native_field(&datetime, "UTC")?;
    let normalized: Bound<'_, PyAny> =
        if native_field(value, "tzinfo")?.is_none() || value.call_method0("utcoffset")?.is_none() {
            let arguments: Bound<'_, PyDict> = PyDict::new(py);
            arguments.set_item("tzinfo", &utc)?;
            value.call_method("replace", (), Some(&arguments))?
        } else {
            value.call_method1("astimezone", (&utc,))?
        };
    let rendered: String = quoted(&normalized.call_method0("isoformat")?.extract::<String>()?);
    if declared_type == TEXT_TIMESTAMP_TYPE {
        return Ok(rendered);
    }
    Ok(format!(
        "CAST({rendered} AS {})",
        timestamp_type.unwrap_or(SQL_TIMESTAMP_TYPE)
    ))
}

fn json_literal(value: &Bound<'_, PyAny>) -> PyResult<String> {
    let py: Python<'_> = value.py();
    let arguments: Bound<'_, PyDict> = PyDict::new(py);
    arguments.set_item("ensure_ascii", false)?;
    arguments.set_item("separators", (",", ":"))?;
    arguments.set_item("sort_keys", true)?;
    match py
        .import(JSON_MODULE)?
        .getattr("dumps")?
        .call((value,), Some(&arguments))
    {
        Ok(serialized) => Ok(quoted(&serialized.extract::<String>()?)),
        Err(error)
            if error.is_instance_of::<PyTypeError>(py)
                || error.is_instance_of::<PyValueError>(py) =>
        {
            let raised: PyErr =
                rendering_error(py, "state JSON columns require JSON-serializable values");
            raised.set_cause(py, Some(error));
            Err(raised)
        }
        Err(error) => Err(error),
    }
}

fn quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn typed_null(declared_type: &str) -> &'static str {
    match declared_type {
        INTEGER_TYPE => "CAST(NULL AS BIGINT)",
        TIMESTAMP_TYPE => "CAST(NULL AS TIMESTAMP)",
        DATE_TYPE => "CAST(NULL AS DATE)",
        _ => "NULL",
    }
}

fn rendering_error(py: Python<'_>, message: &str) -> PyErr {
    host_error(py, ExecutorHost::SqlValueRenderingError, message)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(render_state_sql_literal, module)?)
}
