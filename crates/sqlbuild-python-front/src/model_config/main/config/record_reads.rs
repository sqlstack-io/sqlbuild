//! Record native template environment and context reads for project reuse, in order.

use crate::model_config::_helpers::config::template_data;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Record native template environment and context reads for project reuse, in order.
#[inline]
pub fn record_reads<'a>(
    py: Python<'_>,
    reads: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> PyResult<()> {
    template_data::record_reads(py, reads)
}
