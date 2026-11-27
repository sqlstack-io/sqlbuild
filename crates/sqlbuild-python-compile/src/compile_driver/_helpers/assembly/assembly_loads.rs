//! The loads a project compiles, one per sources file that declares one.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::{PyString, PyTuple};
use sqlbuild_load::loads::main::load_fingerprint::load_fingerprint;
use sqlbuild_load::loads::main::load_settings::load_settings;
use sqlbuild_load::loads::models::LoadDeclaration;
use sqlbuild_python_base::boundary::main::json_value_to_python::json_value_to_python;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::compile_driver::main::discovered_loads::discovered_loads;
use sqlbuild_python_front::compile_driver::main::project_load_warehouse::project_load_warehouse;
use sqlbuild_python_front::compile_driver::models::ProjectConfiguration;
use sqlload::column_types::models::Warehouse;

/// `load`'s settings as SQLLoad reads them in `warehouse`, with its sources' column types.
fn sqlload_settings<'py>(
    py: Python<'py>,
    (load, warehouse): (&LoadDeclaration, Warehouse),
) -> PyResult<Bound<'py, PyAny>> {
    let value: serde_json::Value = serde_json::to_value(
        load_settings(load, warehouse).map_err(|problem| PyValueError::new_err(problem.message))?,
    )
    .map_err(|error| PyValueError::new_err(error.to_string()))?;
    json_value_to_python(py, &value)
}

/// A `CompiledLoad` for each load the discovered source files declare, in file order.
pub(crate) fn compiled_loads<'py>(discovered: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = discovered.py();
    let mut loads: Vec<Bound<'py, PyAny>> = Vec::new();
    let declared = discovered_loads(&native_field(discovered, "source_files")?)?;
    if declared.is_empty() {
        return PyTuple::new(py, loads);
    }
    let (project_dir, project_config, local_config) = (
        native_field(discovered, "project_dir")?,
        native_field(discovered, "project_config")?,
        native_field(discovered, "local_config")?,
    );
    let warehouse: Warehouse = project_load_warehouse(&ProjectConfiguration {
        project_dir: &project_dir,
        project_config: &project_config,
        local_config: &local_config,
    })?
    .map_err(PyValueError::new_err)?;
    for (file, declaration) in declared {
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
                ("settings", sqlload_settings(py, (&declaration, warehouse))?),
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
