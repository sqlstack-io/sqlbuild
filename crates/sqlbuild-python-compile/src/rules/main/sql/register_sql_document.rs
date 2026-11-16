//! Register the sql document functions and classes.

use crate::rules::_helpers::sql::sql_document::SqlDocument;
use crate::rules::_helpers::sql::sql_document::SqlTreeNode;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the sql document functions and classes.
pub fn register_sql_document(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<SqlDocument>()?;
    module.add_class::<SqlTreeNode>()?;
    Ok(())
}
