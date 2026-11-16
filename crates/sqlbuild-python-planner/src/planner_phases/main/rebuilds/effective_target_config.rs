//! The resolved config of the planned target, or defaults without one.

use crate::planner_phases::_helpers::inputs::runtime::runtime_field;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::contracts::main::resolve_target_config::resolve_target_config;
use sqlbuild_python_project::planner_phases::main::native::native;

/// The resolved config of the planned target, or defaults without one.
pub fn effective_target_config<'py>(runtime: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = runtime.py();
    let project_config = runtime_field(runtime, |runtime| &runtime.project_config)?;
    let local_config = runtime_field(runtime, |runtime| &runtime.local_config)?;
    let target_name = native_field(&native_field(runtime, "project")?, "effective_target_name")?;
    if project_config.is_none() || local_config.is_none() || target_name.is_none() {
        return native(py, RecordId::TargetConfig, &[]);
    }
    resolve_target_config(py, &project_config, &local_config, &target_name)
}
