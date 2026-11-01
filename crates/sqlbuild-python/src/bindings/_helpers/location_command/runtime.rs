//! A location keeps its interpreter warm; only the supervisor writes generation requests.

use std::io::{self, Write};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_orchestration::sensors::models::{CustomSensorOutcome, SensorEvaluationRequest};

use crate::bindings::_helpers::boundary::python_calls::within;
use crate::bindings::_helpers::location_command::command::{location_arguments, publish_location};
use crate::bindings::_helpers::native_host::host::{redirect_stdout_to_stderr, write_console};
use crate::bindings::_helpers::native_host::orchestration_sensors::evaluate_sensor;

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
            let request: serde_json::Value = serde_json::from_str(&line).map_err(|_| {
                PyValueError::new_err(
                    "runtime expects a JSON generation or sensor evaluation per line",
                )
            })?;
            let output = if let Some(generation) = request.as_i64() {
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
                serde_json::to_string(&record)
            } else {
                let evaluation: SensorEvaluationRequest =
                    serde_json::from_value(request).map_err(|_| {
                        PyValueError::new_err("runtime received an invalid sensor evaluation")
                    })?;
                let arguments = location_arguments(py, Some(options.clone()))?;
                if arguments.value("location_id")?.extract::<String>()?
                    != evaluation.location_id.to_string()
                    || arguments.value("selected_target")?.extract::<String>()? != evaluation.target
                {
                    return Err(PyValueError::new_err(
                        "sensor work belongs to another location or target",
                    ));
                }
                let directory = arguments
                    .value("project_dir")?
                    .extract::<std::path::PathBuf>()?;
                let outcome = match evaluate_sensor(py, &directory, &evaluation) {
                    Ok(outcome) => outcome,
                    Err(_) => {
                        write_console(
                            py,
                            "stderr",
                            "Custom sensor evaluation failed. No cursor or run requests committed.\n",
                        )?;
                        CustomSensorOutcome::Failed
                    }
                };
                serde_json::to_string(&outcome)
            };
            let output = output.map_err(|error| PyValueError::new_err(error.to_string()))?;
            py.detach(|| -> io::Result<()> {
                let mut stdout = io::stdout().lock();
                writeln!(stdout, "{output}")?;
                stdout.flush()
            })?;
        }
        Ok(0)
    })
}
