//! Check each `@loader`'s options with SQLLoad, as the Python table it runs as.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use sqlbuild_load::loads::main::check_loader::check_loader;
use sqlbuild_load::loads::models::LoadCheck;
use sqlbuild_python_base::records::main::construct::new_error::new_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

use crate::compile_driver::_helpers::loaders::loader_declaration;
use crate::compile_driver::_helpers::loads::load_check;
use crate::compile_driver::models::ProjectConfiguration;

/// Reject the first loader SQLLoad refuses, or any loader when the project's warehouse can't hold one.
pub fn reject_invalid_loaders(
    config: &ProjectConfiguration<'_, '_>,
    inputs: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py: Python<'_> = config.project_dir.py();
    let fail = |message: String| -> PyErr {
        new_error(py, ErrorId::PythonNodeDiscovery, &message, (None, None))
    };
    let source_files: Bound<'_, PyAny> = native_field(inputs, "source_files")?;
    let mut check: Option<LoadCheck> = None;
    for loader in native_field(inputs, "loader_functions")?.try_iter()? {
        let loader: Bound<'_, PyAny> = loader?;
        let (declaration, shown) = loader_declaration(&loader, &source_files)?;
        let place: String = format!("{shown} @loader '{}'", declaration.name);
        let checking: &LoadCheck = match &mut check {
            Some(checking) => checking,
            empty => empty.insert(
                load_check(config)?.map_err(|message| fail(format!("{place}: {message}")))?,
            ),
        };
        if let Err(problem) = check_loader(checking, &declaration) {
            return Err(fail(format!("{place}: {}", problem.message)));
        }
    }
    Ok(())
}
