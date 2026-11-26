//! Check each sources file's load with SQLLoad, naming the file and line of a setting it refuses.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use sqlbuild_load::loads::main::check_load::check_load;
use sqlbuild_load::loads::main::yaml_line::yaml_line;
use sqlbuild_load::loads::models::{LoadCheck, LoadDeclaration};
use sqlbuild_python_base::records::main::construct::new_error::new_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

use crate::compile_driver::_helpers::loads::{declaration, load_check};
use crate::compile_driver::models::ProjectConfiguration;

/// Reject the first load SQLLoad refuses, or one the project's warehouse can't hold, as a sources file error.
pub fn reject_invalid_loads(
    config: &ProjectConfiguration<'_, '_>,
    source_files: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py: Python<'_> = config.project_dir.py();
    let fail =
        |message: String| -> PyErr { new_error(py, ErrorId::SourceParse, &message, (None, None)) };
    let mut check: Option<LoadCheck> = None;
    for file in source_files.try_iter()? {
        let file: Bound<'_, PyAny> = file?;
        let load: Bound<'_, PyAny> = native_field(&file, "load")?;
        if load.is_none() {
            continue;
        }
        let shown: String = native_field(&file, "file_path")?.str()?.to_string();
        let checking: &LoadCheck = match &mut check {
            Some(checking) => checking,
            empty => empty.insert(
                load_check(config)?.map_err(|message| fail(format!("{shown} load: {message}")))?,
            ),
        };
        let declaration: LoadDeclaration = declaration(&file, &load)?;
        let Err(problem) = check_load(checking, &declaration) else {
            continue;
        };
        let contents: String = native_field(&file, "contents")?.extract()?;
        let line: String = match yaml_line(&contents, &problem.path) {
            Ok(Some(line)) => format!(":{line}"),
            Ok(None) | Err(_) => String::new(),
        };
        return Err(fail(format!(
            "{shown}{line} load '{}': {}",
            declaration.name, problem.message
        )));
    }
    Ok(())
}
