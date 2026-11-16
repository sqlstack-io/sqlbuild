//! Environment, settings and run-identity reads templates make during one compile.

use crate::compile_context::models::CompileInputRecording;
use std::collections::BTreeSet;
use std::sync::{Mutex, MutexGuard, PoisonError};

use pyo3::prelude::{Bound, Py, PyAny, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::PyTuple;
use pyo3::{pyclass, pyfunction, pymethods, wrap_pyfunction};

const RUN_ID_KEY: &str = "run.id";

/// The recorders of every compile currently recording, delivered to from every thread.
#[pyclass(frozen, module = "sqlbuild._native")]
pub(crate) struct CompileInputReadRegistry {
    recorders: Mutex<Vec<Py<CompileInputReads>>>,
}

impl CompileInputReadRegistry {
    pub(crate) fn locked(&self) -> MutexGuard<'_, Vec<Py<CompileInputReads>>> {
        self.recorders
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn recorders(&self, py: Python<'_>) -> Vec<Py<CompileInputReads>> {
        self.locked()
            .iter()
            .map(|recorder| recorder.clone_ref(py))
            .collect()
    }
}

static REGISTRY: PyOnceLock<Py<CompileInputReadRegistry>> = PyOnceLock::new();

pub(crate) fn registry(py: Python<'_>) -> PyResult<&Py<CompileInputReadRegistry>> {
    REGISTRY.get_or_try_init(py, || {
        Py::new(
            py,
            CompileInputReadRegistry {
                recorders: Mutex::new(Vec::new()),
            },
        )
    })
}

fn recorders(py: Python<'_>) -> PyResult<Vec<Py<CompileInputReads>>> {
    Ok(registry(py)?.get().recorders(py))
}

/// Note that a template looked up one environment variable, present or not.
pub(crate) fn environment_read(py: Python<'_>, name: &str) -> PyResult<()> {
    for recorder in recorders(py)? {
        recorder.get().add_environment_name(name.to_owned());
    }
    Ok(())
}

/// Note a CTX lookup; only the per-invocation run identity is volatile.
pub(crate) fn context_read(py: Python<'_>, name: &str) -> PyResult<()> {
    if name != RUN_ID_KEY {
        return Ok(());
    }
    for recorder in recorders(py)? {
        recorder.get().mark_run_id_read();
    }
    Ok(())
}

/// Note a provider settings class instantiated from the environment and files.
#[pyfunction]
pub(crate) fn record_settings_class_read(settings_class: &Bound<'_, PyAny>) -> PyResult<()> {
    for recorder in recorders(settings_class.py())? {
        recorder.get().add_settings_class(settings_class);
    }
    Ok(())
}

/// Record template input reads from every thread until the returned block exits.
#[pyfunction]
pub(crate) fn record_compile_input_reads() -> CompileInputRecording {
    CompileInputRecording {
        recorder: Mutex::new(None),
    }
}

#[derive(Default)]
pub(crate) struct Reads {
    environment_names: BTreeSet<String>,
    read_run_id: bool,
    settings_classes: Vec<Py<PyAny>>,
}

/// Environment, settings, and run-identity reads observed during one compile.
#[pyclass(frozen, module = "sqlbuild._native")]
pub(crate) struct CompileInputReads {
    pub(crate) reads: Mutex<Reads>,
}

impl CompileInputReads {
    pub(crate) fn locked(&self) -> MutexGuard<'_, Reads> {
        self.reads.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[pymethods]
impl CompileInputReads {
    #[new]
    pub(crate) fn new() -> Self {
        Self {
            reads: Mutex::new(Reads::default()),
        }
    }

    /// Record one environment variable a template looked up, present or not.
    pub(crate) fn add_environment_name(&self, name: String) {
        self.locked().environment_names.insert(name);
    }

    /// Record one settings class whose values come from the environment or files.
    pub(crate) fn add_settings_class(&self, settings_class: &Bound<'_, PyAny>) {
        let mut reads: MutexGuard<'_, Reads> = self.locked();
        if !reads
            .settings_classes
            .iter()
            .any(|known| known.is(settings_class))
        {
            reads.settings_classes.push(settings_class.clone().unbind());
        }
    }

    /// Record that a template read the per-invocation run identity.
    pub(crate) fn mark_run_id_read(&self) {
        self.locked().read_run_id = true;
    }

    /// The recorded environment variable names in sorted order.
    #[getter]
    pub(crate) fn environment_names<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, self.locked().environment_names.iter())
    }

    /// The recorded settings classes in discovery order.
    #[getter]
    pub(crate) fn settings_classes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let classes: Vec<Py<PyAny>> = self
            .locked()
            .settings_classes
            .iter()
            .map(|class| class.clone_ref(py))
            .collect();
        PyTuple::new(py, classes)
    }

    /// Whether any template read the per-invocation run identity.
    #[getter]
    pub(crate) fn read_run_id(&self) -> bool {
        self.locked().read_run_id
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<CompileInputReads>()?;
    module.add_class::<CompileInputRecording>()?;
    module.add_function(wrap_pyfunction!(record_settings_class_read, module)?)?;
    module.add_function(wrap_pyfunction!(record_compile_input_reads, module)?)
}
