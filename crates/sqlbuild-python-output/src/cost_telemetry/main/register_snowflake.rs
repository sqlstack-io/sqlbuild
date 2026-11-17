//! Register the snowflake functions and classes.

use crate::cost_telemetry::_helpers::snowflake::collect_snowflake_cost;
use crate::cost_telemetry::_helpers::snowflake::parse_sqlbuild_query_tag;
use crate::cost_telemetry::_helpers::snowflake::render_snowflake_query_history_sql;
use crate::cost_telemetry::_helpers::snowflake::snowflake_query_observations;
use crate::cost_telemetry::_helpers::snowflake::snowflake_statement_is_metadata_only;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the snowflake functions and classes.
pub fn register_snowflake(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(collect_snowflake_cost, module)?)?;
    module.add_function(wrap_pyfunction!(snowflake_query_observations, module)?)?;
    module.add_function(wrap_pyfunction!(
        render_snowflake_query_history_sql,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        snowflake_statement_is_metadata_only,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(parse_sqlbuild_query_tag, module)?)?;
    Ok(())
}
