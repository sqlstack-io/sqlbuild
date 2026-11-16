//! Add the planning handle and entry points, and join planning threads at interpreter exit.

use crate::planner_entries::_helpers::plans::test_planning::SqlTestPlanning;
use crate::planner_entries::_helpers::plans::test_planning::join_sql_test_planning_threads;
use crate::planner_entries::_helpers::plans::test_planning::start_sql_test_planning;
use pyo3::prelude::{Bound, PyAnyMethods, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;
use sqlbuild_python_base::boundary::main::imported::imported;

/// Add the planning handle and entry points, and join planning threads at interpreter exit.
pub fn register_test_planning(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<SqlTestPlanning>()?;
    module.add_function(wrap_pyfunction!(start_sql_test_planning, module)?)?;
    let hook = wrap_pyfunction!(join_sql_test_planning_threads, module)?;
    module.add_function(hook.clone())?;
    imported(module.py(), "atexit", "register")?.call1((hook,))?;
    Ok(())
}
