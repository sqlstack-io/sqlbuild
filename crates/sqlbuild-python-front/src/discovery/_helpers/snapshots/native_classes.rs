//! Discovery's Python classes and the mapping requests its native calls read.

use pyo3::{FromPyObject, pyclass};

/// The directory listings one discovery pass shares across its native calls.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub(crate) struct NativeProjectTree {
    pub(crate) inner: sqlbuild_discovery::tree::models::ProjectTree,
}

/// One discovery pass of a project: the memo its native calls share, active inside its scope.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub(crate) struct NativeDirectorySnapshot {
    pub(crate) project_dir: pyo3::Py<pyo3::PyAny>,
    pub(crate) memo: pyo3::Py<pyo3::types::PyDict>,
}

/// One native YAML file discovery request; `kind` is `source` or `schema`.
#[derive(FromPyObject, Debug)]
#[pyo3(from_item_all)]
pub(crate) struct YamlDiscoveryRequest {
    pub(crate) project_dir: String,
    pub(crate) display_prefix: String,
    pub(crate) kind: String,
}

/// One in-memory SQL model parse request; `file_path` is the path messages name.
#[derive(FromPyObject, Debug)]
#[pyo3(from_item_all)]
pub(crate) struct ModelTextRequest {
    pub(crate) file_path: String,
    pub(crate) supported_keys: Vec<String>,
    pub(crate) removed_keys: Vec<String>,
    pub(crate) extract_implicit_alias_columns: bool,
    pub(crate) extract_output_column_locations: bool,
}

/// The declaration collections one Python discovery pass read natively, kept for later stages.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub(crate) struct NativeDiscoverySession {
    pub(crate) inner: sqlbuild_discovery::declaration_files::models::DiscoverySession,
}

/// One in-memory declaration file parse request; `kind` names the collection it belongs to.
#[derive(FromPyObject, Debug)]
#[pyo3(from_item_all)]
pub(crate) struct DeclarationTextRequest {
    pub(crate) kind: String,
    pub(crate) function_keys: Vec<String>,
    pub(crate) audit_keys: Vec<String>,
    pub(crate) hook_keys: Vec<String>,
}
