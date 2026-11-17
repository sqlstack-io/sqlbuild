//! The shared progress/footer/command label of an already-converted executor test result.

use crate::build_progress::_helpers::result_text::test_label_values;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_host::models::SqlTestExecutionResult;

/// The shared progress/footer/command label of an already-converted executor test result.
pub fn test_result_label(test: &SqlTestExecutionResult<Bound<'_, PyAny>>) -> PyResult<String> {
    test_label_values(
        &test.test_name,
        test.source_path
            .as_ref()
            .map(|path| {
                path.to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/")
            })
            .as_deref(),
        &test.parameter_schema,
        &test.parameter_values,
    )
}
