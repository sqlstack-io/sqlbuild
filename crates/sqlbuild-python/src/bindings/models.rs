//! Python-facing classes and mapping requests, converted to native models at the boundary.

use pyo3::prelude::{Bound, IntoPyObject, PyAny, PyErr, PyResult, Python};
use pyo3::types::PyTuple;
use pyo3::{FromPyObject, pyclass};
use sqlbuild_analysis::semantic_validation::models as validation;
use sqlbuild_analysis::semantic_validation::models::Expansion;
use sqlbuild_analysis::semantic_validation::types::Relations;

use sqlbuild_python_base::types::FromPositional;
use sqlbuild_python_base::types::IntoPositional;
use std::collections::HashMap;

#[derive(FromPyObject, Debug)]
#[pyo3(from_item_all)]
pub(crate) struct CatalogInput {
    pub(crate) dialect: String,
    pub(crate) quoted_ignore_case: bool,
    pub(crate) known_functions: Vec<String>,
    pub(crate) known_types: Vec<String>,
    #[pyo3(from_py_with = crate::bindings::_helpers::analysis::conversions::relations)]
    pub(crate) relations: Relations,
}

#[derive(FromPyObject, Debug)]
#[pyo3(from_item_all)]
pub(crate) struct NormalizationInput {
    pub(crate) sql: String,
    pub(crate) dialect: String,
    pub(crate) stubs: HashMap<String, String>,
    pub(crate) placeholders: HashMap<String, String>,
}

#[derive(FromPyObject, Debug)]
#[pyo3(from_item_all)]
pub(crate) struct PositionInput {
    pub(crate) authored: String,
    pub(crate) query: String,
    pub(crate) expanded: String,
    pub(crate) cleaned: String,
    pub(crate) passes: Vec<Vec<Positional<Expansion>>>,
}

#[pyclass(module = "sqlbuild._native")]
#[derive(Debug)]
pub(crate) struct ProjectCatalog {
    pub(crate) inner: validation::ProjectCatalog,
}

/// One resolved compact analysis batch that runs without borrowing its project catalog.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct CompactAnalysisJob {
    pub(crate) inner: validation::CompactAnalysisJob,
}

#[pyclass(module = "sqlbuild._native")]
#[derive(Debug)]
pub(crate) struct BindingPositions {
    pub(crate) inner: validation::BindingPositions,
}

impl From<CatalogInput> for validation::CatalogInput {
    fn from(input: CatalogInput) -> Self {
        Self {
            dialect: input.dialect,
            quoted_ignore_case: input.quoted_ignore_case,
            known_functions: input.known_functions,
            known_types: input.known_types,
            relations: input.relations,
        }
    }
}

impl From<NormalizationInput> for validation::NormalizationInput {
    fn from(input: NormalizationInput) -> Self {
        Self {
            sql: input.sql,
            dialect: input.dialect,
            stubs: input.stubs,
            placeholders: input.placeholders,
        }
    }
}

impl From<PositionInput> for validation::PositionInput {
    fn from(input: PositionInput) -> Self {
        Self {
            authored: input.authored,
            query: input.query,
            expanded: input.expanded,
            cleaned: input.cleaned,
            passes: input
                .passes
                .into_iter()
                .map(Positional::unwrap_all)
                .collect(),
        }
    }
}

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

/// The context manager that makes one snapshot active for a project until it exits.
#[pyclass(module = "sqlbuild._native")]
#[derive(Debug)]
pub(crate) struct NativeDirectorySnapshotScope {
    pub(crate) project_dir: pyo3::Py<pyo3::PyAny>,
    /// The context-variable token of the snapshot this scope activated, if it activated one.
    pub(crate) token: Option<pyo3::Py<pyo3::PyAny>>,
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

/// A record of another crate exchanged with Python as its positional tuple.
#[derive(Clone, Debug)]
pub(crate) struct Positional<T>(pub(crate) T);

impl<T> Positional<T> {
    /// Each record of `items`, for Python.
    pub(crate) fn wrap_all(items: Vec<T>) -> Vec<Self> {
        items.into_iter().map(Self).collect()
    }

    /// Each record Python sent in `items`.
    pub(crate) fn unwrap_all(items: Vec<Self>) -> Vec<T> {
        items.into_iter().map(|item| item.0).collect()
    }
}

impl<'py, T: FromPositional> FromPyObject<'py> for Positional<T> {
    fn extract_bound(value: &Bound<'py, PyAny>) -> PyResult<Self> {
        T::from_positional(value).map(Self)
    }
}

impl<'py, T: IntoPositional> IntoPyObject<'py> for Positional<T> {
    type Target = PyTuple;
    type Output = Bound<'py, PyTuple>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        self.0.into_positional(py)
    }
}
