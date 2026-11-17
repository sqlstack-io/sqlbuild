//! Build result text.

use crate::build_progress::_helpers::result_text::audits;
use crate::build_progress::_helpers::result_text::function_result_text;
use crate::build_progress::_helpers::result_text::load_result_text;
use crate::build_progress::_helpers::result_text::model_result_text;
use crate::build_progress::_helpers::result_text::seed_result_text;
use crate::build_progress::_helpers::result_text::test_result_text;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_core::build_progress::models::BuildResultText;
use sqlbuild_host::models::{BuildExecutionResult, BuildStatus};

/// Build result text.
pub fn build_result_text(
    py: Python<'_>,
    native: &BuildExecutionResult<Bound<'_, PyAny>>,
) -> PyResult<BuildResultText> {
    Ok(BuildResultText {
        failed: native.status == BuildStatus::Failed,
        warning_count: native.warning_count,
        insufficient_count: native.insufficient_count,
        models: native
            .model_results
            .iter()
            .map(|model| model_result_text(py, model))
            .collect::<PyResult<_>>()?,
        seeds: native.seed_results.iter().map(seed_result_text).collect(),
        functions: native
            .function_results
            .iter()
            .map(function_result_text)
            .collect(),
        loads: native.load_results.iter().map(load_result_text).collect(),
        tests: native
            .test_results
            .iter()
            .map(test_result_text)
            .collect::<PyResult<_>>()?,
        source_audits: audits(&native.source_audit_results),
        end_audits: audits(&native.end_audit_results),
    })
}
