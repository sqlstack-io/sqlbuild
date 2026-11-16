//! Register the sql test records functions and classes.

use crate::discovery::_helpers::records::sql_test_records::parse_sql_scenario_file;
use crate::discovery::main::records::discover_scenario_files::discover_scenario_files;
use crate::discovery::main::records::discover_test_files::discover_test_files;
use crate::discovery::main::records::parse_sql_test_file::parse_sql_test_file;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the sql test records functions and classes.
pub fn register_sql_test_records(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(discover_test_files, module)?)?;
    module.add_function(wrap_pyfunction!(discover_scenario_files, module)?)?;
    module.add_function(wrap_pyfunction!(parse_sql_test_file, module)?)?;
    module.add_function(wrap_pyfunction!(parse_sql_scenario_file, module)?)
}
