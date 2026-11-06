//! The Python classes a model's `CompileModelConfig` is built from, and building one from a native build.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyStringMethods, PyTuple};
use pyo3::{FromPyObject, IntoPyObject};
use sqlbuild_model_config::model_validation::models::{RetentionOverride, TableTypeOverride};

use crate::bindings::_helpers::model_config::model_config_builder::{
    BuiltConfig, NativeModelConfigBuilder,
};
use crate::bindings::_helpers::model_config::typed_config::{BuiltConfigValues, retain_values};

const MATERIALIZED_KEY: &str = "materialized";
const VALUES_FIELD: &str = "values";
const PERMANENT_TABLE_TYPE: &str = "permanent";
const TRANSIENT_TABLE_TYPE: &str = "transient";

/// The builder, storage defaults and Python classes a model's `CompileModelConfig` is made of.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct ConfigClasses<'py> {
    pub(crate) builder: Bound<'py, NativeModelConfigBuilder>,
    /// Inherited `(retention, table type)` by table-backed materialization, or `None`.
    pub(crate) inherited_storage: Bound<'py, PyDict>,
    pub(crate) table_backed: Vec<String>,
    pub(crate) compile_model_config: Bound<'py, PyAny>,
    pub(crate) resolved_retention: Bound<'py, PyAny>,
    pub(crate) retention_source: Bound<'py, PyAny>,
    pub(crate) resolved_table_type: Bound<'py, PyAny>,
    pub(crate) table_type: Bound<'py, PyAny>,
    pub(crate) table_type_source: Bound<'py, PyAny>,
    /// `record_template_reads`, the compile-reuse sink of template reads.
    pub(crate) record_template_reads: Bound<'py, PyAny>,
}

/// The model's `CompileModelConfig`, once its template reads are recorded.
pub(crate) fn built_config<'py>(
    classes: &ConfigClasses<'py>,
    built: &BuiltConfig<'py>,
    matched: Option<&str>,
) -> PyResult<Bound<'py, PyAny>> {
    let fields: Bound<'py, PyDict> = config_fields(classes, built, matched)?;
    view(classes, &built.values, &fields, built.nodes.as_ref())
}

/// Every `CompileModelConfig` field but `values`, recording the build's template reads first.
pub(crate) fn config_fields<'py>(
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

/// The materialization whose inherited storage policy applies, or `None` when not table-backed.
fn storage_key<'py>(
    classes: &ConfigClasses<'py>,
    values: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = values.py();
    if let Some(materialized) = values.get_item(MATERIALIZED_KEY)?
        && let Ok(text) = materialized.downcast::<PyString>()
        && let Ok(text) = text.to_str()
        && classes.table_backed.iter().any(|name| name == text)
    {
        return Ok(materialized);
    }
    Ok(py.None().into_bound(py))
}

fn resolved_retention<'py>(
    classes: &ConfigClasses<'py>,
    desired_days: Bound<'py, PyAny>,
    unmanaged: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let fields: Bound<'py, PyDict> = PyDict::new(desired_days.py());
    fields.set_item("desired_days", desired_days)?;
    fields.set_item("unmanaged", unmanaged)?;
    fields.set_item("source", &classes.retention_source)?;
    classes.resolved_retention.call((), Some(&fields))
}

fn resolved_table_type<'py>(
    classes: &ConfigClasses<'py>,
    value: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let fields: Bound<'py, PyDict> = PyDict::new(classes.table_type.py());
    fields.set_item("value", classes.table_type.call1((value,))?)?;
    fields.set_item("source", &classes.table_type_source)?;
    fields.set_item("declared", true)?;
    classes.resolved_table_type.call((), Some(&fields))
}

/// A `CompileModelConfig` of `values` and the other `fields`.
pub(crate) fn view<'py>(
    classes: &ConfigClasses<'py>,
    values: &Bound<'py, PyDict>,
    fields: &Bound<'py, PyDict>,
    nodes: Option<&BuiltConfigValues>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: Bound<'py, PyDict> = fields.copy()?;
    arguments.set_item(VALUES_FIELD, values)?;
    let config = classes.compile_model_config.call((), Some(&arguments))?;
    retain_values(&config, nodes)?;
    Ok(config)
}
