//! Register the publication functions and classes.

use crate::freshness::_helpers::publication::append_eligible_direct_source_freshness_records;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the publication functions and classes.
pub fn register_publication(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        append_eligible_direct_source_freshness_records,
        module
    )?)
}
