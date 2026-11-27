//! A compiled project's loads, kept as Rust facts and read back as `CompiledLoad` records.

use std::path::PathBuf;

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyString, PyTuple};
use sqlbuild_python_base::boundary::main::json_value_to_python::json_value_to_python;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

use crate::compiled_project::main::plain_value::plain_value;

/// One load the project compiles, as its `CompiledLoad` record carries it.
#[derive(Clone, Debug)]
pub(crate) struct LoadFact {
    pub(crate) name: String,
    pub(crate) relative_path: PathBuf,
    pub(crate) settings: serde_json::Value,
    pub(crate) sources: Vec<String>,
    pub(crate) fingerprint: String,
}

/// The facts of each `CompiledLoad` record in `records`, in order.
pub(crate) fn load_facts(records: &Bound<'_, PyAny>) -> PyResult<Vec<LoadFact>> {
    records
        .try_iter()?
        .map(|record| {
            let record: Bound<'_, PyAny> = record?;
            Ok(LoadFact {
                name: native_field(&record, "name")?.extract()?,
                relative_path: native_field(&record, "relative_path")?.extract()?,
                settings: plain_value(&native_field(&record, "settings")?)?,
                sources: native_field(&record, "sources")?.extract()?,
                fingerprint: native_field(&record, "fingerprint")?.extract()?,
            })
        })
        .collect()
}

/// A `CompiledLoad` record for each of `loads`, in order.
pub(crate) fn load_records<'py>(
    py: Python<'py>,
    loads: &[LoadFact],
) -> PyResult<Bound<'py, PyTuple>> {
    let mut records: Vec<Bound<'py, PyAny>> = Vec::with_capacity(loads.len());
    for load in loads {
        let name: Bound<'py, PyAny> = PyString::new(py, &load.name).into_any();
        let key: Bound<'py, PyAny> = new_record_with(
            py,
            RecordId::CompiledObjectKey,
            &[
                (
                    "resource_type",
                    enum_member(py, RecordId::CompiledResourceType, "LOAD")?,
                ),
                ("name", name.clone()),
            ],
        )?;
        records.push(new_record_with(
            py,
            RecordId::CompiledLoad,
            &[
                ("key", key),
                ("name", name),
                (
                    "relative_path",
                    (&load.relative_path).into_pyobject(py)?.into_any(),
                ),
                ("settings", json_value_to_python(py, &load.settings)?),
                ("sources", PyTuple::new(py, &load.sources)?.into_any()),
                (
                    "fingerprint",
                    PyString::new(py, &load.fingerprint).into_any(),
                ),
            ],
        )?);
    }
    PyTuple::new(py, records)
}
