//! `plan_rule_fixes` as a function of `module`.

use crate::rules::main::fix::plan_rule_fixes::plan_rule_fixes;

/// `plan_rule_fixes` as a function of `module`.
pub fn plan_rule_fixes_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(plan_rule_fixes, module)
}
