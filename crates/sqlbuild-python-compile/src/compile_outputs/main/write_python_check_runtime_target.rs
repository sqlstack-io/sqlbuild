//! Write Python check results under `target/run/checks` when any ran.

use crate::compile_outputs::_helpers::runtime_target::CHECKS_DIR;
use crate::compile_outputs::_helpers::runtime_target::JSON_INDENT;
use crate::compile_outputs::_helpers::runtime_target::PYTHON_CHECK_KIND;
use crate::compile_outputs::_helpers::runtime_target::PYTHON_CHECKS_FILE;
use crate::compile_outputs::_helpers::runtime_target::RUN_DIR;
use crate::compile_outputs::_helpers::runtime_target::check_status;
use crate::compile_outputs::_helpers::runtime_target::write_text;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods};
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Write Python check results under `target/run/checks` when any ran.
pub fn write_python_check_runtime_target(
    target_dir: &Bound<'_, PyAny>,
    results: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py: Python<'_> = target_dir.py();
    let checks: Bound<'_, PyList> = PyList::empty(py);
    for result in results.try_iter()? {
        let result: Bound<'_, PyAny> = result?;
        let name: Bound<'_, PyAny> = native_field(&result, "node_name")?;
        let check: Bound<'_, PyDict> = PyDict::new(py);
        check.set_item("kind", PYTHON_CHECK_KIND)?;
        check.set_item("name", &name)?;
        check.set_item("display_name", &name)?;
        check.set_item("check_id", format!("{PYTHON_CHECK_KIND}:{}", name.str()?))?;
        check.set_item("status", check_status(&result)?)?;
        check.set_item("passed", native_field(&result, "passed")?)?;
        check.set_item(
            "severity",
            native_field(&result, "severity")?.getattr("value")?,
        )?;
        check.set_item("message", native_field(&result, "message")?)?;
        check.set_item("error_message", native_field(&result, "error_message")?)?;
        check.set_item("metadata", native_field(&result, "metadata")?)?;
        checks.append(check)?;
    }
    if checks.is_empty() {
        return Ok(());
    }
    let payload: Bound<'_, PyDict> = PyDict::new(py);
    payload.set_item("checks", checks)?;
    let options: Bound<'_, PyDict> = PyDict::new(py);
    options.set_item("indent", JSON_INDENT)?;
    let text: String = imported(py, "json", "dumps")?
        .call((payload,), Some(&options))?
        .extract()?;
    write_text(
        &target_dir
            .div(RUN_DIR)?
            .div(CHECKS_DIR)?
            .div(PYTHON_CHECKS_FILE)?,
        &format!("{text}\n"),
    )
}
