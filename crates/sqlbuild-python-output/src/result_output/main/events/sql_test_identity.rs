//! `sql_test_resource_id` of one result or plan entry carrying `test_name_field` and case fields.

use crate::result_output::_helpers::events::resource_ids::sql_test_resource_id;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `sql_test_resource_id` of one result or plan entry carrying `test_name_field` and case fields.
pub fn sql_test_identity(result: &Bound<'_, PyAny>, test_name_field: &str) -> PyResult<String> {
    sql_test_resource_id(
        &native_field(result, test_name_field)?.extract::<String>()?,
        native_field(result, "source_path")?,
        native_field(result, "block_index")?.extract()?,
        native_field(result, "case_name")?.extract()?,
    )
}
