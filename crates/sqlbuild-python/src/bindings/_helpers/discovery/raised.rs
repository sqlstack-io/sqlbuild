//! Native discovery failures as the Python exceptions discovery raises for them.

use pyo3::exceptions::{PyKeyError, PyRuntimeError};
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyBytes, PyDict, PyDictMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_discovery::models::{DiscoveryFailure, ReadFailure, StageFailure};
use sqlbuild_discovery::tree::models::ProjectTree;

use crate::bindings::_helpers::discovery::project_files::path_object;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::ErrorId;

const BUILTINS_MODULE: &str = "builtins";
const OS_MODULE: &str = "os";
const DECODE_ENCODING: &str = "utf-8";

/// The exception class of each native failure kind.
const FAILURE_CLASSES: [(&str, ErrorId); 14] = [
    ("model_sql", ErrorId::ModelSqlParse),
    ("declaration", ErrorId::DeclarationParse),
    ("sql_test", ErrorId::SqlTestParse),
    ("sql_scenario", ErrorId::SqlScenarioParse),
    ("schema", ErrorId::SchemaParse),
    ("source", ErrorId::SourceParse),
    ("project_path", ErrorId::ProjectPath),
    ("sql_hook", ErrorId::SqlHookParse),
    ("sql_audit", ErrorId::SqlAuditParse),
    ("resource_identity", ErrorId::ResourceIdentity),
    ("conflict", ErrorId::DiscoveryConflict),
    ("seed", ErrorId::SeedDiscovery),
    ("python_path", ErrorId::ProjectPythonPath),
    ("orchestration", ErrorId::OrchestrationDeclaration),
];

fn raised(result: PyResult<Bound<'_, PyAny>>) -> PyErr {
    match result {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

/// `error_class(message, help=help)` for the failure kind `kind`.
pub(crate) fn discovery_error(
    py: Python<'_>,
    kind: &str,
    message: String,
    help: Option<String>,
) -> PyErr {
    let Some((_, class)) = FAILURE_CLASSES.iter().find(|(name, _)| *name == kind) else {
        return PyKeyError::new_err(kind.to_owned());
    };
    raised((|| {
        let keywords: Bound<'_, PyDict> = PyDict::new(py);
        keywords.set_item("help", help)?;
        error_class(py, *class)?.call((message,), Some(&keywords))
    })())
}

/// Python's `native_failure`: the discovery error a native failure describes.
pub(crate) fn failure_error(py: Python<'_>, failure: DiscoveryFailure) -> PyErr {
    discovery_error(py, failure.kind.as_str(), failure.message, failure.help)
}

/// Python's `native_read_error`: what `read_text` raises for a file native reading could not read.
pub(crate) fn read_error(failure: ReadFailure, file_path: &Bound<'_, PyAny>) -> PyErr {
    let py: Python<'_> = file_path.py();
    raised((|| {
        let builtins = py.import(BUILTINS_MODULE)?;
        match failure {
            ReadFailure::Decode {
                bytes,
                start,
                end,
                reason,
            } => native_field(&builtins, "UnicodeDecodeError")?.call1((
                DECODE_ENCODING,
                PyBytes::new(py, &bytes),
                start,
                end,
                reason,
            )),
            ReadFailure::Io {
                errno,
                winerror,
                message,
            } => {
                let os_error = native_field(&builtins, "OSError")?;
                let path: String = file_path.str()?.to_string();
                match (winerror, errno) {
                    (Some(winerror), _) => os_error.call1((py.None(), message, path, winerror)),
                    (None, None) => os_error.call1((message,)),
                    (None, Some(errno)) => {
                        let reason = py.import(OS_MODULE)?.getattr("strerror")?.call1((errno,))?;
                        os_error.call1((errno, reason, path))
                    }
                }
            }
        }
    })())
}

/// Python's `native_collection` failure: an unlistable directory's read error, else the bug.
pub(crate) fn stage_error(
    tree: &ProjectTree,
    project_dir: &Bound<'_, PyAny>,
    failure: StageFailure,
) -> PyErr {
    match failure {
        StageFailure::Unlistable {
            relative_path,
            error,
        } => relative_read_error((tree, project_dir), relative_path, error),
        StageFailure::Internal(reason) => PyRuntimeError::new_err(reason),
    }
}

/// `read_error` for `relative_path` below `project_dir`, or the error building that path raised.
pub(crate) fn relative_read_error(
    (tree, project_dir): (&ProjectTree, &Bound<'_, PyAny>),
    relative_path: String,
    failure: ReadFailure,
) -> PyErr {
    match path_object(project_dir.py(), tree, relative_path)
        .and_then(|relative| project_dir.div(relative.bind(project_dir.py())))
    {
        Ok(path) => read_error(failure, &path),
        Err(error) => error,
    }
}

/// The error `read_error` builds for an operating-system read failure, for the platform oracle.
#[pyfunction]
#[pyo3(signature = (errno, winerror, message, *, file_path))]
fn _oracle_read_error<'py>(
    errno: Option<i32>,
    winerror: Option<i32>,
    message: String,
    file_path: &Bound<'py, PyAny>,
) -> Bound<'py, PyAny> {
    let failure = ReadFailure::Io {
        errno,
        winerror,
        message,
    };
    read_error(failure, file_path)
        .into_value(file_path.py())
        .into_bound(file_path.py())
        .into_any()
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(_oracle_read_error, module)?)
}
