//! User callbacks and provider lifetimes belong only to the isolated location interpreter.

use std::path::Path;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyString};
use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_orchestration::sensors::models::{SensorEvent, SensorFunctionOutcome};

use crate::bindings::_helpers::native_host::setup_context::project_setup_context;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::native_host::constants::SENSOR_CONTEXT_PARAMETER;
use sqlbuild_python_base::native_host::main::host_entry::host_entry as entry;
use sqlbuild_python_base::orchestration_reads::main::callback_code_hash::callback_code_hash;
use sqlbuild_python_base::orchestration_reads::main::instance_secret_values::instance_secret_values;
use sqlbuild_python_base::orchestration_reads::main::mask_sensor_outcome::mask_sensor_outcome;
use sqlbuild_python_base::orchestration_reads::main::orchestration_code_hash::orchestration_code_hash;
use sqlbuild_python_base::orchestration_reads::main::provider_secret_values::provider_secret_values;
use sqlbuild_python_base::orchestration_reads::main::sensor_event_record::sensor_event_record;
use sqlbuild_python_base::orchestration_reads::main::sensor_outcome::sensor_outcome;
use sqlbuild_python_base::orchestration_reads::main::sensor_traceback::sensor_traceback;
use sqlbuild_python_front::discovery::main::python_files::discover_provider_classes::discover_provider_classes;
use sqlbuild_python_front::discovery::main::python_files::provider_by_name::provider_by_name;
use sqlbuild_python_front::discovery::main::python_files::provider_usages::provider_usages;
use sqlbuild_python_front::discovery::models::SecretPolicy;
use sqlbuild_python_project::native_host::main::sensor_declaration::sensor_declaration;

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
        Err(error) => mask_sensor_outcome(
            SensorFunctionOutcome::Failed {
                error: sensor_traceback(py, &error),
            },
            &[],
        ),
    }
}

fn called(
    py: Python<'_>,
    project_dir: &Path,
    call: SensorCall<'_>,
) -> PyResult<SensorFunctionOutcome> {
    let sensor = sensor_declaration(py, project_dir, (call.sensor, call.target))?;
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
    fields.set_item("event", sensor_event_record(py, call.event)?)?;
    fields.set_item("cursor", call.cursor)?;
    fields.set_item("target", call.target)?;
    fields.set_item("evaluation_id", call.evaluation_id)?;
    let context = py
        .import("sqlbuild.orchestration")?
        .getattr("SensorContext")?
        .call((), Some(&fields))?;
    let project_path = imported(py, "pathlib", "Path")?.call1((project_dir,))?;
    let providers = discover_provider_classes(project_path.clone(), SecretPolicy::Defer.name())?;
    let secrets: Vec<String> = provider_secret_values(&providers)?;
    match with_providers(
        py,
        (&function, &context, call),
        (&project_path, providers, &secrets),
    ) {
        Ok(outcome) => Ok(outcome),
        Err(error) => Ok(mask_sensor_outcome(
            SensorFunctionOutcome::Failed {
                error: sensor_traceback(py, &error),
            },
            &secrets,
        )),
    }
}

/// The function called with the target's providers, its context the sensor's; its outcome masks `secrets` and the secret settings of the providers it was given.
fn with_providers<'py>(
    py: Python<'py>,
    (function, context, call): (&Bound<'py, PyAny>, &Bound<'py, PyAny>, SensorCall<'_>),
    (project_path, providers, secrets): (&Bound<'py, PyAny>, Bound<'py, PyAny>, &[String]),
) -> PyResult<SensorFunctionOutcome> {
    let by_name = provider_by_name(&providers)?;
    let target = (
        &PyString::new(py, call.target).into_any(),
        &py.None().into_bound(py),
    );
    let setup_context = project_setup_context((project_path, &providers), target, true)?;
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
        arguments.set_item(SENSOR_CONTEXT_PARAMETER, context)?;
        let mut instances: Vec<Bound<'py, PyAny>> = Vec::new();
        for usage in provider_usages(function, &by_name)?.try_iter()? {
            let usage = usage?;
            if usage.getattr("parameter_name")?.extract::<String>()? != SENSOR_CONTEXT_PARAMETER {
                let instance = session.call_method1("get", (usage.getattr("provider_name")?,))?;
                arguments.set_item(usage.getattr("parameter_name")?, &instance)?;
                instances.push(instance);
            }
        }
        let mut masked: Vec<String> = instance_secret_values(&instances)?;
        masked.extend_from_slice(secrets);
        let called = function.call((), Some(&arguments)).and_then(|result| {
            let cursor: Option<String> = context.getattr("cursor")?.extract()?;
            sensor_outcome(
                py,
                &result,
                (call.event, cursor.filter(|_| call.event.is_none())),
            )
        });
        let outcome = called.unwrap_or_else(|error| SensorFunctionOutcome::Failed {
            error: sensor_traceback(py, &error),
        });
        Ok(mask_sensor_outcome(outcome, &masked))
    })
}
