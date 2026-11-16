//! The compile's natively owned project, which producers record into and consumers read.

use crate::compiled_project::models::DeclarationPy;
use sqlbuild_python_base::positional_record;
use std::collections::BTreeMap;

use pyo3::prelude::PyResult;
use serde_json::Value;
use sqlbuild_analysis::compiled_project::models::DeclarationFact;

use crate::compiled_project::_helpers::values::plain_value;

pub(crate) const INPUT_CLASSES_UNSET: &str = "no model loop recorded inputs into this project";
pub(crate) const ATTACHMENT_LOOP_UNSET: &str =
    "no model loop was kept to attach this project's resources";

pub(crate) const MATERIALIZED_KEY: &str = "materialized";
pub(crate) const TABLE_MATERIALIZATION: &str = "table";

/// `(name, type)`.
pub(crate) type NamedType = (String, Option<String>);

positional_record! {
    extract;
    /// A model's name, relative path, compiled and authored queries, and authored SQL file.
    #[derive(Debug)]
    pub(crate) struct ModelTextsPy {
        pub(crate) name: String,
        pub(crate) relative_path: String,
        pub(crate) query_sql: String,
        pub(crate) authored_query_sql: String,
        pub(crate) authored_sql: String,
    }
}

/// Encoded config values and the encoder's error, if any.
pub(crate) type ConfigValues = (BTreeMap<String, Value>, Option<String>);

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
