//! The directory snapshot one discovery pass shares, and the native tree it memoises.

use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyRefMut, PyResult, Python,
};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyString, PyStringMethods};
use pyo3::{pyfunction, pymethods, wrap_pyfunction};
use sqlbuild_discovery::models::ProjectRoot;
use sqlbuild_discovery::tree::models::ProjectTree;
use sqlbuild_python_base::boundary::main::context_variable::context_variable;
use std::path::PathBuf;

use crate::bindings::_helpers::discovery::raised::discovery_error;
use crate::bindings::models::{
    NativeDirectorySnapshot, NativeDirectorySnapshotScope, NativeProjectTree,
};

const SNAPSHOT_VARIABLE: &str = "sqlbuild_directory_snapshot";
const TREE_MEMO_KEY: &str = "native_project_tree";
const DISPLAY_PROBE: &str = "_";
const PROJECT_PATH_KIND: &str = "project_path";
const SURROGATE_ESCAPE_BASE: u32 = 0xDC00;
const ESCAPED_BYTE_FIRST: u16 = 0xDC80;
const ESCAPED_BYTE_LAST: u16 = 0xDCFF;
const UTF16_ENCODING: &str = "utf-16-le";
const SURROGATE_PASS: &str = "surrogatepass";

static ACTIVE_SNAPSHOT: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

/// The context variable holding the active snapshot, created once per interpreter.
fn active_variable(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    context_variable(py, &ACTIVE_SNAPSHOT, SNAPSHOT_VARIABLE)
}

/// The active snapshot when it belongs to `project_dir`.
fn active_snapshot<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, NativeDirectorySnapshot>>> {
    let py: Python<'py> = project_dir.py();
    let active: Bound<'py, PyAny> = active_variable(py)?.call_method0("get")?;
    let Ok(snapshot) = active.downcast_into::<NativeDirectorySnapshot>() else {
        return Ok(None);
    };
    if snapshot.get().project_dir.bind(py).eq(project_dir)? {
        return Ok(Some(snapshot));
    }
    Ok(None)
}

fn new_snapshot<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, NativeDirectorySnapshot>> {
    let py: Python<'py> = project_dir.py();
    Bound::new(
        py,
        NativeDirectorySnapshot {
            project_dir: project_dir.clone().unbind(),
            memo: PyDict::new(py).unbind(),
        },
    )
}

/// Activate a new snapshot for `project_dir` unless one is active; the token resets it.
fn enter_scope<'py>(project_dir: &Bound<'py, PyAny>) -> PyResult<Option<Bound<'py, PyAny>>> {
    if active_snapshot(project_dir)?.is_some() {
        return Ok(None);
    }
    let snapshot: Bound<'py, NativeDirectorySnapshot> = new_snapshot(project_dir)?;
    Ok(Some(
        active_variable(project_dir.py())?.call_method1("set", (snapshot,))?,
    ))
}

fn exit_scope(token: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
    match token {
        Some(token) => {
            active_variable(token.py())?.call_method1("reset", (token,))?;
            Ok(())
        }
        None => Ok(()),
    }
}

/// Run `body` with one snapshot of `project_dir` shared by every discovery call inside it.
pub(crate) fn in_snapshot<'py, T>(
    project_dir: &Bound<'py, PyAny>,
    body: impl FnOnce() -> PyResult<T>,
) -> PyResult<T> {
    let token: Option<Bound<'py, PyAny>> = enter_scope(project_dir)?;
    let result: PyResult<T> = body();
    exit_scope(token.as_ref())?;
    result
}

/// The memo of the active snapshot of `project_dir`, or of a single-use one outside a scope.
pub(crate) fn snapshot_memo<'py>(project_dir: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let snapshot: Bound<'py, NativeDirectorySnapshot> = match active_snapshot(project_dir)? {
        Some(snapshot) => snapshot,
        None => new_snapshot(project_dir)?,
    };
    Ok(snapshot.get().memo.bind(project_dir.py()).clone())
}

#[pymethods]
impl NativeDirectorySnapshot {
    /// The project directory the snapshot lists.
    #[getter]
    pub(crate) fn project_dir(&self, py: Python<'_>) -> Py<PyAny> {
        self.project_dir.clone_ref(py)
    }

    /// The values the pass's native calls share.
    #[getter]
    pub(crate) fn memo(&self, py: Python<'_>) -> Py<PyDict> {
        self.memo.clone_ref(py)
    }
}

