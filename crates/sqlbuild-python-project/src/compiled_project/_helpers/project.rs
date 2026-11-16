//! The compile's natively owned project, which producers record into and consumers read.

use crate::compiled_project::constants::CONTRACT_KEY;
use crate::compiled_project::constants::INCREMENTAL_MODE_KEY;
use crate::compiled_project::models::DeclarationPy;
use crate::compiled_project::models::NativeCompiledProject;
use sqlbuild_python_base::positional_record;
use std::collections::BTreeMap;

use pyo3::FromPyObject;
use pyo3::prelude::{Bound, PyAnyMethods, PyModule, PyModuleMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyString};
use serde_json::Value;
use sqlbuild_analysis::compiled_project::models::{
    CompiledModelFacts, DeclarationFact, ModelConfigFacts, ModelReferenceFact,
};

use crate::compiled_project::_helpers::values::object_value;
use crate::compiled_project::_helpers::values::plain_value;
use crate::compiled_views::models::NativeAuditInput;
use crate::compiled_views::models::NativeCompiledAudit;
use crate::compiled_views::models::NativeCompiledFunction;
use crate::compiled_views::models::NativeCompiledModel;
use crate::compiled_views::models::NativeCompiledSeed;
use crate::compiled_views::models::NativeCompiledSource;
use crate::compiled_views::models::NativeFunctionInput;
use crate::compiled_views::models::NativeInputsView;
use crate::compiled_views::models::NativeModelInput;
use crate::compiled_views::models::NativeProjectView;
use crate::compiled_views::models::NativeSeedInput;
use crate::compiled_views::models::NativeSourceInput;

pub(crate) const INPUT_CLASSES_UNSET: &str = "no model loop recorded inputs into this project";
pub(crate) const ATTACHMENT_LOOP_UNSET: &str =
    "no model loop was kept to attach this project's resources";

const MATERIALIZED_KEY: &str = "materialized";
const TABLE_MATERIALIZATION: &str = "table";

/// `(name, type)`.
pub(crate) type NamedType = (String, Option<String>);

positional_record! {
    extract;
    /// A model's name, relative path, compiled and authored queries, and authored SQL file.
    pub(crate) struct ModelTextsPy {
        name: String,
        relative_path: String,
        query_sql: String,
        authored_query_sql: String,
        authored_sql: String,
    }
}

/// Encoded config values and the encoder's error, if any.
pub(crate) type ConfigValues = (BTreeMap<String, Value>, Option<String>);

/// One model as the compile-input stage produced it.
#[derive(FromPyObject)]
pub struct ModelInputPy<'py>(
    ModelTextsPy,
    (Bound<'py, PyDict>, Vec<String>, Option<String>),
    Vec<(String, String, Option<String>)>,
    Vec<String>,
    (Vec<DeclarationPy<'py>>, Vec<DeclarationPy<'py>>),
);

pub(crate) fn model_facts(model: ModelInputPy<'_>) -> PyResult<CompiledModelFacts> {
    let ModelInputPy(
        ModelTextsPy {
            name,
            relative_path,
            query_sql,
            authored_query_sql,
            authored_sql,
        },
        (values, header_keys, layer_schema),
        references,
        enum_columns,
        (enum_declarations, constant_declarations),
    ) = model;
    let config_values: ConfigValues = config_values(&values);
    Ok(CompiledModelFacts {
        name,
        relative_path,
        query_sql: query_sql.into(),
        authored_query_sql: authored_query_sql.into(),
        authored_sql: authored_sql.into(),
        config: ModelConfigFacts {
            values: config_values.0,
            encoding_error: config_values.1,
            header_keys,
            layer_schema,
            contract: text_value(&values, CONTRACT_KEY)?,
            materialized: materialization(&values)?,
            incremental_mode: text_value(&values, INCREMENTAL_MODE_KEY)?,
        },
        references: references
            .into_iter()
            .map(|(kind, name, package)| ModelReferenceFact {
                kind,
                name,
                package,
            })
            .collect(),
        schema: None,
        enum_columns,
        enum_declarations: declarations(enum_declarations)?,
        constant_declarations: declarations(constant_declarations)?,
        analysis: None,
        typed_calls: Vec::new(),
        expansion_passes: Vec::new(),
        sql_analysis_skipped: false,
    })
}

/// The config values, or the encoder's error, kept for the consumer that reads them.
pub(crate) fn config_values(values: &Bound<'_, PyDict>) -> ConfigValues {
    match object_value(values) {
        Ok(Value::Object(entries)) => (entries.into_iter().collect(), None),
        Ok(_) => (BTreeMap::new(), None),
        Err(error) => (BTreeMap::new(), Some(error.to_string())),
    }
}

/// The `materialized` config value when it is text, `table` when it is absent.
pub(crate) fn materialization(values: &Bound<'_, PyDict>) -> PyResult<Option<String>> {
    if values.get_item(MATERIALIZED_KEY)?.is_none() {
        return Ok(Some(TABLE_MATERIALIZATION.to_owned()));
    }
    text_value(values, MATERIALIZED_KEY)
}

/// A config value when it is text.
pub(crate) fn text_value(values: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<String>> {
    match values.get_item(key)? {
        Some(value) if value.is_instance_of::<PyString>() => Ok(Some(value.extract()?)),
        _ => Ok(None),
    }
}

pub(crate) fn declarations(rows: Vec<DeclarationPy<'_>>) -> PyResult<Vec<DeclarationFact>> {
    rows.into_iter().map(declaration).collect()
}

pub(crate) fn declaration(row: DeclarationPy<'_>) -> PyResult<DeclarationFact> {
    let DeclarationPy(name, relative_path, members, value, value_type, render_as) = row;
    Ok(DeclarationFact {
        name,
        relative_path,
        members: members
            .into_iter()
            .map(|(name, value)| Ok((name, plain_value(&value)?)))
            .collect::<PyResult<_>>()?,
        value: Some(plain_value(&value)?).filter(|value| !value.is_null()),
        value_type,
        render_as,
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeModelInput>()?;
    module.add_class::<NativeCompiledModel>()?;
    module.add_class::<NativeProjectView>()?;
    module.add_class::<NativeInputsView>()?;
    module.add_class::<NativeSeedInput>()?;
    module.add_class::<NativeCompiledSeed>()?;
    module.add_class::<NativeSourceInput>()?;
    module.add_class::<NativeFunctionInput>()?;
    module.add_class::<NativeAuditInput>()?;
    module.add_class::<NativeCompiledAudit>()?;
    module.add_class::<NativeCompiledFunction>()?;
    module.add_class::<NativeCompiledSource>()?;
    module.add_class::<NativeCompiledProject>()
}
