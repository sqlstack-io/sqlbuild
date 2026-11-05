//! Latest fingerprint rows decoded into `Fingerprint` models and indexed into a `FingerprintSet`.

use std::ffi::CString;

use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::construct::{new_record, record_class};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_values;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};
use pyo3::exceptions::{PyUnicodeDecodeError, PyValueError};
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyBytes, PyDateTime, PyDict, PyString};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::fingerprint_storage::errors::StorageDecodeError;
use sqlbuild_analysis::fingerprint_storage::main::decode_storage::decode_storage;
use sqlbuild_analysis::fingerprint_storage::main::latest_index::latest_index;
use sqlbuild_analysis::fingerprint_storage::main::selected_positions::selected_positions;
use sqlbuild_analysis::fingerprint_storage::models::{FingerprintIdentity, LatestIndex};

const DATETIME_MODULE: &str = "datetime";
const BINASCII_MODULE: &str = "binascii";
const BASE64_MODULE: &str = "base64";
const TIMESTAMP_COLUMN: usize = 11;

/// `FingerprintInputError(message)`.
pub(crate) fn fingerprint_input_error(py: Python<'_>, message: &str) -> PyErr {
    match error_class(py, ErrorId::FingerprintInput).and_then(|class| class.call1((message,))) {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
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

/// `select_latest_fingerprints`: an unfiltered latest read narrowed as a filtered read would be.
#[pyfunction]
#[pyo3(signature = (*, fingerprint_set, node_names, filtered_node_types = Vec::new()))]
pub(crate) fn select_latest_fingerprints<'py>(
    py: Python<'py>,
    fingerprint_set: &Bound<'py, PyAny>,
    node_names: Vec<String>,
    filtered_node_types: Vec<String>,
) -> PyResult<Bound<'py, PyAny>> {
    let schema: String = native_field(fingerprint_set, "schema")?.extract()?;
    if node_names.is_empty() && filtered_node_types.is_empty() {
        return indexed_set(py, &schema, &[]);
    }
    let by_identity: Bound<'py, PyAny> = native_field(fingerprint_set, "fingerprints_by_identity")?;
    let latest: Vec<Bound<'py, PyAny>> = if by_identity.is_truthy()? {
        mapping_values(&by_identity)?
            .try_iter()?
            .collect::<PyResult<_>>()?
    } else {
        Vec::new()
    };
    let identities: Vec<(String, String)> =
        latest.iter().map(identity_of).collect::<PyResult<_>>()?;
    let positions: Vec<usize> =
        selected_positions(&borrowed(&identities), &node_names, &filtered_node_types);
    let selected: Vec<Bound<'py, PyAny>> = positions
        .into_iter()
        .map(|position| latest[position].clone())
        .collect();
    indexed_set(py, &schema, &selected)
}

fn indexed_set<'py>(
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

fn identity_of(fingerprint: &Bound<'_, PyAny>) -> PyResult<(String, String)> {
    Ok((
        native_field(fingerprint, "node_type")?.extract()?,
        native_field(fingerprint, "node_name")?.extract()?,
    ))
}

fn borrowed(identities: &[(String, String)]) -> Vec<FingerprintIdentity<'_>> {
    identities
        .iter()
        .map(|(node_type, node_name)| FingerprintIdentity {
            node_type,
            node_name,
        })
        .collect()
}

pub(crate) fn row_fingerprint<'py>(
    fingerprint_class: &Bound<'py, PyAny>,
    from_iso: &Bound<'py, PyAny>,
    row: &Bound<'py, PyAny>,
    qualified_name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = row.py();
    let raw_ts: Bound<'py, PyAny> = row.get_item(TIMESTAMP_COLUMN)?;
    let ts: Bound<'py, PyAny> = if raw_ts.is_instance_of::<PyDateTime>() {
        raw_ts
    } else {
        from_iso.call1((raw_ts.str()?,))?
    };
    let node_type: Bound<'py, PyString> = row.get_item(0)?.str()?;
    let node_name: Bound<'py, PyString> = row.get_item(1)?.str()?;
    let raw_target_database: Bound<'py, PyAny> = row.get_item(2)?;
    let raw_target_schema: Bound<'py, PyAny> = row.get_item(3)?;
    let raw_target_name: Bound<'py, PyAny> = row.get_item(4)?;
    let definition_storage: Bound<'py, PyString> = row.get_item(9)?.str()?;
    let metadata_json_storage: Bound<'py, PyString> = row.get_item(10)?.str()?;
    let definition: String = decoded(&definition_storage, &node_name, qualified_name)?;
    let metadata_json: String = decoded(&metadata_json_storage, &node_name, qualified_name)?;
    let kwargs: Bound<'py, PyDict> = PyDict::new(py);
    kwargs.set_item("node_type", node_type)?;
    kwargs.set_item("node_name", node_name)?;
    kwargs.set_item("target_database", optional_text(&raw_target_database)?)?;
    kwargs.set_item("target_schema", optional_text(&raw_target_schema)?)?;
    kwargs.set_item("target_name", optional_text(&raw_target_name)?)?;
    kwargs.set_item("run_id", row.get_item(5)?.str()?)?;
    kwargs.set_item("definition_hash", row.get_item(6)?.str()?)?;
    kwargs.set_item("version_hash", row.get_item(7)?.str()?)?;
    kwargs.set_item("schema_fingerprint", row.get_item(8)?.str()?)?;
    kwargs.set_item("definition", definition)?;
    kwargs.set_item("metadata_json", metadata_json)?;
    kwargs.set_item("ts", ts)?;
    fingerprint_class.call((), Some(&kwargs))
}

fn optional_text<'py>(value: &Bound<'py, PyAny>) -> PyResult<Option<Bound<'py, PyString>>> {
    if value.is_none() {
        return Ok(None);
    }
    value.str().map(Some)
}

/// Base64 storage as text, raising Python's errors for non-ASCII or invalid storage.
fn decoded(
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(select_latest_fingerprints, module)?)?;
    Ok(())
}
