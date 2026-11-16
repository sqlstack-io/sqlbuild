//! Analysis's Python classes and the mapping requests converted to native models at the boundary.

use crate::analysis::models::Positional;
use pyo3::FromPyObject;
use sqlbuild_analysis::semantic_validation::models as validation;
use sqlbuild_analysis::semantic_validation::models::Expansion;
use sqlbuild_analysis::semantic_validation::types::Relations;

#[derive(FromPyObject, Debug)]
#[pyo3(from_item_all)]
pub(crate) struct CatalogInput {
    pub(crate) dialect: String,
    pub(crate) quoted_ignore_case: bool,
    pub(crate) known_functions: Vec<String>,
    pub(crate) known_types: Vec<String>,
    #[pyo3(from_py_with = crate::analysis::_helpers::conversions::relations)]
    pub(crate) relations: Relations,
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
