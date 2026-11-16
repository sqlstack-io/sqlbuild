//! What the analysed SQL of a native model input must show for its SQL macro calls to fit.

use crate::compiled_views::models::NativeModelInput;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_analysis::sql_macros::models::SqlMacroCheck;

/// What the analysed SQL of a native model input must show for its SQL macro calls to fit.
pub fn input_sql_macro_checks(model_input: &Bound<'_, PyAny>) -> PyResult<Vec<SqlMacroCheck>> {
    if let Ok(native) = model_input.cast::<NativeModelInput>() {
        return native.get().read(model_input.py(), |record, _| {
            Ok(record.sql_macro_checks.clone())
        });
    }
    Ok(Vec::new())
}
