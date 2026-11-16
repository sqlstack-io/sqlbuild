//! One model's change and backfill.

use crate::planner_changes::_helpers::detect::model_change;
use crate::planner_changes::models::ModelPass;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::models::KeywordArguments;

/// One model's change and backfill.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn detect_model_changes<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "detect_model_changes",
        options,
        (
            &[
                "model",
                "snapshot",
                "sql_analysis_enabled",
                "query_change_tracking",
                "full_refresh",
            ],
            &[
                "function_local_hashes",
                "hook_version_hashes",
                "expected_version_hash",
                "expected_metadata_json",
            ],
        ),
    )?;
    let pass = ModelPass {
        snapshot: arguments.value("snapshot")?,
        query_change_tracking: arguments.flag("query_change_tracking")?,
        full_refresh: arguments.flag("full_refresh")?,
        function_local_hashes: arguments.value("function_local_hashes")?,
        hook_version_hashes: arguments.value("hook_version_hashes")?,
    };
    model_change(
        &pass,
        &arguments.value("model")?,
        (
            arguments.value("expected_version_hash")?.extract()?,
            arguments.value("expected_metadata_json")?.extract()?,
        ),
    )
}
