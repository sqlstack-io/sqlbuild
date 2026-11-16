//! Native-only pivot requests; the public wrapper alone projects contract records.

use crate::project_assembly::_helpers::syntax_checks::PivotModelRows;
use crate::project_assembly::_helpers::syntax_checks::analysis_sql;
use crate::project_assembly::_helpers::syntax_checks::model_name;
use crate::project_assembly::main::model_placeholders::model_placeholders;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyDictMethods;
use sqlbuild_python_project::compiled_views::main::model_inputs::attached_schema::attached_schema;
use sqlbuild_sqltext::compiler::main::placeholder_defaults::placeholder_defaults;

/// Native-only pivot requests; the public wrapper alone projects contract records.
pub fn standalone_pivot_rows(
    model_inputs: &Bound<'_, PyAny>,
    analysed: &Bound<'_, PyAny>,
) -> PyResult<PivotModelRows> {
    let mut models: PivotModelRows = Vec::new();
    for (index, model_input) in model_inputs.try_iter()?.enumerate() {
        let model_input = model_input?;
        let Some(schema) = attached_schema(&model_input)? else {
            continue;
        };
        let families = schema.dynamic_columns;
        if families.is_empty() || analysed.contains(model_name(&model_input)?)? {
            continue;
        }
        let query_sql: String = analysis_sql(&model_input)?.extract()?;
        let sql: String = match model_placeholders(&model_input)? {
            Some(placeholders) if !placeholders.is_empty() => {
                let pairs: Vec<(String, String)> = placeholders
                    .iter()
                    .map(|(key, value)| Ok((key.extract()?, value.extract()?)))
                    .collect::<PyResult<_>>()?;
                placeholder_defaults(&query_sql, &pairs)
            }
            _ => query_sql,
        };
        models.push((index, (sql, families)));
    }
    Ok(models)
}
