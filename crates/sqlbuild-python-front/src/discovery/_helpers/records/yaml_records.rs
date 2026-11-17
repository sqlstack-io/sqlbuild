//! Source and schema YAML files discovered natively and built as Python discovery records.

use crate::discovery::main::snapshots::escaped::escaped;
use pyo3::exceptions::PyUnicodeEncodeError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyList, PyStringMethods};
use pyo3::{IntoPyObjectExt, pyfunction};
use sqlbuild_config::models::ConfigValue;
use sqlbuild_discovery::models::{DiscoveryFailure, FailureKind, ProjectRoot, StageFailure};
use sqlbuild_discovery::tree::models::ProjectTree;
use sqlbuild_discovery::yaml_files::main::load_yaml_files::load_yaml_files as load_yaml;
use sqlbuild_discovery::yaml_files::main::load_yaml_text::load_yaml_text;
use sqlbuild_discovery::yaml_files::models::YamlFileOutcome;
use sqlbuild_python_base::compile_context::main::yaml_cache::retain_read_sources::retain_read_sources;
use std::path::PathBuf;

use crate::discovery::_helpers::failures::raised::{
    discovery_error, failure_error, read_error, stage_error,
};
use crate::discovery::_helpers::project::project_files::path_object;
use crate::discovery::_helpers::snapshots::native_classes::{
    NativeProjectTree, YamlDiscoveryRequest,
};
use crate::discovery::_helpers::snapshots::snapshot::{project_root, project_tree};
use sqlbuild_python_base::boundary::main::config_value_to_python::config_value_to_python;
use sqlbuild_python_base::boundary::main::imported::imported;

/// The most YAML bytes one native load reads before its files are built and released.
const YAML_BATCH_BYTES: u64 = 4 * 1024 * 1024;
const PATHLIB_MODULE: &str = "pathlib";
const PROJECT_PATH_KIND: &str = "project_path";
pub(crate) const SCHEMA_KIND: &str = "schema";

/// The YAML files one discovery reads, and the kind their failures raise as.
#[derive(Clone, Copy)]
pub(crate) enum YamlKind {
    Source,
    Schema,
}

impl YamlKind {
    fn failure_kind(self) -> FailureKind {
        match self {
            Self::Source => FailureKind::Source,
            Self::Schema => FailureKind::Schema,
        }
    }
}

pub(crate) fn path_class(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    imported(py, PATHLIB_MODULE, "Path")
}

/// `(contents, loaded)` of a loaded file; a value Python cannot build fails like invalid YAML.
pub(crate) fn loaded<'py>(
    py: Python<'py>,
    (display_path, kind): (&str, FailureKind),
    (contents, value): (String, ConfigValue),
) -> PyResult<(String, Bound<'py, PyAny>)> {
    match config_value_to_python(py, value) {
        Ok(loaded) => Ok((contents, loaded.into_bound(py))),
        Err(error) => Err(failure_error(
            py,
            DiscoveryFailure::new(
                kind,
                format!("{display_path} contains invalid YAML: {}", error.value(py)),
            ),
        )),
    }
}

/// One file's `(contents, loaded)`, or the error Python raises for it.
fn outcome_loaded<'py>(
    root: &ProjectRoot,
    (relative_path, kind): (&str, FailureKind),
    outcome: YamlFileOutcome,
    file_path: &Bound<'py, PyAny>,
) -> PyResult<(String, Bound<'py, PyAny>)> {
    let py: Python<'py> = file_path.py();
    match outcome {
        YamlFileOutcome::Loaded { contents, value } => loaded(
            py,
            (&root.display_path(relative_path), kind),
            (contents, value),
        ),
        YamlFileOutcome::Failed(failure) => Err(failure_error(py, failure)),
        YamlFileOutcome::Unreadable(failure) => Err(read_error(failure, file_path)),
    }
}

/// The pass's tree, or a private one for an undecodable project directory (fails on first read).
pub(crate) fn listing_tree<'py>(project_dir: &Bound<'py, PyAny>) -> PyResult<ListingTree<'py>> {
    match project_dir.str()?.to_str() {
        Ok(_) => Ok(ListingTree::Shared(project_tree(project_dir)?)),
        Err(error) if error.is_instance_of::<PyUnicodeEncodeError>(project_dir.py()) => {
            let directory: PathBuf = project_dir.extract()?;
            Ok(ListingTree::Private(ProjectTree::new(&directory)))
        }
        Err(error) => Err(error),
    }
}

pub(crate) enum ListingTree<'py> {
    Shared(Bound<'py, NativeProjectTree>),
    Private(ProjectTree),
}

impl ListingTree<'_> {
    pub(crate) fn tree(&self) -> &ProjectTree {
        match self {
            Self::Shared(tree) => &tree.get().inner,
            Self::Private(tree) => tree,
        }
    }
}

/// Split the files into batches bounded by size, as Python's `_batches` does.
fn batches(tree: &ProjectTree, paths: Vec<String>, limit: u64) -> Vec<Vec<String>> {
    let mut batches: Vec<Vec<String>> = Vec::new();
    let mut batch: Vec<String> = Vec::new();
    let mut batch_bytes: u64 = 0;
    for path in paths {
        let size: u64 =
            std::fs::metadata(tree.absolute(&path)).map_or(0, |metadata| metadata.len());
        if !batch.is_empty() && batch_bytes + size > limit {
            batches.push(std::mem::take(&mut batch));
            batch_bytes = 0;
        }
        batch.push(path);
        batch_bytes += size;
    }
    if !batch.is_empty() {
        batches.push(batch);
    }
    batches
}

