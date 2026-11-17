//! Model facts.

use crate::compiled_project::_helpers::project::ConfigValues;
use crate::compiled_project::_helpers::project::ModelTextsPy;
use crate::compiled_project::constants::CONTRACT_KEY;
use crate::compiled_project::constants::INCREMENTAL_MODE_KEY;
use crate::compiled_project::main::config_values::config_values;
use crate::compiled_project::main::declarations::declarations;
use crate::compiled_project::main::materialization::materialization;
use crate::compiled_project::models::ModelInputPy;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::compiled_project::models::{
    CompiledModelFacts, ModelConfigFacts, ModelReferenceFact,
};
use sqlbuild_python_base::boundary::main::text_item::text_item;

/// Model facts.
pub fn model_facts(model: ModelInputPy<'_>) -> PyResult<CompiledModelFacts> {
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
            contract: text_item(&values, CONTRACT_KEY)?,
            materialized: materialization(&values)?,
            incremental_mode: text_item(&values, INCREMENTAL_MODE_KEY)?,
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
