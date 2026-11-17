//! Register the relation fingerprint functions and classes.

use crate::lineage::_helpers::graph::relation_fingerprint::relation_lineage_fingerprint;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the relation fingerprint functions and classes.
pub fn register_relation_fingerprint(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(relation_lineage_fingerprint, module)?)?;
    Ok(())
}
