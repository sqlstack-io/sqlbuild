//! Write the planner's `PlanOutput` when SQLBUILD_PLANNER_CAPTURE_DIR names a directory.

use crate::frontier::_helpers::stage_capture::CAPTURE_SUFFIX;
use crate::frontier::_helpers::stage_capture::PLAN_CAPTURE_DIR_ENV_VAR;
use crate::frontier::_helpers::stage_capture::PLAN_CAPTURE_NAME;
use crate::frontier::_helpers::stage_capture::capture_number;
use crate::frontier::_helpers::stage_capture::write_capture_file;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_python_base::boundary::main::imported::imported;

/// Write the planner's `PlanOutput` when SQLBUILD_PLANNER_CAPTURE_DIR names a directory.
pub fn write_plan_capture(plan_output: &Bound<'_, PyAny>) -> PyResult<()> {
    let py: Python<'_> = plan_output.py();
    let directory: Bound<'_, PyAny> =
        imported(py, "os", "environ")?.call_method1("get", (PLAN_CAPTURE_DIR_ENV_VAR,))?;
    if !directory.is_truthy()? {
        return Ok(());
    }
    let number: u64 = capture_number(&directory)?;
    let path: Bound<'_, PyAny> = imported(py, "pathlib", "Path")?
        .call1((directory,))?
        .call_method1(
            "__truediv__",
            (format!("{number:03}-{PLAN_CAPTURE_NAME}{CAPTURE_SUFFIX}"),),
        )?;
    write_capture_file(&path, plan_output)
}
