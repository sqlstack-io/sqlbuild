//! One model query with canonical intrinsic calls, raising when the model may not use them.

use crate::attachments::_helpers::cursor_intrinsics::INCREMENTAL_MATERIALIZATION;
use crate::attachments::_helpers::cursor_intrinsics::text_value;
use crate::attachments::main::compile_input_error::compile_input_error;
use crate::attachments::main::reserved_cursor_markers::reserved_cursor_markers;
use pyo3::Python;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_attachments::cursor_intrinsics::main::validated_model_intrinsics::validated_model_intrinsics;
use sqlbuild_attachments::cursor_intrinsics::models::IntrinsicModel;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;

/// One model query with canonical intrinsic calls, raising when the model may not use them.
pub fn validated_model_cursor_sql(
    py: Python<'_>,
    sql: &str,
    (model_name, config_values): (&str, &Bound<'_, PyAny>),
) -> PyResult<String> {
    let incremental: bool = text_value(&mapping_get(config_values, "materialized")?)
        .is_some_and(|materialized| materialized == INCREMENTAL_MATERIALIZATION);
    let cursor: Option<String> = text_value(&mapping_get(config_values, "cursor")?);
    let reserved_markers: Vec<String> = reserved_cursor_markers();
    let outcome: Result<String, String> = compiler_guard(|| {
        Ok(validated_model_intrinsics(
            sql,
            &reserved_markers,
            &IntrinsicModel {
                name: model_name,
                incremental,
                cursor: cursor.as_deref(),
            },
        ))
    })?;
    outcome.or_else(|message| Err(compile_input_error(py, message)?))
}
