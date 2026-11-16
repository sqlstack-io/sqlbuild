//! The `SchemaColumn` tuple of one authored columns mapping, raising Python's error for it.

use crate::model_config::_helpers::headers::header_columns::ColumnOptions;
use crate::model_config::_helpers::headers::header_columns::ColumnSite;
use crate::model_config::_helpers::headers::header_columns::raised;
use crate::model_config::_helpers::headers::header_metadata::column_tuple;
use crate::model_config::main::model_nodes::contract_classes::contract_classes;
use crate::model_config::models::ContractClasses;
use crate::model_config::models::PyNode;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_model_config::header_metadata::main::parse_schema_columns::parse_schema_columns as parse_columns;
use sqlbuild_model_config::header_metadata::models::ColumnRequest;

/// The `SchemaColumn` tuple of one authored columns mapping, raising Python's error for it.
pub fn schema_columns<'py>(
    raw_columns: &Bound<'py, PyAny>,
    site: ColumnSite<'_>,
    options: ColumnOptions,
    column_locations: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = raw_columns.py();
    let (path, label) = site;
    let (require_columns, allow_migrate_from, declaration_errors) = options;
    let request = ColumnRequest {
        path,
        label,
        require_columns,
        allow_migrate_from,
    };
    let columns = parse_columns(&PyNode(raw_columns.clone()), request)
        .map_err(|error| raised(py, error, declaration_errors))?;
    let classes: ContractClasses<'py> = contract_classes(py)?.extract()?;
    column_tuple(py, &classes, &columns, column_locations)
}
