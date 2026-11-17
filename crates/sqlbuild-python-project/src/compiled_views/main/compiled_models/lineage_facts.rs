//! Each lineage output with its transform kind and confidence values, `None` without lineage.

use crate::compiled_views::_helpers::compiled_models::model_reads::CONFIDENCE_VALUES;
use crate::compiled_views::_helpers::compiled_models::model_reads::TRANSFORM_VALUES;
use crate::compiled_views::_helpers::compiled_models::model_reads::coded;
use crate::compiled_views::main::compiled_models::native_model::native_model;
use crate::compiled_views::models::LineageFactRow;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Each lineage output with its transform kind and confidence values, `None` without lineage.
pub fn lineage_facts(model: &Bound<'_, PyAny>) -> PyResult<Option<Vec<LineageFactRow>>> {
    if let Some(view) = native_model(model)? {
        let Some(rows) = view.lineage_rows(model.py())? else {
            return Ok(None);
        };
        let mut facts: Vec<LineageFactRow> = Vec::with_capacity(rows.len());
        for row in rows {
            facts.push(LineageFactRow {
                output_column: row.output_column,
                transform: coded(&TRANSFORM_VALUES, row.transform_code)?,
                confidence: coded(&CONFIDENCE_VALUES, row.confidence_code)?,
                sources: row.sources,
            });
        }
        return Ok(Some(facts));
    }
    let columns: Bound<'_, PyAny> = native_field(model, "fast_lineage_columns")?;
    if columns.is_none() {
        return Ok(None);
    }
    let mut facts: Vec<LineageFactRow> = Vec::new();
    for output in columns.try_iter()? {
        let output: Bound<'_, PyAny> = output?;
        let mut sources: Vec<(String, String, String)> = Vec::new();
        for source in native_field(&output, "upstream_columns")?.try_iter()? {
            let source: Bound<'_, PyAny> = source?;
            sources.push((
                native_field(&source, "resource_type")?.extract()?,
                native_field(&source, "resource_name")?.extract()?,
                native_field(&source, "column_name")?.extract()?,
            ));
        }
        facts.push(LineageFactRow {
            output_column: native_field(&output, "output_column")?.extract()?,
            transform: native_field(&output, "transform_kind")?.str()?.extract()?,
            confidence: native_field(&output, "confidence")?.str()?.extract()?,
            sources,
        });
    }
    Ok(Some(facts))
}
