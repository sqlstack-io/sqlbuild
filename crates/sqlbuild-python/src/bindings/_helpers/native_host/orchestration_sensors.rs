//! User callbacks and provider lifetimes belong only to the isolated location interpreter.

use std::path::Path;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods};
use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_orchestration::sensors::models::{
    CustomSensorOutcome, SensorCondition, SensorEvaluationRequest,
};

use crate::bindings::_helpers::boundary::python_calls::{call_with, imported, within};
use crate::bindings::_helpers::compiled_project::snapshot_declarations::discover_declarations;
use crate::bindings::_helpers::discovery::imported_modules::{provider_by_name, provider_usages};
use crate::bindings::_helpers::discovery::imported_providers::discover_provider_classes;
use crate::bindings::_helpers::native_host::constants::SENSOR_CONTEXT_PARAMETER;
use crate::bindings::_helpers::native_host::host::entry;
use crate::bindings::_helpers::native_host::orchestration_environment::{
    callback_code_hash, orchestration_code_hash,
};

pub(crate) fn evaluate_sensor(
    py: Python<'_>,
    project_dir: &Path,
    evaluation: &SensorEvaluationRequest,
) -> PyResult<CustomSensorOutcome> {
    let SensorCondition::Custom {
        function: name,
        code_hash,
    } = &evaluation.definition.when
    else {
        return Err(PyValueError::new_err("custom sensor work required"));
    };
    let authoring = py.import("sqlbuild.orchestration_declarations.models.authoring")?;
    let sensor_class = authoring.getattr("Sensor")?;
    let mut matches: Vec<Bound<'_, PyAny>> = Vec::new();
    for (declaration, _) in discover_declarations(py, project_dir)? {
        if declaration.is_instance(&sensor_class)?
            && declaration.getattr("name")?.extract::<String>()? == *name
        {
            matches.push(declaration);
        }
    }
    let [sensor] = matches.as_slice() else {
        return Err(PyValueError::new_err(
            "custom sensor declaration must resolve uniquely",
        ));
    };
    let targets: Vec<String> = sensor.getattr("targets")?.extract()?;
    if !sensor.getattr("when")?.is_none()
        || (!targets.is_empty() && !targets.contains(&evaluation.target))
        || sensor
            .getattr("run")?
            .getattr("name")?
            .extract::<String>()?
            != evaluation.definition.job_key
    {
        return Err(PyValueError::new_err("custom sensor definition changed"));
    }
    let function = sensor.getattr("function")?;
    let sources = py.detach(|| orchestration_code_hash(project_dir))?;
    if callback_code_hash(&function, &sources)? != *code_hash {
        return Err(PyValueError::new_err("custom sensor code changed"));
    }
    let fields = PyDict::new(py);
    fields.set_item("cursor", &evaluation.cursor.custom)?;
    fields.set_item("target", &evaluation.target)?;
    fields.set_item("evaluation_id", evaluation.evaluation_id.to_string())?;
    let context = py
        .import("sqlbuild.orchestration")?
        .getattr("SensorContext")?
        .call((), Some(&fields))?;
    let providers =
        discover_provider_classes(imported(py, "pathlib", "Path")?.call1((project_dir,))?)?;
    let by_name = provider_by_name(&providers)?;
    let session = call_with(
        &entry(py, UserCodeHost::BuildProviderSession)?,
        &[
            ("discovered_providers", providers),
            ("allow_shared", PyBool::new(py, false).to_owned().into_any()),
        ],
    )?;
    within(&session, |_| {
        let arguments = PyDict::new(py);
        arguments.set_item("ctx", &context)?;
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
        if result.is_instance(&authoring.getattr("SkipReason")?)? {
            let reason: String = result.getattr("reason")?.extract()?;
            if reason.trim().is_empty() {
                return Err(PyValueError::new_err("SkipReason needs a nonempty reason"));
            }
            return Ok(CustomSensorOutcome::Skipped {
                cursor: context.getattr("cursor")?.extract()?,
                reason,
            });
        }
        let request_class = authoring.getattr("RunRequest")?;
        let mut run_keys: Vec<String> = Vec::new();
        for request in result
            .try_iter()?
            .take(sqlbuild_orchestration::sensors::constants::MAX_SENSOR_RUN_REQUESTS + 1)
        {
            let request = request?;
            if !request.is_instance(&request_class)? {
                return Err(PyValueError::new_err(
                    "custom sensors return RunRequest records or SkipReason",
                ));
            }
            let key: String = request.getattr("run_key")?.extract()?;
            if key.trim().is_empty() {
                return Err(PyValueError::new_err("RunRequest needs a nonempty run_key"));
            }
            run_keys.push(key);
        }
        if run_keys.len() > sqlbuild_orchestration::sensors::constants::MAX_SENSOR_RUN_REQUESTS {
            return Err(PyValueError::new_err(
                "custom sensor exceeds the bounded run-request page",
            ));
        }
        Ok(CustomSensorOutcome::Completed {
            cursor: context.getattr("cursor")?.extract()?,
            run_keys,
        })
    })
}
