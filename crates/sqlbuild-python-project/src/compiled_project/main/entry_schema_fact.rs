//! The fact of a hand-built model's `SchemaModelEntry`, or `None` without one.

use crate::compiled_project::_helpers::schema_facts::AGGREGATE_FIELD;
use crate::compiled_project::_helpers::schema_facts::AUDITS_FIELD;
use crate::compiled_project::_helpers::schema_facts::COLUMNS_FIELD;
use crate::compiled_project::_helpers::schema_facts::DYNAMIC_COLUMNS_FIELD;
use crate::compiled_project::_helpers::schema_facts::MODEL_SCHEMA_FIELD;
use crate::compiled_project::_helpers::schema_facts::NAME_FIELD;
use crate::compiled_project::_helpers::schema_facts::NAME_PATTERN_FIELD;
use crate::compiled_project::_helpers::schema_facts::PIVOT_COLUMN_FIELD;
use crate::compiled_project::_helpers::schema_facts::TYPE_ENFORCEMENT_FIELD;
use crate::compiled_project::_helpers::schema_facts::TYPE_FIELD;
use crate::compiled_project::_helpers::schema_facts::VALUE_COLUMN_FIELD;
use crate::compiled_project::main::family_fact::family_fact;
use crate::compiled_project::main::python_column_facts::python_column_facts;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::compiled_project::models::{DynamicFamilyFact, ModelSchemaFact};
use sqlbuild_model_config::header_metadata::models::DynamicFamily;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The fact of a hand-built model's `SchemaModelEntry`, or `None` without one.
pub fn entry_schema_fact(
    entry: &Bound<'_, PyAny>,
    model_path: &Bound<'_, PyAny>,
) -> PyResult<Option<ModelSchemaFact>> {
    if entry.is_none() {
        return Ok(None);
    }
    let mut dynamic_columns: Vec<DynamicFamilyFact> = Vec::new();
    for family in native_field(entry, DYNAMIC_COLUMNS_FIELD)?.try_iter()? {
        let family: Bound<'_, PyAny> = family?;
        dynamic_columns.push(family_fact(&DynamicFamily {
            name: native_field(&family, NAME_FIELD)?.extract()?,
            pivot_column: native_field(&family, PIVOT_COLUMN_FIELD)?.extract()?,
            value_column: native_field(&family, VALUE_COLUMN_FIELD)?.extract()?,
            aggregate: native_field(&family, AGGREGATE_FIELD)?.extract()?,
            data_type: native_field(&family, TYPE_FIELD)?.extract()?,
            name_pattern: native_field(&family, NAME_PATTERN_FIELD)?.extract()?,
        }));
    }
    Ok(Some(ModelSchemaFact {
        audit_count: u32::try_from(native_field(entry, AUDITS_FIELD)?.len()?).unwrap_or(u32::MAX),
        columns: python_column_facts(&native_field(entry, COLUMNS_FIELD)?, Some(model_path))?,
        dynamic_columns,
        type_enforcement: native_field(entry, TYPE_ENFORCEMENT_FIELD)?.is_truthy()?,
        named_schema: !native_field(entry, MODEL_SCHEMA_FIELD)?.is_none(),
    }))
}
