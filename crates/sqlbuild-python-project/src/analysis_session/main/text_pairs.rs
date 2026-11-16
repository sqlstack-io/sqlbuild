//! `{column: value}` as ordered pairs, each value as `str(value)`.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::assembly::analysis_session::types::Pairs;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;

/// `{column: value}` as ordered pairs, each value as `str(value)`.
pub fn text_pairs(shape: &Bound<'_, PyAny>) -> PyResult<Pairs> {
    let mut pairs: Pairs = Vec::new();
    for column in mapping_items(shape)?.try_iter()? {
        let (column, value): (String, Bound<'_, PyAny>) = column?.extract()?;
        pairs.push((column, value.str()?.extract()?));
    }
    Ok(pairs)
}
