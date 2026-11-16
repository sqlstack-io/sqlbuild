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
