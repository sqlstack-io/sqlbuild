//! `sqb dbt debug`: dbt's own diagnostics, then SQLBuild's.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_core::dbt_command::constants::NO_CONNECTION_OPTION;

use crate::bindings::_helpers::dbt_command::auto_init::args_tuple;
use crate::bindings::_helpers::dbt_command::support::color_enabled;
use crate::bindings::_helpers::dbt_command::support::flag;
use crate::bindings::_helpers::dbt_command::support::status;
use crate::bindings::_helpers::dbt_command::support::stream;
use crate::bindings::_helpers::dbt_interop::pipeline::debug_dbt_from_project;
use crate::bindings::_helpers::debug_command::run_debug::run_debug_command;
use sqlbuild_python_base::boundary::main::keywords::keywords;

/// Run `body` under a stderr status line that is always closed; returns its exit code.
fn with_status(
    py: Python<'_>,
    (message, use_color): (&str, bool),
    body: impl FnOnce() -> PyResult<i64>,
) -> PyResult<i64> {
    let reporter: Bound<'_, PyAny> = status(&stream(py, "stderr")?, use_color)?;
    reporter.call_method1("start", (message,))?;
    let outcome: PyResult<i64> = body();
    let closed: PyResult<Bound<'_, PyAny>> = reporter.call_method0("close");
    let code: i64 = outcome?;
    closed?;
    Ok(code)
}

/// Run dbt debug, then SQLBuild diagnostics; 0 only when both pass.
pub(crate) fn run_dbt_debug(
    project_dir: &Bound<'_, PyAny>,
    args: &[String],
    no_color: bool,
) -> PyResult<i32> {
    let py: Python<'_> = project_dir.py();
    let use_color: bool = !no_color && color_enabled(py)?;
    let dbt_code: i64 = with_status(py, ("Running dbt debug...", use_color), || {
        debug_dbt_from_project(
            py,
            Some(keywords(
                py,
                &[
                    ("project_dir", project_dir.clone()),
                    ("args", args_tuple(py, args)?),
                    ("stdout_stream", stream(py, "stdout")?),
                    ("stderr_stream", stream(py, "stderr")?),
                ],
            )?),
        )
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?
        .extract()
    })?;
    stream(py, "stdout")?.call_method1("write", ("\n",))?;
    let sqlbuild_code: i64 =
        with_status(py, ("Running SQLBuild diagnostics...", use_color), || {
            run_debug_command(
                py,
                Some(keywords(
                    py,
                    &[
                        ("project_dir", project_dir.clone()),
                        ("no_color", flag(py, no_color)),
                        (
                            "no_connection",
                            flag(py, args.iter().any(|arg| arg == NO_CONNECTION_OPTION)),
                        ),
                        ("json_output", flag(py, false)),
                    ],
                )?),
            )
            .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?
            .extract()
        })?;
    Ok(i32::from(dbt_code != 0 || sqlbuild_code != 0))
}
