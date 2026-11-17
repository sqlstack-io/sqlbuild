//! The target path of one compiled SQL test from its chain's model names.

use crate::compile_outputs::_helpers::runtime_target::path_facts;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_analysis::target_paths::main::sql_test_output_path::sql_test_output_path;
use std::path::PathBuf;

/// The target path of one compiled SQL test from its chain's model names.
pub fn compiled_sql_test_path(
    test: &Bound<'_, PyAny>,
    model_names: Vec<String>,
) -> PyResult<PathBuf> {
    Ok(PathBuf::from(sql_test_output_path(&path_facts(
        test,
        model_names,
    )?)))
}
