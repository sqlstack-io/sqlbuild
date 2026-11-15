//! Version-identity metadata helpers the remaining Python planner reads.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::{IntoPyObject, pyfunction, wrap_pyfunction};

use sqlbuild_analysis::planner_changes::main::version_identity_payload::version_identity_payload;
use sqlbuild_analysis::planner_changes::main::with_declared_columns_hash::with_declared_columns_hash as with_hash;
use sqlbuild_analysis::planner_changes::main::without_declared_columns_hash::without_declared_columns_hash as without_hash;
use sqlbuild_analysis::planner_changes::models::IdentityPayload;

use sqlbuild_python_base::boundary::main::imported::imported;

/// The decoded metadata that participates in version identity, the raw text when it is not JSON, or `None`.
#[pyfunction]
#[pyo3(signature = (metadata_json))]
fn version_identity_metadata_payload<'py>(
    py: Python<'py>,
    metadata_json: Option<&str>,
) -> PyResult<Bound<'py, PyAny>> {
    match version_identity_payload(metadata_json, false) {
        IdentityPayload::Absent => Ok(py.None().into_bound(py)),
        IdentityPayload::Raw(text) => Ok(text.into_pyobject(py)?.into_any()),
        IdentityPayload::Object(object) => {
            imported(py, "json", "loads")?.call1((serde_json::Value::Object(object).to_string(),))
        }
    }
}

/// Fingerprint metadata JSON that also records the model's declared-columns hash.
#[pyfunction]
#[pyo3(signature = (*, metadata_json, declared_columns_hash))]
fn with_declared_columns_hash(metadata_json: &str, declared_columns_hash: &str) -> String {
    with_hash(metadata_json, declared_columns_hash)
}

/// Fingerprint metadata JSON without the declared-columns hash key, if present.
#[pyfunction]
pub(crate) fn without_declared_columns_hash(metadata_json: &str) -> String {
    without_hash(metadata_json)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(version_identity_metadata_payload, module)?)?;
    module.add_function(wrap_pyfunction!(with_declared_columns_hash, module)?)?;
    module.add_function(wrap_pyfunction!(without_declared_columns_hash, module)?)
}
