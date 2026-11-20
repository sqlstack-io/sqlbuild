//! User callbacks and provider lifetimes belong only to the isolated location interpreter.

use std::path::Path;

use pyo3::IntoPyObject;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyString, PyTuple};
use serde_json::Value;
use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_orchestration::sensors::constants::MAX_SENSOR_RUN_REQUESTS;
use sqlbuild_orchestration::sensors::models::{RequestedRun, SensorEvent, SensorFunctionOutcome};

use crate::bindings::_helpers::native_host::setup_context::project_setup_context;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::json_value_to_python::json_value_to_python;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::native_host::constants::SENSOR_CONTEXT_PARAMETER;
use sqlbuild_python_base::native_host::main::host_entry::host_entry as entry;
use sqlbuild_python_base::orchestration_reads::main::callback_code_hash::callback_code_hash;
use sqlbuild_python_base::orchestration_reads::main::orchestration_code_hash::orchestration_code_hash;
use sqlbuild_python_front::discovery::main::python_files::discover_provider_classes::discover_provider_classes;
use sqlbuild_python_front::discovery::main::python_files::provider_by_name::provider_by_name;
use sqlbuild_python_front::discovery::main::python_files::provider_usages::provider_usages;
use sqlbuild_python_front::discovery::main::sources::orchestration_declarations::orchestration_declarations;
use sqlbuild_python_front::discovery::models::SecretPolicy;
use sqlbuild_python_project::project_snapshot::main::python_declarations::python_declarations;

/// The module the authoring records live in.
const AUTHORING_MODULE: &str = "sqlbuild.orchestration_declarations.models.authoring";
/// The module the event records live in.
const EVENTS_MODULE: &str = "sqlbuild.orchestration_declarations.models.sensor_events";
/// The module the event kinds live in.
const KINDS_MODULE: &str = "sqlbuild.orchestration_declarations.types";

/// One call of a sensor's function: which sensor, for which target and event, and the published code it must match.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SensorCall<'a> {
    pub(crate) sensor: &'a str,
    pub(crate) target: &'a str,
    pub(crate) evaluation_id: &'a str,
    pub(crate) event: Option<&'a SensorEvent>,
    pub(crate) cursor: Option<&'a str>,
    /// The published code's hash; `None` for `sqb sensor try`, which runs the local code as it is.
    pub(crate) code_hash: Option<&'a str>,
}

/// What the function returned; its exception or a return value it may not give is a failed call with the reason.
pub(crate) fn evaluate_sensor(
    py: Python<'_>,
    project_dir: &Path,
    call: SensorCall<'_>,
) -> SensorFunctionOutcome {
    match called(py, project_dir, call) {
        Ok(outcome) => outcome,
        Err(error) => SensorFunctionOutcome::Failed {
            error: traceback(py, &error),
        },
    }
}

