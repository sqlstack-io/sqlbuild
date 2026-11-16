//! `compile_facts` as a function of `module`.

use crate::rules::_helpers::fix::fix_facts::compile_facts;

/// `compile_facts` as a function of `module`.
pub fn compile_facts_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(compile_facts, module)
}
