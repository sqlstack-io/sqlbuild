//! SQL model files discovered natively and built as Python discovery records.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyString, PyTuple};
use pyo3::{IntoPyObject, pyfunction, wrap_pyfunction};
use sqlbuild_discovery::model_files::main::discover_model_files::discover_model_files as discover_models;
use sqlbuild_discovery::model_files::main::model_file_stem::model_file_stem;
use sqlbuild_discovery::model_files::models::{DiscoveredModelFile, ModelFileOptions};
use sqlbuild_discovery::models::{DiscoveredFile, FileOutcome, StageFailure};
use std::path::Path;

use crate::bindings::_helpers::discovery::faults::{FileFaults, record_tuple};
use crate::bindings::_helpers::discovery::model_file_record::DiscoveredPaths;
use crate::bindings::_helpers::discovery::project_files::{model_record, path_object};
use crate::bindings::_helpers::discovery::raised::{failure_error, read_error, stage_error};
use crate::bindings::_helpers::discovery::snapshot::{in_snapshot, project_root, project_tree};
use crate::bindings::models::NativeProjectTree;
use sqlbuild_discovery::model_files::main::model_file_options::model_file_options;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const PATHLIB_MODULE: &str = "pathlib";
const STEM_ATTRIBUTE: &str = "stem";

/// One discovered model file: its relative path and record, or the error building it raises.
type ModelItem<'py> = (RelativePath<'py>, PyResult<Bound<'py, PyAny>>);

/// A discovered file's relative path: native text, or the Python path already built for it.
enum RelativePath<'py> {
    Native(String),
    Python(Bound<'py, PyAny>),
}

impl<'py> RelativePath<'py> {
    fn object(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match self {
            Self::Native(relative) => Path::new(relative).into_pyobject(py),
            Self::Python(relative) => Ok(relative.clone()),
        }
    }

    /// The model's name, its file stem.
    fn name(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match self {
            Self::Native(relative) => Ok(PyString::new(py, model_file_stem(relative)).into_any()),
            Self::Python(relative) => native_field(relative, STEM_ATTRIBUTE),
        }
    }
}

/// Discover every model file natively, each as its record or the error it raises when built.
fn model_items<'py>(
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
                crate::bindings::_helpers::discovery::header_keys::record_header_keys(
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

/// Which discovered model files to build, and where failing files are reported.
pub(crate) struct ModelFileSelection<'py> {
    pub(crate) selected_model_names: Option<Bound<'py, PyAny>>,
    pub(crate) on_fault: Option<Bound<'py, PyAny>>,
}

/// The (selected) model files; `extraction` is `(implicit alias columns, output locations)`.
pub(crate) fn discovered_model_files<'py>(
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

/// Discover the (selected) model files natively, reporting failing files to `on_fault`.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn discover_model_files<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "discover_model_files",
        options,
        (
            &[
                "project_dir",
                "extract_implicit_alias_columns",
                "extract_output_column_locations",
            ],
            &["selected_model_names", "on_fault"],
        ),
    )?;
    discovered_model_files(
        &arguments.value("project_dir")?,
        (
            arguments
                .value("extract_implicit_alias_columns")?
                .extract()?,
            arguments
                .value("extract_output_column_locations")?
                .extract()?,
        ),
        ModelFileSelection {
            selected_model_names: Some(arguments.value("selected_model_names")?),
            on_fault: Some(arguments.value("on_fault")?),
        },
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(discover_model_files, module)?)
}
