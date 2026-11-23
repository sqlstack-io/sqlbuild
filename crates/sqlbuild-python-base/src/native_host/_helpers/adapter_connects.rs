//! The adapter connects in progress, found on the running threads' stacks.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};

use crate::boundary::main::imported::imported;
use crate::native_host::constants::{ADAPTER_BASE_CLASS, ADAPTER_BASE_MODULE, CONNECT_METHOD};

/// Each running frame of an adapter's `connect`; none before the adapters are imported.
pub(crate) fn connects_in_progress(py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
    let modules: Bound<'_, PyAny> = py.import("sys")?.getattr("modules")?;
    let module: Bound<'_, PyAny> = modules.call_method1("get", (ADAPTER_BASE_MODULE,))?;
    let Some(adapter) = module.getattr_opt(ADAPTER_BASE_CLASS)? else {
        return Ok(Vec::new());
    };
    let stacks: Bound<'_, PyAny> = imported(py, "sys", "_current_frames")?
        .call0()?
        .call_method0("values")?;
    let mut connecting: Vec<Py<PyAny>> = Vec::new();
    for top in stacks.try_iter()? {
        let mut frame: Bound<'_, PyAny> = top?;
        while !frame.is_none() {
            if frame
                .getattr("f_code")?
                .getattr("co_name")?
                .eq(CONNECT_METHOD)?
            {
                let receiver: Bound<'_, PyAny> =
                    frame.getattr("f_locals")?.call_method1("get", ("self",))?;
                if receiver.is_instance(&adapter)? {
                    connecting.push(frame.clone().unbind());
                }
            }
            frame = frame.getattr("f_back")?;
        }
    }
    Ok(connecting)
}
