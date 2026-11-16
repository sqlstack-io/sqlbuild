//! The (selected) model files; `extraction` is `(implicit alias columns, output locations)`.

use crate::discovery::_helpers::failures::faults::{FileFaults, record_tuple};
use crate::discovery::_helpers::records::model_records::ModelItem;
use crate::discovery::_helpers::records::model_records::model_items;
use crate::discovery::models::ModelFileSelection;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_discovery::model_files::main::model_file_options::model_file_options;
use sqlbuild_discovery::model_files::models::ModelFileOptions;

/// The (selected) model files; `extraction` is `(implicit alias columns, output locations)`.
pub fn discovered_model_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    extraction: (bool, bool),
    selection: ModelFileSelection<'py>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = project_dir.py();
    let options: ModelFileOptions = model_file_options(
        extraction,
        sqlbuild_python_base::compile_context::main::delta::injection::injection()
            .map(|injection| std::sync::Arc::new(injection.model_paths.clone())),
    );
    let items: Vec<ModelItem<'py>> = model_items(project_dir, &options)?;
    let faults = FileFaults {
        project_dir: project_dir.clone(),
        on_fault: selection.on_fault.filter(|on_fault| !on_fault.is_none()),
    };
    let selected: Option<Bound<'py, PyAny>> = selection
        .selected_model_names
        .filter(|selected| !selected.is_none());
    let mut records: Vec<Bound<'py, PyAny>> = Vec::with_capacity(items.len());
    for (relative_path, built) in items {
        if let Some(selected) = &selected
            && !selected.contains(relative_path.name(py)?)?
        {
            continue;
        }
        match built {
            Ok(record) => records.push(record),
            Err(error) => {
                records.extend(faults.kept(&relative_path.object(py)?, Err(error))?);
            }
        }
    }
    record_tuple(py, records)
}
