//! Register the native project assembly functions.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

use sqlbuild_python_compile::project_assembly::main::register_native_resources::register_native_resources;
use sqlbuild_python_compile::project_assembly::main::register_resources::register_resources;
use sqlbuild_python_compile::project_assembly::main::register_syntax_checks::register_syntax_checks;
use sqlbuild_python_compile::project_assembly::main::register_syntax_validation::register_syntax_validation;
use sqlbuild_python_compile::project_assembly::models::NativeModelFacts;
use sqlbuild_python_compile::project_assembly::models::NativeProjectFacts;
use sqlbuild_python_compile::project_assembly::models::NativeProjectResources;
use sqlbuild_python_compile::project_assembly::models::NativeResourceFailure;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeProjectResources>()?;
    module.add_class::<NativeResourceFailure>()?;
    module.add_class::<NativeModelFacts>()?;
    module.add_class::<NativeProjectFacts>()?;
    register_native_resources(module)?;
    register_syntax_validation(module)?;
    register_syntax_checks(module)?;
    register_resources(module)
}
