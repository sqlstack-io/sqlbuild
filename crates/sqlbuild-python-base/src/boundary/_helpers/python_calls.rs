//! Calling the Python stages the native driver still runs: imports, keyword calls and `with`.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods};

/// Standard-library modules whose attributes tests and users never replace, so lookups are kept.
const STABLE_MODULES: &[&str] = &[
    "builtins",
    "collections.abc",
    "dataclasses",
    "decimal",
    "enum",
    "functools",
    "hashlib",
    "itertools",
    "json",
    "math",
    "operator",
    "pathlib",
    "re",
    "uuid",
];

type ImportedCache = Mutex<HashMap<(String, String), Py<PyAny>>>;

/// `imported` attributes of `STABLE_MODULES`, looked up once.
static IMPORTED: PyOnceLock<ImportedCache> = PyOnceLock::new();

/// `module.name`, imported.
pub(crate) fn imported<'py>(
    py: Python<'py>,
    module: &str,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    if !STABLE_MODULES.contains(&module) {
        return py.import(module)?.getattr(name);
    }
    let cache: &ImportedCache = IMPORTED.get_or_init(py, || Mutex::new(HashMap::new()));
    let key: (String, String) = (module.to_owned(), name.to_owned());
    let cached: Option<Py<PyAny>> = cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
        .map(|found| found.clone_ref(py));
    if let Some(found) = cached {
        return Ok(found.into_bound(py));
    }
    let found: Bound<'py, PyAny> = py.import(module)?.getattr(name)?;
    let _ = cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(key, found.clone().unbind());
    Ok(found)
}

/// Keyword arguments built from `(name, value)` pairs, in order.
pub(crate) fn keywords<'py>(
    py: Python<'py>,
    pairs: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyDict>> {
    let arguments: Bound<'py, PyDict> = PyDict::new(py);
    for (name, value) in pairs {
        arguments.set_item(*name, value)?;
    }
    Ok(arguments)
}

/// `callable(**pairs)`.
pub(crate) fn call_with<'py>(
    callable: &Bound<'py, PyAny>,
    pairs: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    callable.call((), Some(&keywords(callable.py(), pairs)?))
}

/// Run `body` inside `with manager as entered:`; managers here never suppress exceptions.
pub(crate) fn within<'py, T>(
    manager: &Bound<'py, PyAny>,
    body: impl FnOnce(&Bound<'py, PyAny>) -> PyResult<T>,
) -> PyResult<T> {
    let py: Python<'py> = manager.py();
    let entered: Bound<'py, PyAny> = manager.call_method0("__enter__")?;
    match body(&entered) {
        Ok(value) => {
            manager.call_method1("__exit__", (py.None(), py.None(), py.None()))?;
            Ok(value)
        }
        Err(error) => {
            manager.call_method1(
                "__exit__",
                (error.get_type(py), error.value(py), error.traceback(py)),
            )?;
            Err(error)
        }
    }
}

/// The `contextvars.ContextVar(name, default=None)` kept in `cell`, created once per interpreter.
pub(crate) fn context_variable<'py>(
    py: Python<'py>,
    cell: &'static PyOnceLock<Py<PyAny>>,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    cell.get_or_try_init(py, || {
        let keywords: Bound<'_, PyDict> = PyDict::new(py);
        keywords.set_item("default", py.None())?;
        imported(py, "contextvars", "ContextVar")?
            .call((name,), Some(&keywords))
            .map(Bound::unbind)
    })
    .map(|variable| variable.bind(py).clone())
}
