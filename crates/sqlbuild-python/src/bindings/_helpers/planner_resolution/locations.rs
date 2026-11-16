//! Location maps retaining the original compiled Python destination objects.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_python_project::resource_views::main::resource_reads::destination::destination;
use sqlbuild_python_project::resource_views::main::resource_reads::name::name;

fn locations<'py>(items: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let result = PyDict::new(items.py());
    for item in items.try_iter()? {
        let item = item?;
        result.set_item(name(&item)?, destination(&item)?)?;
    }
    Ok(result)
}

#[pyfunction]
pub(crate) fn build_model_locations<'py>(
    models: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    locations(models)
}

#[pyfunction]
pub(crate) fn build_seed_locations<'py>(seeds: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    locations(seeds)
}

#[pyfunction]
pub(crate) fn build_function_locations<'py>(
    functions: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    locations(functions)
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_model_locations, module)?)?;
    module.add_function(wrap_pyfunction!(build_seed_locations, module)?)?;
    module.add_function(wrap_pyfunction!(build_function_locations, module)?)
}
