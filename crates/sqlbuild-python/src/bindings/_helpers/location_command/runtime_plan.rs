//! A location's plan previews, run in its runtime from its last publication over its synced state.

use std::sync::{Arc, Mutex, PoisonError};

use pyo3::PyErr;
use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::{Bound, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyList, PyString};
use serde_json::{Value, json};
use sqlbuild_orchestration::warehouse_metadata::main::location_reads::preview_state_parts::preview_state_parts;
use sqlbuild_orchestration::warehouse_metadata::models::PreviewCopy;
use sqlbuild_state::warehouse_state::main::warehouse_state::warehouse_state;
use sqlbuild_state::warehouse_state::models::WarehouseState;

use crate::bindings::_helpers::cli::argv::parse_cli_invocation;
use crate::bindings::_helpers::cli::dispatch::with_routed_request;
use crate::bindings::_helpers::location_command::constants::PLAN_FAILED_CODE;
use crate::bindings::_helpers::location_command::types::HeldProject;
use crate::bindings::_helpers::plan_command::plan::PlanProject;
use crate::bindings::_helpers::plan_command::plan::location_plan;
use crate::bindings::_helpers::review_command::support::attribute;
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_output::recall::main::write_stderr::write_stderr;
use sqlbuild_python_project::job_settings::main::pinned_plan::pinned_plan;
use sqlbuild_python_project::job_settings::main::within_pinned_build::within_pinned_build;

/// The synced state the runtime plans previews from, and the mirror version it was read at.
struct HeldState {
    version: String,
    state: Arc<WarehouseState>,
}

/// One state for the runtime's life; a newer mirror version replaces it.
static HELD_STATE: Mutex<Option<HeldState>> = Mutex::new(None);

/// The state a request plans from: the held state at its mirror version, else built from its copy file.
fn requested_state(
    held_state: &Mutex<Option<HeldState>>,
    request: &Value,
) -> PyResult<Arc<WarehouseState>> {
    let mut held = held_state.lock().unwrap_or_else(PoisonError::into_inner);
    let version: Option<&str> = request["version"].as_str();
    if let Some(current) = held
        .as_ref()
        .filter(|held| Some(held.version.as_str()) == version)
    {
        return Ok(Arc::clone(&current.state));
    }
    let path: &str = request["copy"].as_str().unwrap_or_default();
    let text: String = std::fs::read_to_string(path)?;
    let copy: PreviewCopy = serde_json::from_str(&text)
        .map_err(|error| PyValueError::new_err(format!("invalid synced copy: {error}")))?;
    let version: String = copy.version.clone();
    let state: Arc<WarehouseState> = Arc::new(warehouse_state(preview_state_parts(
        copy,
        request["consumption"].as_bool().unwrap_or(false),
    )));
    *held = Some(HeldState {
        version,
        state: Arc::clone(&state),
    });
    Ok(state)
}

/// One plan request's reply: `sqb plan`'s exit code and JSON output, or why it failed.
pub(super) fn runtime_plan<'py>(
    py: Python<'py>,
    held: Option<&HeldProject<'py>>,
    request: &Value,
) -> PyResult<Value> {
    let mut argv: Vec<String> = serde_json::from_value(request["argv"].clone())
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
    let select: Vec<String> = serde_json::from_value(request["select"].clone()).unwrap_or_default();
    let state: Arc<WarehouseState> = requested_state(&HELD_STATE, request)?;
    let pinned = pinned_plan(py, request)?;
    argv.extend(pinned.flags.clone());
    let planned = || -> PyResult<(i32, Option<String>)> {
        let argv: Vec<Bound<'_, PyString>> =
            argv.iter().map(|part| PyString::new(py, part)).collect();
        let args = field(&parse_cli_invocation(py, argv, false)?, "args")?;
        if args.is_none() {
            return Err(PyException::new_err("the plan command line is invalid"));
        }
        if !select.is_empty() {
            args.setattr("select", PyList::new(py, &select)?)?;
        }
        let project = match held {
            Some((discovered, compiled)) => PlanProject::Held {
                discovered,
                compiled,
            },
            None => PlanProject::Compiled,
        };
        with_routed_request(&args, |request| location_plan(request, (project, &state)))
    };
    let planned = within_pinned_build(&pinned, planned);
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
        "code": attribute(value.as_any(), "code")?.unwrap_or_else(|| PLAN_FAILED_CODE.to_owned()),
        "message": attribute(value.as_any(), "message")?.unwrap_or(value.str()?.to_string()),
        "help": attribute(value.as_any(), "help")?,
    }))
}