#[pymethods]
impl NativeDirectorySnapshotScope {
    fn __enter__<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, NativeDirectorySnapshot>> {
        let project_dir: Bound<'py, PyAny> = slf.project_dir.bind(py).clone();
        slf.token = enter_scope(&project_dir)?.map(Bound::unbind);
        active_snapshot(&project_dir)?.map_or_else(|| new_snapshot(&project_dir), Ok)
    }

    #[pyo3(signature = (*_exc_info))]
    fn __exit__(
        &mut self,
        py: Python<'_>,
        _exc_info: Bound<'_, pyo3::types::PyTuple>,
    ) -> PyResult<bool> {
        let token: Option<Py<PyAny>> = self.token.take();
        exit_scope(token.as_ref().map(|token| token.bind(py)))?;
        Ok(false)
    }
}

/// Share one directory snapshot across every discovery call for `project_dir` inside the scope.
#[pyfunction]
#[pyo3(signature = (*, project_dir))]
pub(crate) fn directory_snapshot_scope(
    project_dir: Bound<'_, PyAny>,
) -> NativeDirectorySnapshotScope {
    NativeDirectorySnapshotScope {
        project_dir: project_dir.unbind(),
        token: None,
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeDirectorySnapshot>()?;
    module.add_class::<NativeDirectorySnapshotScope>()?;
    module.add_function(wrap_pyfunction!(directory_snapshot_scope, module)?)?;
    Ok(())
}

/// `str(project_dir / "_")` without its probe: the text printed before a relative path.
pub(crate) fn display_prefix(project_dir: &Bound<'_, PyAny>) -> PyResult<String> {
    let probed: String = project_dir.div(DISPLAY_PROBE)?.str()?.to_str()?.to_owned();
    Ok(probed[..probed.len() - DISPLAY_PROBE.len()].to_owned())
}

/// The project root native discovery reads, named as Python prints it.
pub(crate) fn project_root(project_dir: &Bound<'_, PyAny>) -> PyResult<ProjectRoot> {
    Ok(ProjectRoot {
        directory: PathBuf::from(project_dir.str()?.to_str()?),
        display_prefix: display_prefix(project_dir)?,
    })
}

/// The native listings shared by every native call of the pass, created once per snapshot.
pub(crate) fn project_tree<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, NativeProjectTree>> {
    let py: Python<'py> = project_dir.py();
    let memo: Bound<'py, PyDict> = snapshot_memo(project_dir)?;
    if let Some(tree) = memo.get_item(TREE_MEMO_KEY)?
        && let Ok(tree) = tree.downcast_into::<NativeProjectTree>()
    {
        return Ok(tree);
    }
    let directory: Bound<'py, PyString> = project_dir.str()?;
    let prefixed: Bound<'py, PyString> = project_dir.div(DISPLAY_PROBE)?.str()?;
    for text in [&directory, &prefixed] {
        if text.to_str().is_err() {
            let mut shown: String = escaped(text)?;
            if text.is(&prefixed) {
                shown.truncate(shown.len() - DISPLAY_PROBE.len());
            }
            return Err(discovery_error(
                py,
                PROJECT_PATH_KIND,
                format!(
                    "Project directory {shown} is not valid UTF-8; move the project to a \
                     directory whose path is valid UTF-8"
                ),
                None,
            ));
        }
    }
    let created: Bound<'py, NativeProjectTree> = Bound::new(
        py,
        NativeProjectTree {
            inner: ProjectTree::new(std::path::Path::new(directory.to_str()?)),
        },
    )?;
    memo.set_item(TREE_MEMO_KEY, &created)?;
    Ok(created)
}

/// Python's `_escaped`: undecodable name bytes as `\xNN`, other lone surrogates as `\uXXXX`.
pub(crate) fn escaped(text: &Bound<'_, PyString>) -> PyResult<String> {
    let encoded: Vec<u8> = text
        .call_method1("encode", (UTF16_ENCODING, SURROGATE_PASS))?
        .extract()?;
    let units = encoded
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair));
    Ok(char::decode_utf16(units)
        .map(|decoded| match decoded {
            Ok(character) => character.to_string(),
            Err(error) => escaped_unit(error.unpaired_surrogate()),
        })
        .collect())
}

fn escaped_unit(unit: u16) -> String {
    if cfg!(unix) && (ESCAPED_BYTE_FIRST..=ESCAPED_BYTE_LAST).contains(&unit) {
        return format!("\\x{:02x}", u32::from(unit) - SURROGATE_ESCAPE_BASE);
    }
    format!("\\u{unit:04x}")
}