fn called(
    py: Python<'_>,
    project_dir: &Path,
    call: SensorCall<'_>,
) -> PyResult<SensorFunctionOutcome> {
    let sensor = declaration(py, project_dir, call)?;
    let function = sensor.getattr("function")?;
    if let Some(published) = call.code_hash {
        let sources = py.detach(|| orchestration_code_hash(project_dir))?;
        if callback_code_hash(&function, &sources)? != published {
            return Err(PyValueError::new_err(
                "the sensor's code changed since the location published it; the next publication runs the new code",
            ));
        }
    }
    let fields = PyDict::new(py);
    fields.set_item("event", event_record(py, call.event)?)?;
    fields.set_item("cursor", call.cursor)?;
    fields.set_item("target", call.target)?;
    fields.set_item("evaluation_id", call.evaluation_id)?;
    let context = py
        .import("sqlbuild.orchestration")?
        .getattr("SensorContext")?
        .call((), Some(&fields))?;
    let project_path = imported(py, "pathlib", "Path")?.call1((project_dir,))?;
    let providers = discover_provider_classes(project_path.clone(), SecretPolicy::Defer.name())?;
    let by_name = provider_by_name(&providers)?;
    let target = (
        &PyString::new(py, call.target).into_any(),
        &py.None().into_bound(py),
    );
    let setup_context = project_setup_context((&project_path, &providers), target, true)?;
    let session = call_with(
        &entry(py, UserCodeHost::BuildProviderSession)?,
        &[
            ("discovered_providers", providers),
            ("setup_context", setup_context),
            ("allow_shared", PyBool::new(py, false).to_owned().into_any()),
        ],
    )?;
    within(&session, |_| {
        let arguments = PyDict::new(py);
        arguments.set_item(SENSOR_CONTEXT_PARAMETER, &context)?;
        for usage in provider_usages(&function, &by_name)?.try_iter()? {
            let usage = usage?;
            if usage.getattr("parameter_name")?.extract::<String>()? != SENSOR_CONTEXT_PARAMETER {
                arguments.set_item(
                    usage.getattr("parameter_name")?,
                    session.call_method1("get", (usage.getattr("provider_name")?,))?,
                )?;
            }
        }
        let result = function.call((), Some(&arguments))?;
        let cursor: Option<String> = context.getattr("cursor")?.extract()?;
        let cursor = cursor.filter(|_| call.event.is_none());
        outcome(py, &result, (call.event, cursor))
    })
}

/// The one decorated sensor named `call.sensor` the project declares for the call's target.
fn declaration<'py>(
    py: Python<'py>,
    project_dir: &Path,
    call: SensorCall<'_>,
) -> PyResult<Bound<'py, PyAny>> {
    let sensor_class = py.import(AUTHORING_MODULE)?.getattr("Sensor")?;
    let inventory = orchestration_declarations(project_dir.into_pyobject(py)?.as_any())?;
    let mut matches: Vec<Bound<'py, PyAny>> = Vec::new();
    for declaration in python_declarations(py, project_dir, &inventory)? {
        if declaration.is_instance(&sensor_class)?
            && declaration
                .getattr("name")?
                .extract::<Option<String>>()?
                .as_deref()
                == Some(call.sensor)
        {
            matches.push(declaration);
        }
    }
    let [sensor] = <[Bound<'py, PyAny>; 1]>::try_from(matches).map_err(|found| {
        PyValueError::new_err(format!(
            "the project declares {} sensors named {}; it needs exactly one",
            found.len(),
            call.sensor
        ))
    })?;
    let targets: Vec<String> = sensor.getattr("targets")?.extract()?;
    if !targets.is_empty() && !targets.iter().any(|target| target == call.target) {
        return Err(PyValueError::new_err(format!(
            "sensor {} isn't declared for target {}",
            call.sensor, call.target
        )));
    }
    if sensor.getattr("function")?.is_none() {
        return Err(PyValueError::new_err(format!(
            "sensor {} decorates no function",
            call.sensor
        )));
    }
    Ok(sensor)
}

/// The event as the record its condition's functions receive, such as a `RunEvent`.
fn event_record<'py>(py: Python<'py>, event: Option<&SensorEvent>) -> PyResult<Bound<'py, PyAny>> {
    let Some(event) = event else {
        return Ok(py.None().into_bound(py));
    };
    let mut fields: Value =
        serde_json::to_value(event).map_err(|error| PyValueError::new_err(error.to_string()))?;
    let (class, kinds): (&str, &[(&str, &str)]) = match event {
        SensorEvent::Run { .. } => ("RunEvent", &[("status", "RunEventKind")]),
        SensorEvent::Node { .. } => ("NodeEvent", &[("event", "NodeEventKind")]),
        SensorEvent::Check { .. } => ("CheckEvent", &[("status", "CheckEventKind")]),
        SensorEvent::SourceNewData { .. } => ("SourceNewDataEvent", &[]),
        SensorEvent::SourceStale { .. } => ("SourceStaleEvent", &[]),
        SensorEvent::Platform { .. } => ("PlatformEvent", &[("event", "PlatformEventKind")]),
    };
    if let Value::Object(map) = &mut fields {
        map.remove("kind");
    }
    let record = json_value_to_python(py, &fields)?.cast_into::<PyDict>()?;
    let kinds_module = py.import(KINDS_MODULE)?;
    for (field, kind) in kinds {
        if let Some(value) = record.get_item(field)? {
            record.set_item(field, kinds_module.getattr(*kind)?.call1((value,))?)?;
        }
    }
    let parse = imported(py, "datetime", "datetime")?.getattr("fromisoformat")?;
    if let Some(occurred_at) = record.get_item("occurred_at")? {
        record.set_item("occurred_at", parse.call1((occurred_at,))?)?;
    }
    for field in ["failed_nodes", "failed_checks"] {
        if let Some(items) = record.get_item(field)? {
            let items: Vec<Bound<'_, PyAny>> = items.try_iter()?.collect::<PyResult<_>>()?;
            record.set_item(field, PyTuple::new(py, items)?)?;
        }
    }
    py.import(EVENTS_MODULE)?
        .getattr(class)?
        .call((), Some(&record))
}

