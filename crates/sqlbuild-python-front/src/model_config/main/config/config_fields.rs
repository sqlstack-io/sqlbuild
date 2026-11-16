//! Every `CompileModelConfig` field but `values`, recording the build's template reads first.

use crate::model_config::_helpers::config::config_classes::PERMANENT_TABLE_TYPE;
use crate::model_config::_helpers::config::config_classes::TRANSIENT_TABLE_TYPE;
use crate::model_config::_helpers::config::config_classes::resolved_retention;
use crate::model_config::_helpers::config::config_classes::resolved_table_type;
use crate::model_config::_helpers::config::config_classes::storage_key;
use crate::model_config::models::BuiltConfig;
use crate::model_config::models::ConfigClasses;
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_model_config::model_validation::models::{RetentionOverride, TableTypeOverride};

/// Every `CompileModelConfig` field but `values`, recording the build's template reads first.
pub fn config_fields<'py>(
    classes: &ConfigClasses<'py>,
    built: &BuiltConfig<'py>,
    matched: Option<&str>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = built.values.py();
    if !built.reads.is_empty() {
        classes
            .record_template_reads
            .call1((PyTuple::new(py, &built.reads)?,))?;
    }
    let none: Bound<'py, PyAny> = py.None().into_bound(py);
    let (inherited_retention, inherited_table_type): (Bound<'py, PyAny>, Bound<'py, PyAny>) =
        classes
            .inherited_storage
            .as_any()
            .get_item(storage_key(classes, &built.values)?)?
            .extract()?;
    let retention: Bound<'py, PyAny> = match built.retention {
        RetentionOverride::Inherit => inherited_retention,
        RetentionOverride::Unmanaged => resolved_retention(classes, none.clone(), true)?,
        RetentionOverride::Days(days) => {
            resolved_retention(classes, days.into_pyobject(py)?.into_any(), false)?
        }
    };
    let table_type: Bound<'py, PyAny> = match built.table_type {
        TableTypeOverride::Inherit => inherited_table_type,
        TableTypeOverride::Permanent => resolved_table_type(classes, PERMANENT_TABLE_TYPE)?,
        TableTypeOverride::Transient => resolved_table_type(classes, TRANSIENT_TABLE_TYPE)?,
    };
    let logical_schema: Bound<'py, PyAny> = built.logical_schema.clone().unwrap_or(none.clone());
    let fields: Bound<'py, PyDict> = PyDict::new(py);
    fields.set_item("model_header_keys", PyTuple::new(py, &built.header_keys)?)?;
    fields.set_item("matched_path_default", matched)?;
    fields.set_item("logical_schema", &logical_schema)?;
    fields.set_item(
        "layer_schema",
        if built.layer_schema_configured {
            logical_schema
        } else {
            none.clone()
        },
    )?;
    fields.set_item(
        "logical_database",
        built.logical_database.clone().unwrap_or(none),
    )?;
    fields.set_item("time_travel_retention", retention)?;
    fields.set_item("table_type", table_type)?;
    Ok(fields)
}
