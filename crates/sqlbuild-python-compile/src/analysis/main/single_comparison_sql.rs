//! The comparison SQL of one planned SQL test under `adapter`'s operator and dialect.

use crate::analysis::_helpers::sql_test_comparisons::RenderSettings;
use crate::analysis::_helpers::sql_test_comparisons::rendered_comparisons;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::boundary::main::value_error::value_error;
use std::collections::HashMap;

/// The comparison SQL of one planned SQL test under `adapter`'s operator and dialect.
pub fn single_comparison_sql(
    entry: &Bound<'_, PyAny>,
    adapter: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let settings = RenderSettings {
        set_difference_operator: adapter
            .call_method0("render_set_difference_operator")?
            .extract()?,
        sql_analysis_dialect: adapter.call_method0("sql_analysis_dialect")?.extract()?,
        probe_step_index: None,
        output_step_index: None,
        actual_relations: HashMap::new(),
    };
    rendered_comparisons(entry.py(), std::slice::from_ref(entry), &settings)?
        .into_iter()
        .next()
        .ok_or_else(|| value_error("native SQL-test rendering returned an invalid batch response"))
}
