//! Model description resolution for header-only tooling, without importing project Python.

use crate::discovery::main::project::load_project_config_file::load_project_config_file;
use crate::discovery::main::records::discover_model_schema_files::discover_model_schema_files;
use crate::discovery::main::records::discovered_model_files::discovered_model_files;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyCFunction;

use crate::discovery::_helpers::snapshots::snapshot::in_snapshot;
use crate::discovery::models::ModelFileSelection;
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