/// Pass each file's paths and `(contents, loaded)` or error to `on_file`, in size-bounded batches.
pub(crate) fn load_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    (paths, kind): (Vec<String>, YamlKind),
    mut on_file: impl FnMut(
        (Bound<'py, PyAny>, Bound<'py, PyAny>),
        PyResult<(String, Bound<'py, PyAny>)>,
    ) -> PyResult<()>,
) -> PyResult<()> {
    let py: Python<'py> = project_dir.py();
    let tree: Bound<'py, NativeProjectTree> = project_tree(project_dir)?;
    let root: ProjectRoot = project_root(project_dir)?;
    let mut documents: Option<sqlbuild_python_base::compile_context::models::DocumentStore> =
        if matches!(kind, YamlKind::Source) {
            sqlbuild_python_base::compile_context::main::yaml_cache::open_documents::open_documents(
                py,
            )
        } else {
            None
        };
    for batch in batches(&tree.get().inner, paths, YAML_BATCH_BYTES) {
        let named: Vec<String> = batch
            .iter()
            .filter(|path| !tree.get().inner.is_undecodable(path))
            .cloned()
            .collect();
        let outcomes: Result<Vec<YamlFileOutcome>, StageFailure> = {
            let inner = &tree.get().inner;
            let failure_kind: FailureKind = kind.failure_kind();
            match documents.as_mut() {
                Some(documents) => py.detach(|| {
                    documents.load_with(
                        &named,
                        |path| inner.absolute(path),
                        |missed| load_yaml(&root, inner, missed, failure_kind),
                    )
                }),
                None => py.detach(|| load_yaml(&root, inner, &named, failure_kind)),
            }
        };
        let mut outcomes: Vec<YamlFileOutcome> =
            outcomes.map_err(|failure| stage_error(&tree.get().inner, project_dir, failure))?;
        if matches!(kind, YamlKind::Source) {
            outcomes = retain_read_sources(outcomes);
        }
        let mut outcomes = outcomes.into_iter();
        for path in batch {
            let relative: Bound<'py, PyAny> =
                path_object(py, &tree.get().inner, path.clone())?.into_bound(py);
            let relative_path: Bound<'py, PyAny> = path_class(py)?.call1((&relative,))?;
            let file_path: Bound<'py, PyAny> = project_dir.div(&relative_path)?;
            let built: PyResult<(String, Bound<'py, PyAny>)> =
                if tree.get().inner.is_undecodable(&path) {
                    Err(unnamed_error(project_dir, &relative)?)
                } else {
                    match outcomes.next() {
                        Some(outcome) => {
                            outcome_loaded(&root, (&path, kind.failure_kind()), outcome, &file_path)
                        }
                        None => Err(pyo3::exceptions::PyRuntimeError::new_err(
                            "native YAML loading returned too few files",
                        )),
                    }
                };
            on_file((relative_path, file_path), built)?;
        }
    }
    if let Some(documents) = &documents {
        sqlbuild_python_base::compile_context::main::yaml_cache::save_documents::save_documents(
            py, documents,
        );
    }
    Ok(())
}

/// The `ProjectPathError` of a file whose name native discovery cannot read.
fn unnamed_error(project_dir: &Bound<'_, PyAny>, relative: &Bound<'_, PyAny>) -> PyResult<PyErr> {
    let shown: String = escaped(&project_dir.div(relative)?.str()?)?;
    Ok(discovery_error(
        project_dir.py(),
        PROJECT_PATH_KIND,
        format!("Project path {shown} is not valid UTF-8; rename it so SQLBuild can read it"),
        None,
    ))
}

fn yaml_kind(kind: &str) -> PyResult<FailureKind> {
    match kind {
        "source" => Ok(FailureKind::Source),
        "schema" => Ok(FailureKind::Schema),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown YAML file kind {other:?}"
        ))),
    }
}

/// Read and load YAML files: each file's `(contents, loaded)`, or the error it raises.
#[pyfunction]
pub(crate) fn load_yaml_files<'py>(
    py: Python<'py>,
    request: YamlDiscoveryRequest,
    relative_paths: Vec<String>,
    tree: &NativeProjectTree,
) -> PyResult<Bound<'py, PyList>> {
    let kind: FailureKind = yaml_kind(&request.kind)?;
    let root = ProjectRoot {
        directory: PathBuf::from(&request.project_dir),
        display_prefix: request.display_prefix,
    };
    let project_dir: Bound<'py, PyAny> = path_class(py)?.call1((request.project_dir.as_str(),))?;
    let outcomes: Vec<YamlFileOutcome> = py
        .detach(|| load_yaml(&root, &tree.inner, &relative_paths, kind))
        .map_err(|failure| stage_error(&tree.inner, &project_dir, failure))?;
    let mut rows: Vec<Bound<'py, PyAny>> = Vec::with_capacity(outcomes.len());
    for (outcome, relative_path) in outcomes.into_iter().zip(&relative_paths) {
        let file_path: Bound<'py, PyAny> = project_dir.div(relative_path)?;
        rows.push(
            match outcome_loaded(&root, (relative_path, kind), outcome, &file_path) {
                Ok(loaded) => loaded.into_bound_py_any(py)?,
                Err(error) => error.into_value(py).into_any().into_bound(py),
            },
        );
    }
    PyList::new(py, rows)
}

/// Load one in-memory YAML document as a discovered `kind` file named `file_path` loads.
#[pyfunction]
pub(crate) fn load_yaml_document<'py>(
    py: Python<'py>,
    file_path: &str,
    text: String,
    kind: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let kind: FailureKind = yaml_kind(kind)?;
    let value: ConfigValue =
        load_yaml_text(file_path, &text, kind).map_err(|failure| failure_error(py, failure))?;
    Ok(loaded(py, (file_path, kind), (text, value))?.1)
}
