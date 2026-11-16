//! Register the native `sqb rename` and `sqb mv` command and its contract-edit helpers.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::refactor::refactor_command;
use sqlbuild_python_front::refactor::main::register_contract_compare::register_contract_compare;
use sqlbuild_python_front::refactor::main::register_contract_edits::register_contract_edits;
use sqlbuild_python_front::refactor::main::register_python_strings::register_python_strings;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    refactor_command::register(module)?;
    register_contract_edits(module)?;
    register_contract_compare(module)?;
    register_python_strings(module)
}
