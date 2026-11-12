//! Bounded catalog pages use adapter inspection, never model declarations or warehouse writes.

use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyTuple};
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
    let schema_scoped = adapter.is_instance(
        &py.import("sqlbuild.adapter.relations.types")?
            .getattr("SchemaScopedMetadataReader")?,
    )?;
    let mut selected: Vec<(String, String, Bound<'_, PyAny>)> = Vec::new();
    let listed = if schema_scoped {
        field(
            &call_with(
                &adapter.getattr("read_schema_relation_listing")?,
                &[
                    ("connection", connection.clone()),
                    namespace[0].clone(),
                    namespace[1].clone(),
                ],
            )?,
            "entries",
        )?
    } else {
        call_with(
            &adapter.getattr("list_relations")?,
            &[
                ("connection", connection.clone()),
                namespace[0].clone(),
                (
                    "schemas",
                    (request.location.schema.clone(),).into_bound_py_any(py)?,
                ),
            ],
        )?
    };
    for item in listed.try_iter()? {
        let item = item?;
        let (relation, key) = if schema_scoped {
            (field(&item, "relation")?, field(&item, "stored_name")?)
        } else {
            (item.clone(), field(&item, "name")?)
        };
        let name: String = field(&relation, "name")?.extract()?;
        if request
            .after
            .as_ref()
            .and_then(|cursor| cursor[0].as_deref())
            .is_none_or(|after| name.as_str() > after)
        {
            let relation_type: String = field(&relation, "relation_type")?.extract()?;
            selected.push((name, relation_type, key));
        }
    }
    selected.sort_by(|left, right| left.0.cmp(&right.0));
    let more = selected.len() > limit;
    selected.truncate(limit);
    if selected.is_empty() {
        return Ok(WarehouseHistoryPage {
            records: Vec::new(),
            next_after: None,
            change_token: None,
            unchanged: false,
        });
    }
    let keys = selected.iter().map(|(_, _, key)| key);
    let columns = if schema_scoped {
        field(
            &call_with(
                &adapter.getattr("read_schema_column_listing")?,
                &[
                    ("connection", connection.clone()),
                    namespace[0].clone(),
                    namespace[1].clone(),
                    ("stored_names", PyFrozenSet::new(py, keys)?.into_any()),
                ],
            )?,
            "columns_by_stored_name",
        )?
    } else {
        call_with(
            &adapter.getattr("get_all_columns")?,
            &[
                ("connection", connection.clone()),
                namespace[0].clone(),
                (
                    "schemas",
                    (request.location.schema.clone(),).into_bound_py_any(py)?,
                ),
                ("names", PyTuple::new(py, keys)?.into_any()),
            ],
        )?
    };
    let columns = columns.cast::<PyDict>()?;
    let next_after = if more {
        selected.last().map(|(name, _, _)| vec![Some(name.clone())])
    } else {
        None
    };
    let mut records: Vec<WarehouseHistoryRecord> = Vec::new();
    for (name, relation_type, key) in selected {
        let relation_columns = columns.get_item(key)?.ok_or_else(|| {
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
            relation_type,
            columns: decoded,
        }));
    }
    Ok(WarehouseHistoryPage {
        records,
        next_after,
        change_token: None,
        unchanged: false,
    })
}
