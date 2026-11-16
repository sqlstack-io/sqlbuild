//! SQL model files discovered natively and built as Python discovery records.

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_discovery::model_files::main::discover_model_files::discover_model_files as discover_models;
use sqlbuild_discovery::model_files::main::model_file_stem::model_file_stem;
use sqlbuild_discovery::model_files::models::{DiscoveredModelFile, ModelFileOptions};
use sqlbuild_discovery::models::{DiscoveredFile, FileOutcome, StageFailure};
use std::path::Path;

use crate::discovery::_helpers::failures::raised::{failure_error, read_error, stage_error};
use crate::discovery::_helpers::project::project_files::{model_record, path_object};
use crate::discovery::_helpers::records::model_file_record::DiscoveredPaths;
use crate::discovery::_helpers::snapshots::native_classes::NativeProjectTree;
use crate::discovery::_helpers::snapshots::snapshot::{in_snapshot, project_root, project_tree};
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const PATHLIB_MODULE: &str = "pathlib";
const STEM_ATTRIBUTE: &str = "stem";

/// One discovered model file: its relative path and record, or the error building it raises.
pub(crate) type ModelItem<'py> = (RelativePath<'py>, PyResult<Bound<'py, PyAny>>);

/// A discovered file's relative path: native text, or the Python path already built for it.
pub(crate) enum RelativePath<'py> {
    Native(String),
    Python(Bound<'py, PyAny>),
}

impl<'py> RelativePath<'py> {
    pub(crate) fn object(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match self {
            Self::Native(relative) => Path::new(relative).into_pyobject(py),
            Self::Python(relative) => Ok(relative.clone()),
        }
    }

    /// The model's name, its file stem.
    pub(crate) fn name(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match self {
            Self::Native(relative) => Ok(PyString::new(py, model_file_stem(relative)).into_any()),
            Self::Python(relative) => native_field(relative, STEM_ATTRIBUTE),
        }
    }
}

/// Discover every model file natively, each as its record or the error it raises when built.
pub(crate) fn model_items<'py>(
    project_dir: &Bound<'py, PyAny>,
    options: &ModelFileOptions,
) -> PyResult<Vec<ModelItem<'py>>> {
    let py: Python<'py> = project_dir.py();
    in_snapshot(project_dir, || {
        let tree: Bound<'py, NativeProjectTree> = project_tree(project_dir)?;
        let root = project_root(project_dir)?;
        let discovered: Result<Vec<DiscoveredFile<DiscoveredModelFile>>, StageFailure> = {
            let inner = &tree.get().inner;
            py.detach(|| discover_models(&root, inner, options))
        };
        let files =
            discovered.map_err(|failure| stage_error(&tree.get().inner, project_dir, failure))?;
        let path_class: Bound<'py, PyAny> = imported(py, PATHLIB_MODULE, "Path")?;
        let mut items: Vec<ModelItem<'py>> = Vec::with_capacity(files.len());
        for file in files {
            let raw: bool = tree.get().inner.raw_relative(&file.relative_path).is_some();
            if !raw && let FileOutcome::Parsed(model) = file.outcome {
                crate::discovery::_helpers::headers::header_keys::record_header_keys(
                    &file.relative_path,
                    &model.header_values,
                );
                let file_path: String = root.display_path(&file.relative_path);
                let paths = DiscoveredPaths::Native(file.relative_path.clone(), file_path);
                let built = model_record(py, model, paths, options.extract_implicit_alias_columns)?
                    .map(|record| record.into_bound(py));
                items.push((RelativePath::Native(file.relative_path), built));
                continue;
            }
            let relative_path: Bound<'py, PyAny> =
                path_class.call1((path_object(py, &tree.get().inner, file.relative_path)?,))?;
            let file_path: Bound<'py, PyAny> = project_dir.div(&relative_path)?;
            let built: PyResult<Bound<'py, PyAny>> = match file.outcome {
                FileOutcome::Parsed(model) => model_record(
                    py,
                    model,
                    DiscoveredPaths::Python(file_path.unbind(), relative_path.clone().unbind()),
                    options.extract_implicit_alias_columns,
                )?
                .map(|record| record.into_bound(py)),
                FileOutcome::Unreadable(failure) => Err(read_error(failure, &file_path)),
                FileOutcome::Failed(failure) => Err(failure_error(py, failure)),
            };
            items.push((RelativePath::Python(relative_path), built));
        }
        Ok(items)
    })
}
