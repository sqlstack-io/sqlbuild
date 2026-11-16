//! Decoded integration-result records, read by the integrations that consume them.

use pyo3::exceptions::PyAttributeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyStringMethods, PyTuple};
use pyo3::{pyclass, pymethods};
use sqlbuild_core::integration_results::errors::IntegrationRecordError;
use sqlbuild_core::integration_results::main::encode_integration_value::encode_integration_value;
use sqlbuild_core::integration_results::main::integration_record_fits::integration_record_fits;
use sqlbuild_core::integration_results::main::normalize_integration_record::normalize_integration_record;
use sqlbuild_core::integration_results::main::strip_optional_enrichment::strip_optional_enrichment;
use sqlbuild_core::json::models::JsonValue;

use crate::bindings::_helpers::scopes::scope_report_objects::json_to_python;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::result_output::main::json_value::json_value;
use sqlbuild_python_base::result_output::main::validation_error::validation_error;

/// One decoded part of a record: its fields as attributes, and as a dictionary.
fn field(fields: &Py<PyDict>, py: Python<'_>, name: &str) -> PyResult<Py<PyAny>> {
    match fields.bind(py).get_item(name)? {
        Some(value) => Ok(value.unbind()),
        None => Err(PyAttributeError::new_err(name.to_owned())),
    }
}

fn copied(fields: &Py<PyDict>, py: Python<'_>) -> PyResult<Py<PyDict>> {
    Ok(fields.bind(py).copy()?.unbind())
}

/// Asset enrichment of one integration result.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct IntegrationAssetResult {
    fields: Py<PyDict>,
}

/// Check enrichment of one integration result.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct IntegrationCheckResult {
    fields: Py<PyDict>,
}

/// One validated integration-result record.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct IntegrationResultEnvelope {
    record: JsonValue,
    fields: Py<PyDict>,
    asset: Option<Py<IntegrationAssetResult>>,
    checks: Py<PyTuple>,
}

#[pymethods]
impl IntegrationAssetResult {
    pub(crate) fn __getattr__(&self, py: Python<'_>, name: &str) -> PyResult<Py<PyAny>> {
        field(&self.fields, py, name)
    }

    /// Every field, as `dataclasses.asdict` gave it.
    fn as_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        copied(&self.fields, py)
    }
}

#[pymethods]
impl IntegrationCheckResult {
    pub(crate) fn __getattr__(&self, py: Python<'_>, name: &str) -> PyResult<Py<PyAny>> {
        field(&self.fields, py, name)
    }

    /// Every field, as `dataclasses.asdict` gave it.
    fn as_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        copied(&self.fields, py)
    }
}

fn parsed(py: Python<'_>, line: &str) -> PyResult<Result<JsonValue, IntegrationRecordError>> {
    let json: Bound<'_, PyModule> = py.import("json")?;
    let payload: Bound<'_, PyAny> = match json.call_method1("loads", (line,)) {
        Ok(payload) => payload,
        Err(error) if error.is_instance(py, &native_field(&json, "JSONDecodeError")?) => {
            return Ok(Err(IntegrationRecordError {
                message: "malformed integration result JSON".to_owned(),
            }));
        }
        Err(error) => return Err(error),
    };
    Ok(json_value(&payload)?.ok_or_else(|| IntegrationRecordError {
        message: "integration result must be a JSON object".to_owned(),
    }))
}

fn decoded(py: Python<'_>, record: JsonValue) -> PyResult<IntegrationResultEnvelope> {
    let normalized: Vec<(String, JsonValue)> =
        normalize_integration_record(record).map_err(|error| validation_error(py, &error))?;
    let fields: Bound<'_, PyDict> = PyDict::new(py);
    let mut asset: Option<Py<IntegrationAssetResult>> = None;
    let mut checks: Vec<Py<IntegrationCheckResult>> = Vec::new();
    for (key, value) in normalized.iter().cloned() {
        match (key.as_str(), value) {
            ("asset", JsonValue::Object(entries)) => {
                let fields: Py<PyAny> = json_to_python(py, JsonValue::Object(entries), false)?;
                asset = Some(Py::new(
                    py,
                    IntegrationAssetResult {
                        fields: fields.extract(py)?,
                    },
                )?);
            }
            ("asset", _) => {}
            ("checks", JsonValue::Array(items)) => {
                for item in items {
                    let fields: Py<PyAny> = json_to_python(py, item, false)?;
                    checks.push(Py::new(
                        py,
                        IntegrationCheckResult {
                            fields: fields.extract(py)?,
                        },
                    )?);
                }
            }
            (_, value) => fields.set_item(key, json_to_python(py, value, false)?)?,
        }
    }
    Ok(IntegrationResultEnvelope {
        record: JsonValue::Object(normalized),
        fields: fields.unbind(),
        asset,
        checks: PyTuple::new(py, checks)?.unbind(),
    })
}

#[pymethods]
impl IntegrationResultEnvelope {
    /// Decode and validate one integration result JSON record.
    #[staticmethod]
    fn from_json(py: Python<'_>, value: &Bound<'_, PyString>) -> PyResult<Self> {
        let line: String = value.to_string_lossy().into_owned();
        if !integration_record_fits(line.len()) {
            return Err(validation_error(
                py,
                &IntegrationRecordError {
                    message: "integration result record exceeds size limit".to_owned(),
                },
            ));
        }
        let record: JsonValue = parsed(py, &line)?.map_err(|error| validation_error(py, &error))?;
        decoded(py, record)
    }

    /// Decode a record after dropping forward-compatible optional enrichment.
    #[staticmethod]
    fn from_json_without_optional_enrichment(
        py: Python<'_>,
        value: &Bound<'_, PyString>,
    ) -> PyResult<Self> {
        let line: String = value.to_string_lossy().into_owned();
        let record: JsonValue = parsed(py, &line)?.map_err(|error| validation_error(py, &error))?;
        if !matches!(record, JsonValue::Object(_)) {
            return Err(validation_error(
                py,
                &IntegrationRecordError {
                    message: "integration result must be a JSON object".to_owned(),
                },
            ));
        }
        decoded(py, strip_optional_enrichment(record))
    }

    /// Encode the validated record as one JSON Lines record.
    fn to_json(&self, py: Python<'_>) -> PyResult<String> {
        encode_integration_value(&self.record, true)
            .map(|line| line + "\n")
            .map_err(|error| validation_error(py, &error))
    }

    #[getter]
    pub(crate) fn asset(&self, py: Python<'_>) -> Option<Py<IntegrationAssetResult>> {
        self.asset.as_ref().map(|asset| asset.clone_ref(py))
    }

    #[getter]
    pub(crate) fn checks(&self, py: Python<'_>) -> Py<PyTuple> {
        self.checks.clone_ref(py)
    }

    pub(crate) fn __getattr__(&self, py: Python<'_>, name: &str) -> PyResult<Py<PyAny>> {
        field(&self.fields, py, name)
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<IntegrationAssetResult>()?;
    module.add_class::<IntegrationCheckResult>()?;
    module.add_class::<IntegrationResultEnvelope>()
}
