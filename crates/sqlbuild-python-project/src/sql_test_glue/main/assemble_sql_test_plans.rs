//! Plan results of SQL tests, with fixture and expected-output diagnostics in place of failing plans.

use crate::sql_test_glue::_helpers::plan::plan_assembly;
use crate::sql_test_glue::models::SqlTestAssembly;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyTuple;

/// Plan results of SQL tests, with fixture and expected-output diagnostics in place of failing plans.
#[inline]
pub fn assemble_sql_test_plans<'py>(
    tests: &[Bound<'py, PyAny>],
    (project, adapter): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    assembly: &SqlTestAssembly<'py>,
) -> PyResult<Bound<'py, PyTuple>> {
    plan_assembly::assemble_sql_test_plans(tests, (project, adapter), assembly)
}
