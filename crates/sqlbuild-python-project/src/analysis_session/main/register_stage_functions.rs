//! Register the stage functions functions and classes.

use crate::analysis_session::_helpers::stage_functions::infer_native_query_column_rows;
use crate::analysis_session::_helpers::stage_functions::prove_native_dynamic_contracts;
use crate::analysis_session::main::infer_native_expression_source_shapes::infer_native_expression_source_shapes;
use crate::analysis_session::main::infer_native_query_columns::infer_native_query_columns;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the stage functions functions and classes.
pub fn register_stage_functions(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(infer_native_query_column_rows, module)?)?;
    module.add_function(wrap_pyfunction!(infer_native_query_columns, module)?)?;
    module.add_function(wrap_pyfunction!(
        infer_native_expression_source_shapes,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(prove_native_dynamic_contracts, module)?)?;
    Ok(())
}
