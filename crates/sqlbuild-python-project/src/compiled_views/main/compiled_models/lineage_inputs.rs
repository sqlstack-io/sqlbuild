//! Each lineage output with the `(resource, column)` pairs it reads; empty without lineage.

use crate::compiled_views::main::compiled_models::native_model::native_model;
use crate::compiled_views::types::LineageInput;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Each lineage output with the `(resource, column)` pairs it reads; empty without lineage.
pub fn lineage_inputs(model: &Bound<'_, PyAny>) -> PyResult<Vec<LineageInput>> {
    if let Some(view) = native_model(model)? {
        return Ok(view.lineage_inputs(model.py())?.unwrap_or_default());
    }
    let columns: Bound<'_, PyAny> = native_field(model, "fast_lineage_columns")?;
    if !columns.is_truthy()? {
        return Ok(Vec::new());
    }
    let mut lineage: Vec<LineageInput> = Vec::new();
    for output in columns.try_iter()? {
        let output: Bound<'_, PyAny> = output?;
        let mut upstream: Vec<(String, String)> = Vec::new();
        for source in native_field(&output, "upstream_columns")?.try_iter()? {
            let source: Bound<'_, PyAny> = source?;
            upstream.push((
                native_field(&source, "resource_name")?.extract()?,
                native_field(&source, "column_name")?.extract()?,
            ));
        }
        lineage.push((native_field(&output, "output_column")?.extract()?, upstream));
    }
    Ok(lineage)
}
