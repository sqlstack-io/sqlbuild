//! The analysis text of `model_input`: its compiled query with each typed SQL macro call marked.

use crate::compiled_views::models::NativeModelInput;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::sql_macros::main::typed_analysis_sql::typed_analysis_sql;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The analysis text of `model_input`: its compiled query with each typed SQL macro call marked.
pub fn input_analysis_sql(model_input: &Bound<'_, PyAny>, dialect: &str) -> PyResult<String> {
    if let Ok(native) = model_input.cast::<NativeModelInput>() {
        return native.get().read(model_input.py(), |record, _| {
            Ok(typed_analysis_sql(&record.query_sql, &record.typed_calls, dialect).0)
        });
    }
    native_field(model_input, "query_sql")?.extract()
}
