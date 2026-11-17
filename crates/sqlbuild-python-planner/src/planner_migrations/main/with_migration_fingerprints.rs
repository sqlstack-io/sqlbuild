//! Attach persisted migration identities through the existing cache boundary.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;
use sqlbuild_python_project::compiled_views::main::compiled_models::query_sql::query_sql;
use sqlbuild_python_project::native_host::models::ModelLookup;

/// Attach persisted migration identities through the existing cache boundary.
pub fn with_migration_fingerprints<'py>(
    entries: &Bound<'py, PyAny>,
    models: &ModelLookup,
    cache: &Bound<'py, PyAny>,
    dialect: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    use sqlbuild_python_base::boundary::main::call_with::call_with;
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
                ("query_sql", query_sql(&model)?),
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
