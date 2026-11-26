//! Each sources file's load as SQLLoad reads it, with the file that declares it.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_load::loads::models::LoadDeclaration;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

use crate::compile_driver::_helpers::loads::declaration;

/// Each declared load of `source_files`: the file, its `DiscoveredLoad` and its declaration, in file order.
pub fn discovered_loads<'py>(
    source_files: &Bound<'py, PyAny>,
) -> PyResult<Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>, LoadDeclaration)>> {
    let mut loads: Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>, LoadDeclaration)> = Vec::new();
    for file in source_files.try_iter()? {
        let file: Bound<'py, PyAny> = file?;
        let load: Bound<'py, PyAny> = native_field(&file, "load")?;
        if load.is_none() {
            continue;
        }
        let declared: LoadDeclaration = declaration(&file, &load)?;
        loads.push((file, load, declared));
    }
    Ok(loads)
}
