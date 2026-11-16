//! Rules config.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_rules::dsl_headers::models::HeaderRulesConfig;

/// Rules config.
pub fn rules_config(config: &Bound<'_, PyAny>) -> PyResult<HeaderRulesConfig> {
    Ok(HeaderRulesConfig {
        line_width: native_field(config, "line_width")?.extract()?,
        max_description_lines: native_field(config, "max_description_lines")?.extract()?,
    })
}
