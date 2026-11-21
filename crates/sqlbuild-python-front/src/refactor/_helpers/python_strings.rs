//! Project Python strings that name a model or column a refactoring renames.

use std::path::PathBuf;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_pysource::string_literals::main::string_literal_locations::string_literal_locations;
use sqlbuild_pysource::string_literals::models::StringQuery;

use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// The discovered Python declarations whose files a refactoring searches.
const PYTHON_DECLARATIONS: [&str; 7] = [
    "loader_functions",
    "asset_functions",
    "check_functions",
    "hook_functions",
    "audit_factories",
    "python_function_files",
    "materialization_files",
];

/// The project Python files discovery found, as sorted `(parts, relative path)`.
fn python_paths<'py>(
    discovered: &Bound<'py, PyAny>,
) -> PyResult<Vec<(Vec<String>, Bound<'py, PyAny>)>> {
    let mut paths: Vec<(Vec<String>, Bound<'py, PyAny>)> = Vec::new();
    for declarations in PYTHON_DECLARATIONS {
        for item in native_field(discovered, declarations)?.try_iter()? {
            let path: Bound<'py, PyAny> = native_field(&item?, "relative_path")?;
            let parts: Vec<String> = native_field(&path, "parts")?.extract()?;
            if !paths.iter().any(|(known, _)| *known == parts) {
                paths.push((parts, path));
            }
        }
    }
    paths.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(paths)
}

/// A file's text with universal newlines; unreadable and non-UTF-8 files are skipped.
fn read_text(path: PathBuf) -> Option<String> {
    let bytes: Vec<u8> = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(_unreadable) => return None,
    };
    match String::from_utf8(bytes) {
        Ok(text) => Some(text.replace("\r\n", "\n").replace('\r', "\n")),
        Err(_undecodable) => None,
    }
}

/// Flag Python strings that name a model or column; with context, only SQL naming it too.
#[pyfunction]
#[pyo3(signature = (**request))]
pub(crate) fn find_python_string_locations<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let request = Request::new("find_python_string_locations", request)?;
    let py: Python<'py> = request.py();
    let project_dir: PathBuf = request.get("project_dir")?.extract()?;
    let names: Vec<String> = request.get("names")?.extract()?;
    let reason: Bound<'py, PyAny> = request.get("reason")?;
    let context: Option<String> = match request.optional("context")? {
        Some(context) => context.extract()?,
        None => None,
    };
    let version = py.version_info();
    let mut locations: Vec<Bound<'py, PyAny>> = Vec::new();
    for (_, relative_path) in python_paths(&request.get("discovered")?)? {
        let relative: PathBuf = relative_path.extract()?;
        let Some(contents) = read_text(project_dir.join(relative)) else {
            continue;
        };
        let posix: Bound<'py, PyAny> = relative_path.call_method0("as_posix")?;
        let query = StringQuery {
            names: &names,
            context: context.as_deref(),
        };
        for (line, column) in
            string_literal_locations(&contents, &query, (version.major, version.minor))
        {
            let keywords: Bound<'py, PyDict> = PyDict::new(py);
            keywords.set_item("path", &posix)?;
            keywords.set_item("line", line)?;
            keywords.set_item("column", column)?;
            keywords.set_item("reason", &reason)?;
            locations.push(new_record(
                py,
                RecordId::ManualLocation,
                (),
                Some(&keywords),
            )?);
        }
    }
    PyTuple::new(py, locations)
}
