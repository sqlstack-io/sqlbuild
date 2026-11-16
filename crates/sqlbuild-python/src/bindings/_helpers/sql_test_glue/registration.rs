//! Register the native SQL test planning and assembly functions.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

use crate::bindings::_helpers::sql_test_glue::compiled_tests;
use sqlbuild_python_front::sql_test_glue::main::register_authored_ctes::register_authored_ctes;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        super::plan_assembly::plan_sql_tests,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        super::plan_driver::plan_sql_tests_natively,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        super::plan_driver::plan_compiled_sql_test_artifacts,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        super::plan_driver::sql_test_plan_error_messages,
        module
    )?)?;
    register_authored_ctes(module)?;
    compiled_tests::register(module)
}
