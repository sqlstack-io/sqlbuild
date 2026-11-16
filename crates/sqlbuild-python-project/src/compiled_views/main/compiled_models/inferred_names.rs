//! The names of the inferred output columns, `None` when the shape is unknown.

use crate::compiled_views::_helpers::compiled_models::model_reads::column_names;
use crate::compiled_views::main::compiled_models::inferred_rows::inferred_rows;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// The names of the inferred output columns, `None` when the shape is unknown.
pub fn inferred_names(model: &Bound<'_, PyAny>) -> PyResult<Option<Vec<String>>> {
    Ok(inferred_rows(model)?.map(column_names))
}
