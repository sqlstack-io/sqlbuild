//! Report a scope index's error findings as individual compile diagnostics.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use sqlbuild_python_base::compile_context::main::diagnostic_collector::report_compile_diagnostic::report_compile_diagnostic;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

const ERROR_SEVERITY: &str = "error";

/// `report_scope_index_errors`: each error-severity finding of `index`, keyed for the collector.
pub(crate) fn report_scope_index_errors(index: &Bound<'_, PyAny>) -> PyResult<()> {
    let py: Python<'_> = index.py();
    let path_class = py.import("pathlib")?.getattr("Path")?;
    for item in native_field(index, "diagnostics")?.try_iter()? {
        let item: Bound<'_, PyAny> = item?;
        if native_field(&item, "severity")?
            .str()?
            .extract::<String>()?
            != ERROR_SEVERITY
        {
            continue;
        }
        let code: Bound<'_, PyAny> = native_field(&native_field(&item, "code")?, "value")?;
        let message: Bound<'_, PyAny> = native_field(&item, "message")?;
        let path: Bound<'_, PyAny> = native_field(&item, "path")?;
        let declaration: Bound<'_, PyAny> = native_field(&item, "declaration")?;
        let resource: Bound<'_, PyAny> = native_field(&item, "resource")?;
        let key: Bound<'_, PyTuple> = PyTuple::new(
            py,
            [
                code.clone(),
                or_empty(&path)?,
                or_empty(&declaration)?.str()?.into_any(),
                or_empty(&resource)?.str()?.into_any(),
                message.clone(),
            ],
        )?;
        let (resource_type, resource_name) = owning_resource(&resource, &declaration)?;
        let fields: Bound<'_, PyDict> = PyDict::new(py);
        fields.set_item("phase", "compile")?;
        fields.set_item("severity", ERROR_SEVERITY)?;
        fields.set_item("code", code)?;
        fields.set_item("message", message)?;
        fields.set_item("resource_type", resource_type)?;
        fields.set_item("resource_name", resource_name)?;
        if path.is_none() {
            fields.set_item("path", py.None())?;
        } else {
            fields.set_item("path", path_class.call1((path,))?)?;
        }
        fields.set_item("line", native_field(&item, "line")?)?;
        fields.set_item("column", native_field(&item, "column")?)?;
        let diagnostic: Bound<'_, PyAny> =
            new_record(py, RecordId::CompilerDiagnostic, (), Some(&fields))?;
        report_compile_diagnostic(key.as_any(), &diagnostic)?;
    }
    Ok(())
}

/// `value or ""`.
fn or_empty<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if value.is_truthy()? {
        return Ok(value.clone());
    }
    Ok(PyString::new(value.py(), "").into_any())
}

/// The compiled resource a finding names: its resource, else an audit declaration.
fn owning_resource(
    resource: &Bound<'_, PyAny>,
    declaration: &Bound<'_, PyAny>,
) -> PyResult<(Option<&'static str>, Option<String>)> {
    if !resource.is_none() {
        let kind: String = native_field(resource, "kind")?.str()?.extract()?;
        let resource_type: Option<&'static str> = match kind.as_str() {
            "model" => Some("model"),
            "source" => Some("source"),
            "seed" => Some("seed"),
            "function" => Some("udf"),
            "test" => Some("sql_test"),
            "scenario" => Some("sql_scenario"),
            _ => None,
        };
        if resource_type.is_some() {
            return Ok((
                resource_type,
                Some(native_field(resource, "name")?.extract()?),
            ));
        }
    }
    if !declaration.is_none() {
        let kind: String = native_field(declaration, "kind")?.str()?.extract()?;
        if matches!(kind.as_str(), "audit" | "singular_audit") {
            return Ok((
                Some("audit"),
                Some(native_field(declaration, "name")?.extract()?),
            ));
        }
    }
    Ok((None, None))
}
