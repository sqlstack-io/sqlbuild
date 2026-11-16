//! Register the cost context classes.

use crate::cost_context::models::{CostContext, CostScope};
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the cost context classes.
pub fn register_cost_context(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<CostContext>()?;
    module.add_class::<CostScope>()?;
    Ok(())
}
