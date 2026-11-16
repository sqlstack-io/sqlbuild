//! Register the sql test comparisons functions and classes.

use crate::analysis::_helpers::sql_test_comparisons::build_scenario_expected_comparison_sql;
use crate::analysis::_helpers::sql_test_comparisons::describe_missing_expected_columns;
use crate::analysis::_helpers::sql_test_comparisons::render_sql_test_comparison_sql;
use crate::analysis::_helpers::sql_test_comparisons::render_sql_test_difference_sample_sql;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the sql test comparisons functions and classes.
pub fn register_sql_test_comparisons(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(render_sql_test_comparison_sql, module)?)?;
    module.add_function(wrap_pyfunction!(
        render_sql_test_difference_sample_sql,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(describe_missing_expected_columns, module)?)?;
    module.add_function(wrap_pyfunction!(
        build_scenario_expected_comparison_sql,
        module
    )?)?;
    Ok(())
}
