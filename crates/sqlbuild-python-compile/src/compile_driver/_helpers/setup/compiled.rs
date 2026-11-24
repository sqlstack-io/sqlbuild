//! The finished compile as an invocation phase, with the models it compiled.

use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyTupleMethods;
use sqlbuild_python_base::invocation_phases::constants::COMPILED_PHASE;
use sqlbuild_python_base::invocation_phases::models::InvocationPhase;
use sqlbuild_python_base::native_host::main::publish_invocation_phase::publish_invocation_phase;
use sqlbuild_python_project::compiled_views::models::NativeProjectView;

/// Publish the compile of `project`, which took `seconds`; a project that is not a native view has no model count.
pub(crate) fn publish_compiled(project: &Bound<'_, PyAny>, seconds: f64) -> PyResult<()> {
    let py = project.py();
    let count: Option<i64> = match project.cast::<NativeProjectView>() {
        Ok(view) => {
            Some(i64::try_from(view.borrow().models(py).bind(py).len()).unwrap_or(i64::MAX))
        }
        Err(_) => None,
    };
    publish_invocation_phase(
        py,
        &InvocationPhase {
            phase: COMPILED_PHASE,
            duration_ms: Some(seconds * 1000.0),
            count,
            ..InvocationPhase::default()
        },
    )
}
