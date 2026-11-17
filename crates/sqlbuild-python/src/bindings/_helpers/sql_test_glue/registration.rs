//! Register the native SQL test planning and assembly functions.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

use sqlbuild_python_compile::sql_test_glue::main::register_compiled_tests::register_compiled_tests;
use sqlbuild_python_front::sql_test_glue::main::register_authored_ctes::register_authored_ctes;
use sqlbuild_python_project::sql_test_glue::main::plan_compiled_sql_test_artifacts_function::plan_compiled_sql_test_artifacts_function;
use sqlbuild_python_project::sql_test_glue::main::plan_sql_tests_function::plan_sql_tests_function;
use sqlbuild_python_project::sql_test_glue::main::plan_sql_tests_natively_function::plan_sql_tests_natively_function;
use sqlbuild_python_project::sql_test_glue::main::sql_test_plan_error_messages_function::sql_test_plan_error_messages_function;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(plan_sql_tests_function(module)?)?;
    module.add_function(plan_sql_tests_natively_function(module)?)?;
    module.add_function(plan_compiled_sql_test_artifacts_function(module)?)?;
    module.add_function(sql_test_plan_error_messages_function(module)?)?;
    register_authored_ctes(module)?;
    register_compiled_tests(module)
}
