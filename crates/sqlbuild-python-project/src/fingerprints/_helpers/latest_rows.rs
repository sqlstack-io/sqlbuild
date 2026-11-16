//! Latest fingerprint rows decoded into `Fingerprint` models and indexed into a `FingerprintSet`.

use crate::fingerprints::main::row_fingerprint::row_fingerprint;
use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;
use std::ffi::CString;

use pyo3::exceptions::{PyUnicodeDecodeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyBytes, PyDict, PyString};
use sqlbuild_analysis::fingerprint_storage::errors::StorageDecodeError;
use sqlbuild_analysis::fingerprint_storage::main::decode_storage::decode_storage;
use sqlbuild_analysis::fingerprint_storage::main::latest_index::latest_index;
use sqlbuild_analysis::fingerprint_storage::models::{FingerprintIdentity, LatestIndex};
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

const DATETIME_MODULE: &str = "datetime";
const BINASCII_MODULE: &str = "binascii";
const BASE64_MODULE: &str = "base64";
pub(crate) const TIMESTAMP_COLUMN: usize = 11;

/// `FingerprintInputError(message)`.
pub(crate) fn fingerprint_input_error(py: Python<'_>, message: &str) -> PyErr {
    new_plain_error(py, ErrorId::FingerprintInput, message)
}

/// The `FingerprintSet` of fetched latest rows, a model winning a node-name collision.
pub(crate) fn fingerprint_set_from_rows<'py>(
    py: Python<'py>,
    schema: &str,
    qualified_name: &str,
    rows: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let fingerprint_class: Bound<'py, PyAny> = record_class(py, RecordId::Fingerprint)?;
    let from_iso: Bound<'py, PyAny> = py
        .import(DATETIME_MODULE)?
        .getattr("datetime")?
        .getattr("fromisoformat")?;
    let mut fingerprints: Vec<Bound<'py, PyAny>> = Vec::new();
    for row in rows.try_iter()? {
        fingerprints.push(row_fingerprint(
            &fingerprint_class,
            &from_iso,
            &row?,
            qualified_name,
        )?);
    }
    indexed_set(py, schema, &fingerprints)
}

pub(crate) fn indexed_set<'py>(
    py: Python<'py>,
    schema: &str,
    fingerprints: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    let identities: Vec<(String, String)> = fingerprints
        .iter()
        .map(identity_of)
        .collect::<PyResult<_>>()?;
    let index: LatestIndex = latest_index(&borrowed(&identities));
    let by_name: Bound<'py, PyDict> = PyDict::new(py);
    for position in index.by_name {
        by_name.set_item(&identities[position].1, &fingerprints[position])?;
    }
    let by_identity: Bound<'py, PyDict> = PyDict::new(py);
    for position in index.by_identity {
        by_identity.set_item(identities[position].clone(), &fingerprints[position])?;
    }
    let kwargs: Bound<'py, PyDict> = PyDict::new(py);
    kwargs.set_item("schema", schema)?;
    kwargs.set_item("fingerprints", by_name)?;
    kwargs.set_item("fingerprints_by_identity", by_identity)?;
    new_record(py, RecordId::FingerprintSet, (), Some(&kwargs))
}

pub(crate) fn identity_of(fingerprint: &Bound<'_, PyAny>) -> PyResult<(String, String)> {
    Ok((
        native_field(fingerprint, "node_type")?.extract()?,
        native_field(fingerprint, "node_name")?.extract()?,
    ))
}

pub(crate) fn borrowed(identities: &[(String, String)]) -> Vec<FingerprintIdentity<'_>> {
    identities
        .iter()
        .map(|(node_type, node_name)| FingerprintIdentity {
            node_type,
            node_name,
        })
        .collect()
}

pub(crate) fn optional_text<'py>(
    value: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyString>>> {
    if value.is_none() {
        return Ok(None);
    }
    value.str().map(Some)
}

/// Base64 storage as text, raising Python's errors for non-ASCII or invalid storage.
pub(crate) fn decoded(
    storage: &Bound<'_, PyString>,
    node_name: &Bound<'_, PyString>,
    qualified_name: &str,
) -> PyResult<String> {
    let py: Python<'_> = storage.py();
    let text: String = storage.extract()?;
    if !text.is_ascii() {
        let _ = storage.call_method1("encode", ("ascii",))?;
        return Err(PyValueError::new_err(
            "non-ASCII fingerprint storage encoded as ASCII",
        ));
    }
    let failure: StorageDecodeError = match decode_storage(&text) {
        Ok(decoded) => return Ok(decoded),
        Err(failure) => failure,
    };
    let cause: PyErr = decode_error(storage, failure)?;
    let name: String = node_name.extract()?;
    let raised: PyErr = fingerprint_input_error(
        py,
        &format!(
            "Invalid fingerprint definition storage for '{name}' in {qualified_name}: expected \
             base64-encoded UTF-8. This can happen after upgrading from an older sqlbuild \
             version; delete or rebuild {qualified_name} to regenerate fingerprints."
        ),
    );
    raised.value(py).setattr("__context__", cause.value(py))?;
    raised.set_cause(py, Some(cause));
    Err(raised)
}

/// The interpreter's own decoder error, whose message varies by Python version, else ours.
fn decode_error(storage: &Bound<'_, PyString>, failure: StorageDecodeError) -> PyResult<PyErr> {
    let py: Python<'_> = storage.py();
    match failure {
        StorageDecodeError::Base64(message) => {
            let kwargs: Bound<'_, PyDict> = PyDict::new(py);
            kwargs.set_item("validate", true)?;
            let decoded = py.import(BASE64_MODULE)?.getattr("b64decode")?.call(
                (storage.call_method1("encode", ("ascii",))?,),
                Some(&kwargs),
            );
            match decoded {
                Err(error) => Ok(error),
                Ok(_) => Ok(PyErr::from_value(
                    py.import(BINASCII_MODULE)?
                        .getattr("Error")?
                        .call1((message,))?,
                )),
            }
        }
        StorageDecodeError::Utf8 {
            bytes,
            start,
            end,
            reason,
        } => {
            if let Err(error) = PyBytes::new(py, &bytes).call_method1("decode", ("utf-8",)) {
                return Ok(error);
            }
            let reason: CString =
                CString::new(reason).map_err(|error| PyValueError::new_err(error.to_string()))?;
            Ok(PyErr::from_value(
                PyUnicodeDecodeError::new(py, c"utf-8", &bytes, start..end, &reason)?.into_any(),
            ))
        }
    }
}
