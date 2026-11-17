//! Test observation of custom-rule host runs: a launch log and host fan-out overrides.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyRef, PyResult, Python};
use pyo3::types::PyTuple;
use pyo3::{pyclass, pymethods};

use sqlbuild_python_base::records::main::registry::native_attr::native_attr;
use sqlbuild_python_base::records::types::NativeAttr;

/// The `sqlbuild._native` attribute tests set to a `CustomHostObserver` to observe host runs.
pub(crate) const OBSERVER_ATTRIBUTE: &str = "custom_host_observer";

/// The requests of every host launched while observed, in launch order.
pub(crate) type LaunchLog = Arc<Mutex<Vec<String>>>;

/// Overrides host fan-out for the runs that see it and records every host they launch.
#[pyclass(frozen, module = "sqlbuild._native")]
pub(crate) struct CustomHostObserver {
    cores: Option<usize>,
    min_invocations_per_host: Option<usize>,
    publish_inputs: Option<Py<PyAny>>,
    launches: LaunchLog,
}

#[pymethods]
impl CustomHostObserver {
    #[new]
    #[pyo3(signature = (*, cores=None, min_invocations_per_host=None, publish_inputs=None))]
    fn new(
        cores: Option<usize>,
        min_invocations_per_host: Option<usize>,
        publish_inputs: Option<Py<PyAny>>,
    ) -> Self {
        Self {
            cores,
            min_invocations_per_host,
            publish_inputs,
            launches: LaunchLog::default(),
        }
    }

    /// The request of every host launched so far, in launch order.
    fn launches<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, locked(&self.launches).clone())
    }
}

/// What one host run takes from the observer installed when it started.
pub(crate) struct ObservedHosts {
    pub(crate) cores: Option<usize>,
    pub(crate) min_invocations_per_host: Option<usize>,
    pub(crate) publish_inputs: Option<Py<PyAny>>,
    pub(crate) launches: Option<LaunchLog>,
}

/// The installed observer's overrides and launch log; nothing when no test observes hosts.
pub(crate) fn observed_hosts(py: Python<'_>) -> PyResult<ObservedHosts> {
    let installed: Bound<'_, PyAny> = native_attr(py, NativeAttr::CustomHostObserver)?;
    if installed.is_none() {
        return Ok(ObservedHosts {
            cores: None,
            min_invocations_per_host: None,
            publish_inputs: None,
            launches: None,
        });
    }
    let observer: PyRef<'_, CustomHostObserver> = installed.extract()?;
    Ok(ObservedHosts {
        cores: observer.cores,
        min_invocations_per_host: observer.min_invocations_per_host,
        publish_inputs: observer
            .publish_inputs
            .as_ref()
            .map(|hook| hook.clone_ref(py)),
        launches: Some(Arc::clone(&observer.launches)),
    })
}

/// Record one host launch when the run is observed.
pub(crate) fn record_launch(launches: Option<&LaunchLog>, spec: &str) {
    if let Some(launches) = launches {
        locked(launches).push(spec.to_owned());
    }
}

/// Call the observer's publish hook with the payload path before the payload is written.
pub(crate) fn before_publish(
    py: Python<'_>,
    publish_inputs: Option<&Py<PyAny>>,
    path: &Path,
) -> PyResult<()> {
    if let Some(hook) = publish_inputs {
        hook.bind(py).call1((path.to_path_buf(),))?;
    }
    Ok(())
}

fn locked(launches: &LaunchLog) -> MutexGuard<'_, Vec<String>> {
    match launches.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
