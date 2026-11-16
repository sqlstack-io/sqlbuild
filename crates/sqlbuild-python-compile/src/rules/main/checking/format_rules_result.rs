//! One rules result for the requested CLI surface.

use crate::rules::_helpers::checking::results::rule_findings;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_rules::rule_results::main::format_result::format_result;
use sqlbuild_rules::rule_results::models::ResultCounts;

/// One rules result for the requested CLI surface.
#[pyfunction]
#[pyo3(signature = (*, result, json_output))]
pub fn format_rules_result(result: &Bound<'_, PyAny>, json_output: bool) -> PyResult<String> {
    let counts = ResultCounts {
        evaluated_models: field(result, "evaluated_models")?.extract()?,
        unevaluated_resources: field(result, "unevaluated_resources")?.extract()?,
        cache_hits: field(result, "cache_hits")?.extract()?,
        cache_misses: field(result, "cache_misses")?.extract()?,
    };
    Ok(format_result(
        &rule_findings(&field(result, "findings")?)?,
        counts,
        json_output,
    ))
}
