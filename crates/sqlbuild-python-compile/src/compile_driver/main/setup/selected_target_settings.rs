//! The selected target's settings, with its database and schema resolved as `sqb debug` resolves them.

use crate::compile_attachment::main::effective_target_namespace::effective_target_namespace;
use crate::compile_driver::_helpers::setup::targets::target_settings;
use crate::compile_driver::models::TargetSettingsValues;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyTuple;

/// The selected target's settings, with its database and schema resolved as `sqb debug` resolves them.
pub fn selected_target_settings<'py>(
    (discovered, selected_target): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    adapter_name: &str,
    (connection, effective_vars, adapter): (
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
    ),
) -> PyResult<TargetSettingsValues> {
    let namespace: Bound<'py, PyTuple> = effective_target_namespace(
        (discovered, selected_target),
        (adapter_name, connection, adapter),
    )?;
    target_settings(
        (discovered, &namespace.get_item(0)?),
        (
            namespace.get_item(2)?.extract()?,
            namespace.get_item(3)?.extract()?,
        ),
        (connection, effective_vars, adapter),
    )
}
