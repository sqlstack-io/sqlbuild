//! Add the planning handle and entry points, and join planning threads at interpreter exit.

use crate::planner_entries::_helpers::plans::test_planning;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Add the planning handle and entry points, and join planning threads at interpreter exit.
#[inline]
pub fn register_test_planning(module: &Bound<'_, PyModule>) -> PyResult<()> {
    test_planning::register(module)
}
