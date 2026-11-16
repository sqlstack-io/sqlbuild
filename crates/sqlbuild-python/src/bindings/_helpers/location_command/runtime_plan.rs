//! A location's plan previews run in its runtime interpreter from the project it last published.

use pyo3::PyErr;
use pyo3::exceptions::{PyBaseException, PyException, PyValueError};
use pyo3::prelude::{Bound, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;
use serde_json::{Value, json};

use crate::bindings::_helpers::cli::argv::parse_cli_invocation;
use crate::bindings::_helpers::cli::dispatch::with_routed_request;
use crate::bindings::_helpers::location_command::constants::PLAN_FAILED_CODE;
use crate::bindings::_helpers::location_command::types::HeldProject;
use crate::bindings::_helpers::plan_command::plan::{PlanProject, location_plan};
use crate::bindings::_helpers::recall::console::write_stderr;
use crate::bindings::_helpers::records::fields::field;

/// One plan request's reply: `sqb plan`'s exit code and JSON output, or why it failed.
pub(super) fn runtime_plan<'py>(
    py: Python<'py>,
    held: Option<&HeldProject<'py>>,
    request: &Value,
) -> PyResult<Value> {
    let argv: Vec<String> = serde_json::from_value(request["argv"].clone())
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
    let copy: &str = request["copy"].as_str().unwrap_or_default();
    let planned = (|| -> PyResult<(i32, Option<String>)> {
        let argv: Vec<Bound<'_, PyString>> =
            argv.iter().map(|part| PyString::new(py, part)).collect();
        let args = field(&parse_cli_invocation(py, argv, false)?, "args")?;
        if args.is_none() {
            return Err(PyException::new_err("the plan command line is invalid"));
        }
        let project = match held {
            Some((discovered, compiled)) => PlanProject::Held {
                discovered,
                compiled,
            },
            None => PlanProject::Compiled,
        };
        with_routed_request(&args, |request| location_plan(request, (project, copy)))
    })();
    Ok(match planned {
        Ok((exit_code, Some(output))) => json!({"exit_code": exit_code, "output": output}),
        Ok((_, None)) => {
            json!({"failed": {"code": PLAN_FAILED_CODE, "message": "the plan printed no JSON", "help": null}})
        }
        Err(error) if error.is_instance_of::<PyException>(py) => {
            let failure = failure(py, &error)?;
            write_stderr(
                py,
                &format!(
                    "Location plan failed: {}\n",
                    failure["message"].as_str().unwrap_or_default()
                ),
            )?;
            json!({"failed": failure})
        }
        Err(error) => return Err(error),
    })
}

/// A failed plan's `code`, `message` and `help`, as `sqb plan` prints a coded error.
fn failure(py: Python<'_>, error: &PyErr) -> PyResult<Value> {
    let value = error.value(py);
    Ok(json!({
        "code": error_text(value, "code")?.unwrap_or_else(|| PLAN_FAILED_CODE.to_owned()),
        "message": error_text(value, "message")?.unwrap_or(value.str()?.to_string()),
        "help": error_text(value, "help")?,
    }))
}

/// An error attribute as text; `None` when the error has no such attribute or it is `None`.
fn error_text(value: &Bound<'_, PyBaseException>, name: &str) -> PyResult<Option<String>> {
    match value.getattr_opt(name)? {
        Some(part) if !part.is_none() => Ok(Some(part.str()?.to_string())),
        _ => Ok(None),
    }
}
