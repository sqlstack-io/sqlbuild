//! Register native execution history storage and entry points.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

use sqlbuild_python_front::execution_history::main::register_functions::register_functions;
use sqlbuild_python_front::execution_history::models::PostgresExecutionHistory;
use sqlbuild_python_front::execution_history::models::SqliteExecutionHistory;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<SqliteExecutionHistory>()?;
    module.add_class::<PostgresExecutionHistory>()?;
    register_functions(module)
}
