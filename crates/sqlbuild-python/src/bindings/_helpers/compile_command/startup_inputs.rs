//! Stamps of the interpreter and site-path inputs Python startup depends on.

use std::path::PathBuf;

use pyo3::exceptions::PyOSError;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::PyList;
use pyo3::{IntoPyObject, pyfunction, wrap_pyfunction};
use sqlbuild_cache::startup_inputs::main::startup_stamps::startup_stamps;
use sqlbuild_cache::startup_inputs::models::{StartupLocations, StartupStamp};

use sqlbuild_python_base::boundary::main::imported::imported;

const OS_MODULE: &str = "os";

fn paths(values: &Bound<'_, PyAny>) -> PyResult<Vec<PathBuf>> {
    values.try_iter()?.map(|value| value?.extract()).collect()
}

/// `OSError(errno, os.strerror(errno), path)`, which Python narrows to its subclass.
fn stat_error(py: Python<'_>, path: PathBuf, error: &std::io::Error) -> PyErr {
    let Some(errno) = error.raw_os_error() else {
        return PyOSError::new_err(error.to_string());
    };
    let message: String = match imported(py, OS_MODULE, "strerror")
        .and_then(|strerror| strerror.call1((errno,)))
        .and_then(|text| text.extract())
    {
        Ok(message) => message,
        Err(failure) => return failure,
    };
    let Ok(path) = path.into_os_string().into_pyobject(py);
    PyOSError::new_err((errno, message, path.unbind()))
}

/// `[(path, st_mtime_ns, st_size), ...]` for every startup input path.
#[pyfunction]
#[pyo3(signature = (*, directories, files, prefixes))]
fn startup_input_stamps<'py>(
    directories: &Bound<'py, PyAny>,
    files: &Bound<'py, PyAny>,
    prefixes: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    let py: Python<'py> = directories.py();
    let locations = StartupLocations {
        directories: paths(directories)?,
        files: paths(files)?,
        prefixes: paths(prefixes)?,
    };
    let cwd: PathBuf = imported(py, OS_MODULE, "getcwd")?.call0()?.extract()?;
    let stamps: Vec<StartupStamp> =
        startup_stamps(&locations, &cwd).map_err(|(path, error)| stat_error(py, path, &error))?;
    let rows: Vec<Bound<'py, PyAny>> = stamps
        .into_iter()
        .map(|stamp| {
            Ok((stamp.path.into_os_string(), stamp.modified_ns, stamp.size)
                .into_pyobject(py)?
                .into_any())
        })
        .collect::<PyResult<_>>()?;
    PyList::new(py, rows)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(startup_input_stamps, module)?)?;
    Ok(())
}
