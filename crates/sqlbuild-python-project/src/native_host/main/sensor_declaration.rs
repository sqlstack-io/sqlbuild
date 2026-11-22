//! Find the decorated sensor a tick or trial calls, by importing the project's orchestration modules.

use std::path::Path;

use pyo3::IntoPyObject;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_python_base::orchestration_reads::constants::SENSOR_AUTHORING_MODULE;
use sqlbuild_python_front::discovery::main::sources::orchestration_declarations::orchestration_declarations;

use crate::project_snapshot::main::python_declarations::python_declarations;

/// The one decorated sensor named `name` the project declares for `target`.
pub fn sensor_declaration<'py>(
    py: Python<'py>,
    project_dir: &Path,
    (name, target): (&str, &str),
) -> PyResult<Bound<'py, PyAny>> {
    let sensor_class = py.import(SENSOR_AUTHORING_MODULE)?.getattr("Sensor")?;
    let inventory = orchestration_declarations(project_dir.into_pyobject(py)?.as_any())?;
    let mut matches: Vec<Bound<'py, PyAny>> = Vec::new();
    for declaration in python_declarations(py, project_dir, &inventory)? {
        if declaration.is_instance(&sensor_class)?
            && declaration
                .getattr("name")?
                .extract::<Option<String>>()?
                .as_deref()
                == Some(name)
        {
            matches.push(declaration);
        }
    }
    let [sensor] = <[Bound<'py, PyAny>; 1]>::try_from(matches).map_err(|found| {
        PyValueError::new_err(format!(
            "the project declares {} sensors named {}; it needs exactly one",
            found.len(),
            name
        ))
    })?;
    usable(&sensor, (name, target))?;
    Ok(sensor)
}

/// Whether `sensor` runs for `target` and decorates a function.
fn usable(sensor: &Bound<'_, PyAny>, (name, target): (&str, &str)) -> PyResult<()> {
    let targets: Vec<String> = sensor.getattr("targets")?.extract()?;
    if !targets.is_empty() && !targets.iter().any(|declared| declared == target) {
        return Err(PyValueError::new_err(format!(
            "sensor {name} isn't declared for target {target}"
        )));
    }
    if sensor.getattr("function")?.is_none() {
        return Err(PyValueError::new_err(format!(
            "sensor {name} decorates no function"
        )));
    }
    Ok(())
}
