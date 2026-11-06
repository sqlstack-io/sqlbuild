//! Bounded catalog pages use adapter inspection, never model declarations or warehouse writes.

use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_orchestration::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use sqlbuild_orchestration::warehouse_metadata::models::{
    CatalogRead, WarehouseCatalogColumn, WarehouseCatalogRecord, WarehouseHistoryPage,
    WarehouseHistoryRecord,
};

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::records::fields::field;

pub(crate) fn read_catalog(
    py: Python<'_>,
    (adapter, connection): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    request: &CatalogRead,
) -> PyResult<WarehouseHistoryPage> {
    let limit = request.limit.unwrap_or(MAX_HISTORY_RECORDS);
    if request.observed_since.is_some() {
        return Err(PyValueError::new_err(
            "catalog observations require full scans, not timestamp watermarks",
        ));
    }
    if limit == 0 || limit > MAX_HISTORY_RECORDS {
        return Err(PyValueError::new_err(
            "metadata page limit is outside the supported bounds",
        ));
    }
    let namespace = [
        (
            "database",
            request.location.database.clone().into_bound_py_any(py)?,
        ),
        (
            "schema",
            request.location.schema.clone().into_bound_py_any(py)?,
        ),
    ];
    if !call_with(
        &adapter.getattr("lists_relations_authoritatively")?,
        &namespace,
    )?
    .extract::<bool>()?
    {
        return Err(PyValueError::new_err(
            "catalog listing is not authoritative for this namespace; scan incomplete",
        ));
    }
    let relations = call_with(
        &adapter.getattr("list_relations")?,
        &[
            ("connection", connection.clone()),
            (
                "database",
                request.location.database.clone().into_bound_py_any(py)?,
            ),
            (
                "schemas",
                (request.location.schema.clone(),).into_bound_py_any(py)?,
            ),
        ],
    )?;
    let mut selected: Vec<(String, Bound<'_, PyAny>)> = Vec::new();
    for relation in relations.try_iter()? {
        let relation = relation?;
        let name: String = field(&relation, "name")?.extract()?;
        if request
            .after
            .as_ref()
            .and_then(|cursor| cursor[0].as_deref())
            .is_none_or(|after| name.as_str() > after)
        {
            selected.push((name, relation));
        }
    }
    selected.sort_by(|left, right| left.0.cmp(&right.0));
    let more = selected.len() > limit;
    selected.truncate(limit);
    if selected.is_empty() {
        return Ok(WarehouseHistoryPage {
            records: Vec::new(),
            next_after: None,
        });
    }
    let names = PyTuple::new(py, selected.iter().map(|(name, _)| name))?;
    let columns = call_with(
        &adapter.getattr("get_all_columns")?,
        &[
            ("connection", connection.clone()),
            ("database", namespace[0].1.clone()),
            (
                "schemas",
                (request.location.schema.clone(),).into_bound_py_any(py)?,
            ),
            ("names", names.into_any()),
        ],
    )?;
    let columns = columns.cast::<PyDict>()?;
    let next_after = if more {
        selected.last().map(|(name, _)| vec![Some(name.clone())])
    } else {
        None
    };
    let mut records: Vec<WarehouseHistoryRecord> = Vec::new();
    for (name, relation) in selected {
        let relation_columns = columns.get_item(name.as_str())?.ok_or_else(|| {
            PyValueError::new_err("catalog relation columns are unavailable; scan incomplete")
        })?;
        let mut decoded: Vec<WarehouseCatalogColumn> = Vec::new();
        for column in relation_columns.try_iter()? {
            let column = column?;
            decoded.push(WarehouseCatalogColumn {
                name: field(&column, "name")?.extract()?,
                data_type: field(&column, "type")?.extract()?,
            });
        }
        records.push(WarehouseHistoryRecord::Catalog(WarehouseCatalogRecord {
            state_database: request.location.database.clone(),
            state_schema: request.location.schema.clone(),
            name,
            relation_type: field(&relation, "relation_type")?.extract()?,
            columns: decoded,
        }));
    }
    Ok(WarehouseHistoryPage {
        records,
        next_after,
    })
}
