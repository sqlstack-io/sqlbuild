//! Migration identity binding with unchanged metadata and hash format.

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::native_host::model_lookup::ModelLookup;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::replaced_keywords;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::planner_migrations::main::{migration_fingerprint, migration_ref_names};
use std::collections::BTreeMap;

/// Hash normalized SQL and non-storage metadata through rename identities.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn build_migration_fingerprint(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<Option<String>> {
    let arguments = KeywordArguments::read(
        py,
        "build_migration_fingerprint",
        options,
        (
            &["query_sql", "metadata_json", "ref_identities", "dialect"],
            &[],
        ),
    )?;
    let mut identities: BTreeMap<String, String> = BTreeMap::new();
    for item in arguments
        .value("ref_identities")?
        .call_method0("items")?
        .try_iter()?
    {
        let (key, value): (String, String) = item?.extract()?;
        identities.insert(key, value);
    }
    let dialect: Option<String> = arguments.value("dialect")?.extract()?;
    fingerprint(
        &arguments.value("query_sql")?.extract::<String>()?,
        &arguments.value("metadata_json")?.extract::<String>()?,
        &identities,
        dialect.as_deref(),
    )
}

/// The migration fingerprint of SQL and metadata through rename identities, `generic` by default.
pub(crate) fn fingerprint(
    sql: &str,
    metadata: &str,
    identities: &BTreeMap<String, String>,
    dialect: Option<&str>,
) -> PyResult<Option<String>> {
    migration_fingerprint::migration_fingerprint(
        sql,
        metadata,
        identities,
        dialect
            .filter(|value| !value.is_empty())
            .unwrap_or("generic"),
    )
    .map_err(PyValueError::new_err)
}

/// Every name whose rename identity can change the migration fingerprint.
pub(crate) fn migration_fingerprint_ref_names<'py>(
    py: Python<'py>,
    query_sql: &str,
    metadata_json: &str,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    PyFrozenSet::new(
        py,
        migration_ref_names::migration_ref_names(query_sql, metadata_json)
            .map_err(PyValueError::new_err)?,
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_migration_fingerprint, module)?)?;
    Ok(())
}

/// Attach persisted migration identities through the existing cache boundary.
pub(crate) fn with_migration_fingerprints<'py>(
    entries: &Bound<'py, PyAny>,
    models: &ModelLookup,
    cache: &Bound<'py, PyAny>,
    dialect: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    use crate::bindings::_helpers::boundary::python_calls::call_with;
    let py = entries.py();
    let mut attached: Vec<Bound<'py, pyo3::PyAny>> = Vec::new();
    for entry in entries.try_iter()? {
        let entry = entry?;
        let model = models.get(py, &native_field(&entry, "name")?.extract::<String>()?)?;
        let metadata = native_field(&entry, "fingerprint_metadata_json")?;
        if model.is_none() || metadata.is_none() || native_field(&entry, "action")?.eq("skip")? {
            attached.push(entry);
            continue;
        }
        let fingerprint = call_with(
            &native_field(cache, "fingerprint")?,
            &[
                ("query_sql", model_reads::query_sql(&model)?),
                ("metadata_json", metadata),
                ("ref_identities", PyDict::new(py).into_any()),
                ("dialect", dialect.clone()),
            ],
        )?;
        let kwargs = PyDict::new(py);
        kwargs.set_item("migration_fingerprint", fingerprint)?;
        attached.push(replaced_keywords(&entry, &kwargs)?);
    }
    cache.call_method0("persist")?;
    PyTuple::new(py, attached)
}
