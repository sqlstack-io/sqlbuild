//! A Python column spec's fact; its location is kept when it differs from `model_path`.

use crate::compiled_project::_helpers::schema_facts::AUDITS_FIELD;
use crate::compiled_project::_helpers::schema_facts::DEFINITION_NAME_FIELD;
use crate::compiled_project::_helpers::schema_facts::DESCRIPTION_FIELD;
use crate::compiled_project::_helpers::schema_facts::LOCATION_FIELD;
use crate::compiled_project::_helpers::schema_facts::MIGRATE_FROM_FIELD;
use crate::compiled_project::_helpers::schema_facts::NAME_FIELD;
use crate::compiled_project::_helpers::schema_facts::NULLABLE_FIELD;
use crate::compiled_project::_helpers::schema_facts::PATH_FIELD;
use crate::compiled_project::_helpers::schema_facts::TYPE_FIELD;
use crate::compiled_project::_helpers::schema_facts::optional;
use crate::compiled_project::_helpers::schema_facts::present;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::compiled_project::models::SchemaColumnFact;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// A Python column spec's fact; its location is kept when it differs from `model_path`.
pub fn python_column_fact(
    column: &Bound<'_, PyAny>,
    model_path: Option<&Bound<'_, PyAny>>,
) -> PyResult<SchemaColumnFact> {
    let mut audit_definitions: Vec<String> = Vec::new();
    if let Some(audits) = present(column, AUDITS_FIELD)? {
        for audit in audits.try_iter()? {
            audit_definitions.push(native_field(&audit?, DEFINITION_NAME_FIELD)?.extract()?);
        }
    }
    let location_path: Option<String> = match present(column, LOCATION_FIELD)? {
        Some(location) => {
            let path: Bound<'_, PyAny> = native_field(&location, PATH_FIELD)?;
            let outside: bool = match model_path {
                Some(model_path) => !path.is(model_path) && path.ne(model_path)?,
                None => true,
            };
            outside
                .then(|| path.str().map(|text| text.to_string()))
                .transpose()?
        }
        None => None,
    };
    Ok(SchemaColumnFact {
        name: native_field(column, NAME_FIELD)?.str()?.extract()?,
        data_type: optional(column, TYPE_FIELD)?,
        nullable: optional(column, NULLABLE_FIELD)?,
        described: present(column, DESCRIPTION_FIELD)?.is_some(),
        migrate_from: optional(column, MIGRATE_FROM_FIELD)?,
        audit_definitions,
        location_path,
    })
}
