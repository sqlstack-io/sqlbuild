//! The native owner's inputs view when it holds every model input; else a record.

use crate::compiled_project::models::NativeCompiledProject;
use crate::compiled_views::models::NativeInputsView;
use crate::compiled_views::models::NativeModelInput;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{IntoPyObjectExt, pyfunction};
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::types::RecordId;

/// The native owner's inputs view when it holds every model input; else a record.
#[pyfunction]
#[pyo3(signature = (*, fields, native_project))]
pub fn compile_inputs_view<'py>(
    fields: &Bound<'py, PyDict>,
    native_project: Option<Bound<'py, NativeCompiledProject>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = fields.py();
    if let Some(project) = &native_project {
        let mut native: bool = true;
        for model in fields.as_any().get_item("model_inputs")?.try_iter()? {
            if !model?.is_instance_of::<NativeModelInput>() {
                native = false;
                break;
            }
        }
        if native {
            let view: NativeInputsView =
                NativeCompiledProject::inputs_view(project.clone(), fields.clone())?;
            return view.into_bound_py_any(py);
        }
    }
    let keywords: Bound<'py, PyDict> = fields.copy()?;
    keywords.set_item("native_project", native_project)?;
    new_record(py, RecordId::CompileProjectInputs, (), Some(&keywords))
}
