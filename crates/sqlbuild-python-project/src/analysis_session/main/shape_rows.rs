//! `{relation: {column: value}}` as ordered rows, each value as `str(value)`.

use crate::analysis_session::main::text_pairs::text_pairs;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::assembly::analysis_session::types::Shapes;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;

/// `{relation: {column: value}}` as ordered rows, each value as `str(value)`.
pub fn shape_rows(shapes: &Bound<'_, PyAny>) -> PyResult<Shapes> {
    let mut rows: Shapes = Vec::new();
    for item in mapping_items(shapes)?.try_iter()? {
        let (name, shape): (String, Bound<'_, PyAny>) = item?.extract()?;
        rows.push((name, text_pairs(&shape)?));
    }
    Ok(rows)
}
