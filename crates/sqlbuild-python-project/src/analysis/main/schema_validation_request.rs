//! A `SqlSchemaValidationRequest` over the normalized analysis SQL of `parts.query_sql`.

use crate::analysis::_helpers::schema_request::GENERIC_DIALECT;
use crate::analysis::models::SchemaRequestParts;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDict;
use sqlbuild_analysis::semantic_validation::main::normalize::normalize_analysis_sql;
use sqlbuild_analysis::semantic_validation::models::NormalizationInput;
use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_base::types::CompilerDetach;
use std::collections::HashMap;

/// A `SqlSchemaValidationRequest` over the normalized analysis SQL of `parts.query_sql`.
pub fn schema_validation_request<'py>(
    py: Python<'py>,
    parts: SchemaRequestParts<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let dialect: String = if parts.dialect.is_truthy()? {
        parts.dialect.extract()?
    } else {
        GENERIC_DIALECT.to_owned()
    };
    let placeholders: HashMap<String, String> = if parts.placeholders.is_truthy()? {
        parts.placeholders.extract()?
    } else {
        HashMap::new()
    };
    let input: NormalizationInput = NormalizationInput {
        sql: parts.query_sql,
        dialect,
        stubs: HashMap::new(),
        placeholders,
    };
    let sql: String = py
        .compiler_detach(|| normalize_analysis_sql(input))
        .map_err(value_error)?;
    let arguments: Bound<'py, PyDict> = PyDict::new(py);
    arguments.set_item("sql", sql)?;
    arguments.set_item("dialect", parts.dialect)?;
    arguments.set_item("schema", parts.schema)?;
    arguments.set_item("known_functions", parts.known_functions)?;
    arguments.set_item("known_types", parts.known_types)?;
    arguments.set_item(
        "quoted_identifiers_ignore_case",
        parts.quoted_identifiers_ignore_case,
    )?;
    arguments.set_item("catalog", parts.catalog)?;
    new_record(
        py,
        RecordId::SqlSchemaValidationRequest,
        (),
        Some(&arguments),
    )
}
