//! Register the project functions and classes.

use crate::compiled_project::models::NativeCompiledProject;
use crate::compiled_views::models::NativeCompiledModel;
use crate::compiled_views::models::NativeInputsView;
use crate::compiled_views::models::NativeModelInput;
use crate::compiled_views::models::NativeProjectView;
use crate::resource_views::models::{NativeAuditInput, NativeCompiledAudit};
use crate::resource_views::models::{NativeCompiledFunction, NativeFunctionInput};
use crate::resource_views::models::{NativeCompiledSeed, NativeSeedInput};
use crate::resource_views::models::{NativeCompiledSource, NativeSourceInput};
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the project functions and classes.
pub fn register_project(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeModelInput>()?;
    module.add_class::<NativeCompiledModel>()?;
    module.add_class::<NativeProjectView>()?;
    module.add_class::<NativeInputsView>()?;
    module.add_class::<NativeSeedInput>()?;
    module.add_class::<NativeCompiledSeed>()?;
    module.add_class::<NativeSourceInput>()?;
    module.add_class::<NativeFunctionInput>()?;
    module.add_class::<NativeAuditInput>()?;
    module.add_class::<NativeCompiledAudit>()?;
    module.add_class::<NativeCompiledFunction>()?;
    module.add_class::<NativeCompiledSource>()?;
    module.add_class::<NativeCompiledProject>()
}
