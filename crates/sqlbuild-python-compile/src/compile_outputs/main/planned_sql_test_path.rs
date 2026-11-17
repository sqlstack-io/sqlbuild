//! The target path of one planned SQL test, relative to its artifact directory.

use crate::compile_outputs::_helpers::runtime_target::path_facts;
use crate::compile_outputs::_helpers::runtime_target::path_object;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::target_paths::main::sql_test_output_path::sql_test_output_path;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The target path of one planned SQL test, relative to its artifact directory.
pub fn planned_sql_test_path<'py>(entry: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let mut model_names: Vec<String> = Vec::new();
    for step in native_field(entry, "chain")?.try_iter()? {
        model_names.push(native_field(&step?, "model_name")?.extract()?);
    }
    path_object(
        entry.py(),
        &sql_test_output_path(&path_facts(entry, model_names)?),
    )
}
