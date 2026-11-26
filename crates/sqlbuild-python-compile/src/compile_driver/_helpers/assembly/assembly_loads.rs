//! The loads a project compiles, one per sources file that declares one.

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::{PyString, PyTuple};
use sqlbuild_load::loads::main::load_fingerprint::load_fingerprint;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::compile_driver::main::discovered_loads::discovered_loads;

/// A `CompiledLoad` for each load the discovered source files declare, in file order.
pub(crate) fn compiled_loads<'py>(discovered: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = discovered.py();
    let mut loads: Vec<Bound<'py, PyAny>> = Vec::new();
    for (file, load, declaration) in discovered_loads(&native_field(discovered, "source_files")?)? {
        let name: Bound<'py, PyAny> = PyString::new(py, &declaration.name).into_any();
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
        let sources: Vec<&str> = declaration
            .sources
            .iter()
            .map(|source| source.name.as_str())
            .collect();
        loads.push(new_record_with(
            py,
            RecordId::CompiledLoad,
            &[
                ("key", key),
                ("name", name),
                ("relative_path", native_field(&file, "relative_path")?),
                ("settings", native_field(&load, "settings")?),
                ("sources", PyTuple::new(py, sources)?.into_any()),
                (
                    "fingerprint",
                    PyString::new(py, &load_fingerprint(&declaration)).into_any(),
                ),
            ],
        )?);
    }
    PyTuple::new(py, loads)
}
