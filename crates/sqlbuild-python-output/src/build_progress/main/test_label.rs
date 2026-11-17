//! The label of one test or result: its name, plus source and values for parameterized cases.

use crate::build_progress::_helpers::result_text::test_label_values;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::native_host::types::ReadResult;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The label of one test or result: its name, plus source and values for parameterized cases.
pub fn test_label(item: &Bound<'_, PyAny>, name_field: &str) -> PyResult<String> {
    let name: String = native_field(item, name_field)?.extract()?;
    let source_path: Bound<'_, PyAny> = item.getattr("source_path")?;
    let values = Vec::<(String, Bound<'_, PyAny>)>::read(&native_field(item, "parameter_values")?)?;
    if source_path.is_none() || values.is_empty() {
        return Ok(name);
    }
    let path: String = source_path.call_method0("as_posix")?.extract()?;
    let declarations = Vec::<Bound<'_, PyAny>>::read(&item.getattr("parameter_schema")?)?;
    test_label_values(&name, Some(&path), &declarations, &values)
}
