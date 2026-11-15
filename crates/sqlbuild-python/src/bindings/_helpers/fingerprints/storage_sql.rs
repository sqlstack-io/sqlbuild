//! Fingerprint insert SQL, one row or bounded batches of rows.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_analysis::fingerprint_storage::errors::FingerprintStorageError;
use sqlbuild_analysis::fingerprint_storage::main::insert_batch_lengths::insert_batch_lengths;
use sqlbuild_analysis::fingerprint_storage::main::insert_sql::insert_sql;
use sqlbuild_analysis::fingerprint_storage::models::StoredFingerprint;

use crate::bindings::_helpers::fingerprints::latest_rows::fingerprint_input_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// One `INSERT` of a single fingerprint.
pub(crate) fn fingerprint_insert_sql(
    py: Python<'_>,
    qualified_name: &str,
    fingerprint: &Bound<'_, PyAny>,
) -> PyResult<String> {
    insert_sql(qualified_name, &[stored_fingerprint(fingerprint)?])
        .map_err(|error| storage_error(py, error))
}

/// `(batch, insert_sql)` per bounded batch, given `(batch_size, batch_max_bytes)`.
pub(crate) fn fingerprint_insert_batches<'py>(
    py: Python<'py>,
    qualified_name: &str,
    fingerprints: Vec<Bound<'py, PyAny>>,
    limits: (i64, i64),
) -> PyResult<Vec<(Bound<'py, PyTuple>, String)>> {
    let stored: Vec<StoredFingerprint> = fingerprints
        .iter()
        .map(stored_fingerprint)
        .collect::<PyResult<_>>()?;
    let lengths: Vec<usize> = insert_batch_lengths(qualified_name, &stored, limits.0, limits.1)
        .map_err(|error| storage_error(py, error))?;
    let mut batches: Vec<(Bound<'py, PyTuple>, String)> = Vec::with_capacity(lengths.len());
    let mut start: usize = 0;
    for length in lengths {
        let end: usize = start + length;
        let sql: String = insert_sql(qualified_name, &stored[start..end])
            .map_err(|error| storage_error(py, error))?;
        batches.push((PyTuple::new(py, &fingerprints[start..end])?, sql));
        start = end;
    }
    Ok(batches)
}

fn stored_fingerprint(fingerprint: &Bound<'_, PyAny>) -> PyResult<StoredFingerprint> {
    Ok(StoredFingerprint {
        node_type: native_field(fingerprint, "node_type")?.extract()?,
        node_name: native_field(fingerprint, "node_name")?.extract()?,
        target_database: native_field(fingerprint, "target_database")?.extract()?,
        target_schema: native_field(fingerprint, "target_schema")?.extract()?,
        target_name: native_field(fingerprint, "target_name")?.extract()?,
        run_id: formatted(fingerprint, "run_id")?,
        definition_hash: formatted(fingerprint, "definition_hash")?,
        version_hash: formatted(fingerprint, "version_hash")?,
        schema_fingerprint: formatted(fingerprint, "schema_fingerprint")?,
        definition: native_field(fingerprint, "definition")?.extract()?,
        metadata_json: native_field(fingerprint, "metadata_json")?.extract()?,
        ts: native_field(fingerprint, "ts")?
            .call_method0("isoformat")?
            .extract()?,
    })
}

fn formatted(fingerprint: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    native_field(fingerprint, name)?.str()?.extract()
}

fn storage_error(py: Python<'_>, error: FingerprintStorageError) -> PyErr {
    fingerprint_input_error(py, &error.to_string())
}
