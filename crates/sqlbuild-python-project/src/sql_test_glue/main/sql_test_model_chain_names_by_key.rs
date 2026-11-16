//! Return model closures resolved natively in one project batch, keyed by SQL test key.

use crate::sql_test_glue::_helpers::plan::plan_driver::resolve_sql_test_model_chains;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, pyfunction};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Return model closures resolved natively in one project batch, keyed by SQL test key.
#[pyfunction]
#[pyo3(signature = (*, project, tests))]
pub fn sql_test_model_chain_names_by_key<'py>(
    project: &Bound<'py, PyAny>,
    tests: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py = project.py();
    let chains = resolve_sql_test_model_chains(project, tests)?;
    let by_key = PyDict::new(py);
    for (test, chain) in tests.try_iter()?.zip(chains.try_iter()?) {
        by_key.set_item(native_field(&test?, "key")?, chain?)?;
    }
    Ok(by_key)
}
