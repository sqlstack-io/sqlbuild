//! Model description resolution for header-only tooling, without importing project Python.

use pyo3::prelude::{Bound, PyAny, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyCFunction, PyDict, PyFrozenSet};
use pyo3::{pyfunction, wrap_pyfunction};

use crate::bindings::_helpers::discovery::declaration_records::discover_model_schema_files;
use crate::bindings::_helpers::discovery::model_records::{
    ModelFileSelection, discovered_model_files,
};
use crate::bindings::_helpers::discovery::project_config::load_project_config_file;
use crate::bindings::_helpers::discovery::snapshot::in_snapshot;
use crate::bindings::_helpers::model_config::described_models::externally_described_models as described_models;
use crate::bindings::_helpers::render::declaration_indexes::public_model_schema_index;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;

/// Models and model schemas, skipping files that cannot be read.
#[pyfunction]
#[pyo3(signature = (*, project_dir))]
pub(crate) fn discover_model_description_inputs<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    let project_config: Bound<'py, PyAny> =
        load_project_config_file(py, project_dir)?.into_bound(py);
    let skip: Bound<'py, PyAny> =
        PyCFunction::new_closure(py, None, None, |_arguments, _keywords| -> PyResult<()> {
            Ok(())
        })?
        .into_any();
    in_snapshot(project_dir, || {
        new_record_with(
            py,
            RecordId::DiscoveredProjectInputs,
            &[
                ("project_config", project_config),
                (
                    "local_config",
                    new_record(py, RecordId::LocalConfig, (), None)?,
                ),
                (
                    "model_files",
                    discovered_model_files(
                        project_dir,
                        (false, false),
                        ModelFileSelection {
                            selected_model_names: None,
                            on_fault: Some(skip.clone()),
                        },
                    )?
                    .into_any(),
                ),
                (
                    "model_schema_files",
                    discover_model_schema_files(project_dir, Some(skip))?.into_any(),
                ),
            ],
        )
    })
}

/// Model files that `[defaults]`, `[path_defaults]` or a model schema describe.
pub(crate) fn externally_described_models<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let py: Python<'py> = project_dir.py();
    let inputs: Bound<'py, PyAny> = discover_model_description_inputs(project_dir)?;
    let schemas: Bound<'py, PyDict> = public_model_schema_index(inputs.clone())?;
    PyFrozenSet::new(py, described_models(inputs, schemas)?)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(discover_model_description_inputs, module)?)?;
    Ok(())
}
