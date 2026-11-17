//! Write execution JSON to stdout or a requested side-channel file.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Write execution JSON to stdout or a requested side-channel file.
#[pyfunction]
#[pyo3(signature = (*, payload, json_output, json_output_path))]
pub fn write_execution_json_output(
    py: Python<'_>,
    payload: Bound<'_, PyAny>,
    json_output: bool,
    json_output_path: Bound<'_, PyAny>,
) -> PyResult<()> {
    if !json_output_path.is_none() {
        let keywords: Bound<'_, PyDict> = PyDict::new(py);
        keywords.set_item("parents", true)?;
        keywords.set_item("exist_ok", true)?;
        native_field(&json_output_path, "parent")?.call_method("mkdir", (), Some(&keywords))?;
        let encoding: Bound<'_, PyDict> = PyDict::new(py);
        encoding.set_item("encoding", "utf-8")?;
        json_output_path.call_method("write_text", (payload,), Some(&encoding))?;
        return Ok(());
    }
    if json_output {
        let stdout: Bound<'_, PyAny> = py.import("sys")?.getattr("stdout")?;
        stdout.call_method1("write", (payload,))?;
        stdout.call_method0("flush")?;
    }
    Ok(())
}