/// The tick's outcome from the function's return value: `None`, a `SkipReason`, a `RunRequest` or several.
fn outcome(
    py: Python<'_>,
    result: &Bound<'_, PyAny>,
    (event, cursor): (Option<&SensorEvent>, Option<String>),
) -> PyResult<SensorFunctionOutcome> {
    if result.is_none() {
        return Ok(SensorFunctionOutcome::Handled { cursor });
    }
    let authoring = py.import(AUTHORING_MODULE)?;
    if result.is_instance(&authoring.getattr("SkipReason")?)? {
        let reason: String = result.getattr("reason")?.extract()?;
        if reason.trim().is_empty() {
            return Err(PyValueError::new_err("SkipReason needs a nonempty reason"));
        }
        return Ok(SensorFunctionOutcome::Skipped { cursor, reason });
    }
    let request_class = authoring.getattr("RunRequest")?;
    let requests: Vec<Bound<'_, PyAny>> = if result.is_instance(&request_class)? {
        vec![result.clone()]
    } else if result.is_instance_of::<PyString>() {
        return Err(returned_error());
    } else {
        match result.try_iter() {
            Ok(items) => items
                .take(MAX_SENSOR_RUN_REQUESTS + 1)
                .collect::<PyResult<_>>()?,
            Err(_) => return Err(returned_error()),
        }
    };
    if requests.len() > MAX_SENSOR_RUN_REQUESTS {
        return Err(PyValueError::new_err(format!(
            "a sensor may start at most {MAX_SENSOR_RUN_REQUESTS} runs per call"
        )));
    }
    let mut runs: Vec<RequestedRun> = Vec::new();
    for request in requests {
        if !request.is_instance(&request_class)? {
            return Err(returned_error());
        }
        let job: String = request.getattr("job")?.extract()?;
        if job.trim().is_empty() {
            return Err(PyValueError::new_err("RunRequest needs a job"));
        }
        let key: Option<String> = request.getattr("run_key")?.extract()?;
        let run_key = match (key, event) {
            (Some(key), _) if !key.trim().is_empty() => key,
            (_, Some(event)) => format!("{}:{job}", event.id()),
            (_, None) => {
                return Err(PyValueError::new_err(
                    "a polling sensor's RunRequest needs a run_key, which makes repeated polls start the run once",
                ));
            }
        };
        runs.push(RequestedRun { job, run_key });
    }
    Ok(SensorFunctionOutcome::Requested { cursor, runs })
}

fn returned_error() -> PyErr {
    PyValueError::new_err(
        "a sensor returns None, a SkipReason, a RunRequest or a list of RunRequest records",
    )
}

/// The error with its Python traceback, as the failed tick shows it.
fn traceback(py: Python<'_>, error: &PyErr) -> String {
    let formatted = py.import("traceback").and_then(|module| {
        module
            .getattr("format_exception")?
            .call1((error.value(py),))?
            .extract::<Vec<String>>()
    });
    match formatted {
        Ok(lines) => lines.concat().trim_end().to_owned(),
        Err(_) => error.to_string(),
    }
}
