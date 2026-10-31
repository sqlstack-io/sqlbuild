//! A location keeps its interpreter warm; only the supervisor writes generation requests.

use std::io::{self, Write};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};

use crate::bindings::_helpers::boundary::python_calls::within;
use crate::bindings::_helpers::location_command::command::{location_arguments, publish_location};
use crate::bindings::_helpers::native_host::host::{redirect_stdout_to_stderr, write_console};

pub(super) fn run_runtime(py: Python<'_>, options: Bound<'_, PyDict>) -> PyResult<i32> {
    options.set_item("action", "publish")?;
    let redirect = redirect_stdout_to_stderr(py)?;
    within(&redirect, |_| {
        loop {
            let line = py.detach(|| -> io::Result<Option<String>> {
                let mut line = String::new();
                Ok((io::stdin().read_line(&mut line)? != 0).then_some(line))
            })?;
            let Some(line) = line else {
                break;
            };
            let generation: i64 = serde_json::from_str(&line)
                .map_err(|_| PyValueError::new_err("runtime expects a JSON generation per line"))?;
            options.set_item("generation", generation)?;
            let arguments = location_arguments(py, Some(options.clone()))?;
            let record = match publish_location(py, &arguments) {
                Ok(record) => Some(record),
                Err(error) => {
                    write_console(
                        py,
                        "stderr",
                        &format!("Location compilation/publication failed: {error}\n"),
                    )?;
                    None
                }
            };
            let output = serde_json::to_string(&record)
                .map_err(|error| PyValueError::new_err(error.to_string()))?;
            py.detach(|| -> io::Result<()> {
                let mut stdout = io::stdout().lock();
                writeln!(stdout, "{output}")?;
                stdout.flush()
            })?;
        }
        Ok(0)
    })
}
