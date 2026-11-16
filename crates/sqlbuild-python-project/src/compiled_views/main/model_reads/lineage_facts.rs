//! Each lineage output with its transform kind and confidence values, `None` without lineage.

use crate::compiled_views::_helpers::model_reads;
use crate::compiled_views::models::LineageFactRow;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Each lineage output with its transform kind and confidence values, `None` without lineage.
#[inline]
pub fn lineage_facts(model: &Bound<'_, PyAny>) -> PyResult<Option<Vec<LineageFactRow>>> {
    model_reads::lineage_facts(model)
}
