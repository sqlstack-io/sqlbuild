//! Read catalog relation shapes from Python mappings in their authored column order.

use pyo3::prelude::{Bound, FromPyObject, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_analysis::semantic_validation::models::Columns;
use sqlbuild_analysis::semantic_validation::types::{BindingRequest, Relations};
use std::collections::HashMap;

/// One relation's columns, read from a `{name: type}` mapping.
struct ColumnList(Columns);

/// Relation shapes keyed by relation name.
struct RelationMap(Relations);

/// One binding request: SQL, its referenced relations and per-request relation overrides.
struct BindingRequestItem(BindingRequest);

impl<'py> FromPyObject<'py> for ColumnList {
    fn extract_bound(value: &Bound<'py, PyAny>) -> PyResult<Self> {
        columns(value).map(Self)
    }
}

impl<'py> FromPyObject<'py> for RelationMap {
    fn extract_bound(value: &Bound<'py, PyAny>) -> PyResult<Self> {
        relations(value).map(Self)
    }
}

impl<'py> FromPyObject<'py> for BindingRequestItem {
    fn extract_bound(value: &Bound<'py, PyAny>) -> PyResult<Self> {
        let (sql, references, overrides): (String, Vec<(String, bool)>, RelationMap) =
            value.extract()?;
        Ok(Self((sql, references, overrides.0)))
    }
}

pub(crate) fn columns(value: &Bound<'_, PyAny>) -> PyResult<Columns> {
    value
        .downcast::<PyDict>()?
        .iter()
        .map(|(name, data_type)| Ok((name.extract()?, data_type.extract()?)))
        .collect::<PyResult<Vec<_>>>()
        .map(Columns)
}

pub(crate) fn relations(value: &Bound<'_, PyAny>) -> PyResult<Relations> {
    let dict = value.cast::<PyDict>()?;
    let mut relations = HashMap::with_capacity(dict.len());
    for (name, shape) in dict {
        relations.insert(name.extract()?, columns(&shape)?);
    }
    Ok(relations)
}

pub(crate) fn analysis_relations(
    value: &Bound<'_, PyAny>,
) -> PyResult<HashMap<String, (Columns, Columns)>> {
    let dict = value.cast::<PyDict>()?;
    let mut relations = HashMap::with_capacity(dict.len());
    for (name, shapes) in dict {
        let name: String = name.extract()?;
        let (types, nullability): (ColumnList, ColumnList) = shapes.extract()?;
        relations.insert(name, (types.0, nullability.0));
    }
    Ok(relations)
}

pub(crate) fn binding_requests(value: &Bound<'_, PyAny>) -> PyResult<Vec<BindingRequest>> {
    let requests: Vec<BindingRequestItem> = value.extract()?;
    Ok(requests.into_iter().map(|request| request.0).collect())
}
