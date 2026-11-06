//! Interpreter, native build and invocation facts that decide whether a stored compile is reusable.

use std::collections::BTreeMap;
use std::ffi::OsString;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyTuple};
use pyo3::{IntoPyObjectExt, pyfunction, wrap_pyfunction};

use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::NativeAttr;
use crate::bindings::_helpers::records::registry;

/// The version of the runtime identity a stored compile records.
const REUSE_FORMAT_VERSION: &str = "1";
/// Each `CompileReuseRequest` field the invocation identity records unchanged.
const REQUEST_CHOICES: [&str; 9] = [
    "no_sql_validation",
    "defer_to",
    "selected_target",
    "json_output",
    "manifest",
    "dag_path",
    "no_color",
    "lineage_mode",
    "cli_vars",
];

/// `sys.version`, `sys.executable`, `sys.platform` and `str(sys.implementation.cache_tag)`.
pub(crate) fn interpreter_values(py: Python<'_>) -> PyResult<[String; 4]> {
    let sys: Bound<'_, PyModule> = py.import("sys")?;
    Ok([
        native_field(&sys, "version")?.extract()?,
        native_field(&sys, "executable")?.extract()?,
        native_field(&sys, "platform")?.extract()?,
        native_field(&sys, "implementation")?
            .getattr("cache_tag")?
            .str()?
            .extract()?,
    ])
}

/// The interpreter facts every interpreter-sensitive cache key stamps, in one order.
#[pyfunction]
fn interpreter_identity(py: Python<'_>) -> PyResult<Bound<'_, PyTuple>> {
    PyTuple::new(py, interpreter_values(py)?)
}

/// The interpreter and native build identity that produced a compile.
pub(crate) fn runtime_identity(py: Python<'_>) -> PyResult<BTreeMap<String, OsString>> {
    let [version, executable, platform, cache_tag] = interpreter_values(py)?;
    let native_build: String = registry::native(py, NativeAttr::BuildIdentity)?
        .str()?
        .extract()?;
    Ok(BTreeMap::from([
        ("format".to_owned(), REUSE_FORMAT_VERSION.into()),
        ("python".to_owned(), version.into()),
        ("executable".to_owned(), executable.into()),
        ("platform".to_owned(), platform.into()),
        ("cache_tag".to_owned(), cache_tag.into()),
        ("native_build".to_owned(), native_build.into()),
    ]))
}

fn listed<'py>(value: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyList::new(value.py(), value.try_iter()?.collect::<PyResult<Vec<_>>>()?)?.into_any())
}

/// Every command-line choice and location that can change compile output, as JSON bytes.
pub(crate) fn invocation_identity<'py>(
    request: &Bound<'py, PyAny>,
    project_dir: &Bound<'py, PyAny>,
    use_color: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let payload: Bound<'py, PyDict> = PyDict::new(py);
    payload.set_item("project_dir", project_dir)?;
    let requested: Bound<'py, PyAny> = native_field(request, "project_dir")?;
    let requested: Bound<'py, PyAny> = if requested.is_none() {
        requested
    } else {
        requested.str()?.into_any()
    };
    payload.set_item("requested_project_dir", requested)?;
    for field in REQUEST_CHOICES {
        payload.set_item(field, native_field(request, field)?)?;
    }
    payload.set_item("cwd", py.import("os")?.getattr("getcwd")?.call0()?)?;
    payload.set_item("use_color", use_color)?;
    payload.set_item("select", listed(native_field(request, "select")?)?)?;
    payload.set_item("exclude", listed(native_field(request, "exclude")?)?)?;
    payload.set_item("sys_path", listed(py.import("sys")?.getattr("path")?)?)?;
    let options: Bound<'py, PyDict> = keywords(
        py,
        &[
            ("sort_keys", true.into_bound_py_any(py)?),
            ("separators", (",", ":").into_bound_py_any(py)?),
            ("default", py.import("builtins")?.getattr("repr")?),
        ],
    )?;
    let text: Bound<'py, PyAny> = py
        .import("json")?
        .getattr("dumps")?
        .call((payload,), Some(&options))?;
    text.call_method1("encode", ("utf-8", "surrogateescape"))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(interpreter_identity, module)?)
}
