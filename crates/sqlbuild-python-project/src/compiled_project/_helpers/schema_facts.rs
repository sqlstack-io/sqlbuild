//! Native facts of declared schema columns and families, read once from their Python specs.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::record_attribute_opt::record_attribute_opt;

pub(crate) const NAME_FIELD: &str = "name";
pub(crate) const TYPE_FIELD: &str = "type";
pub(crate) const NULLABLE_FIELD: &str = "nullable";
pub(crate) const DESCRIPTION_FIELD: &str = "description";
pub(crate) const MIGRATE_FROM_FIELD: &str = "migrate_from";
pub(crate) const AUDITS_FIELD: &str = "audits";
pub(crate) const LOCATION_FIELD: &str = "location";
pub(crate) const PATH_FIELD: &str = "path";
pub(crate) const DEFINITION_NAME_FIELD: &str = "definition_name";
pub(crate) const COLUMNS_FIELD: &str = "columns";
pub(crate) const DYNAMIC_COLUMNS_FIELD: &str = "dynamic_columns";
pub(crate) const TYPE_ENFORCEMENT_FIELD: &str = "type_enforcement";
pub(crate) const MODEL_SCHEMA_FIELD: &str = "model_schema";
pub(crate) const PIVOT_COLUMN_FIELD: &str = "pivot_column";
pub(crate) const VALUE_COLUMN_FIELD: &str = "value_column";
pub(crate) const AGGREGATE_FIELD: &str = "aggregate";
pub(crate) const NAME_PATTERN_FIELD: &str = "name_pattern";

/// An attribute's value, `None` when it is missing or `None`.
pub(crate) fn present<'py>(
    spec: &Bound<'py, PyAny>,
    name: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let value: Option<Bound<'py, PyAny>> = match record_attribute_opt(spec, name) {
        Some(value) => value?,
        None => spec.getattr_opt(name)?,
    };
    Ok(value.filter(|value| !value.is_none()))
}

pub(crate) fn optional<'py, T: pyo3::prelude::FromPyObjectOwned<'py>>(
    spec: &Bound<'py, PyAny>,
    name: &str,
) -> PyResult<Option<T>> {
    present(spec, name)?
        .map(|value| value.extract().map_err(Into::into))
        .transpose()
}
