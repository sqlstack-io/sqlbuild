//! Register the session functions and classes.

use crate::analysis_session::models::NativeModelAnalysisSession;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the session functions and classes.
pub fn register_session(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeModelAnalysisSession>()?;
    Ok(())
}
