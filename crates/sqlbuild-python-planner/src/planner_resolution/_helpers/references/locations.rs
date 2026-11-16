//! Location maps retaining the original compiled Python destination objects.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_project::resource_views::main::resource_reads::destination::destination;
use sqlbuild_python_project::resource_views::main::resource_reads::name::name;

pub(crate) fn locations<'py>(items: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let result = PyDict::new(items.py());
    for item in items.try_iter()? {
        let item = item?;
        result.set_item(name(&item)?, destination(&item)?)?;
    }
    Ok(result)
}
