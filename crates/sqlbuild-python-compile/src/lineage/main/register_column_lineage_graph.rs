//! Register the column lineage graph functions and classes.

use crate::lineage::models::ProjectColumnLineage;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the column lineage graph functions and classes.
pub fn register_column_lineage_graph(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<ProjectColumnLineage>()
}
