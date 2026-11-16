//! Native discovery results as the plain data the Python discovery facade materialises.

use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyTuple;
use pyo3::{IntoPyObjectExt, pyfunction, pymethods};
use sqlbuild_discovery::model_files::main::parse_model_text::parse_model_text;
use sqlbuild_discovery::model_files::models::{DiscoveredModelFile, ModelFileOptions};
use sqlbuild_discovery::tree::models::ProjectTree;
use std::path::Path;

use crate::discovery::_helpers::failures::raised::failure_error;
use crate::discovery::_helpers::headers::header_declarations::model_header_declarations;
use crate::discovery::_helpers::records::declaration_records::source_locations;
use crate::discovery::_helpers::records::model_file_record::{
    DiscoveredPaths, ModelFileParts, PendingModelFile,
};
use crate::discovery::_helpers::snapshots::native_classes::{ModelTextRequest, NativeProjectTree};
use crate::sqltext::main::map_to_python::map_to_python;
use crate::sqltext::main::optional_authored_value_to_python::optional_authored_value_to_python;
use sqlbuild_python_base::types::CompilerDetach;

pub(crate) type PyObject = Py<PyAny>;

const ENUMS_KEY: &str = "enums";
const CONSTANTS_KEY: &str = "constants";

/// A relative path as Python names it: the real, surrogate-escaped name where it is not UTF-8.
pub(crate) fn path_object(
    py: Python<'_>,
    tree: &ProjectTree,
    relative_path: String,
) -> PyResult<PyObject> {
    match tree.raw_relative(&relative_path) {
        Some(raw) => raw.into_os_string().into_py_any(py),
        None => relative_path.into_py_any(py),
    }
}

#[pymethods]
impl NativeProjectTree {
    #[new]
    fn new(project_dir: &str) -> Self {
        Self {
            inner: ProjectTree::new(Path::new(project_dir)),
        }
    }
}

/// One parsed model file's record, or its header declaration error, raised only if it is built.
pub(crate) fn model_record(
    py: Python<'_>,
    model: DiscoveredModelFile,
    paths: DiscoveredPaths,
    extract_implicit_alias_columns: bool,
) -> PyResult<Result<PyObject, PyErr>> {
    let pending: PendingModelFile = PendingModelFile::new(
        py,
        paths,
        ModelFileParts {
            contents: model.contents,
            header_values: model.header_values,
            header_column_locations: model.header_column_locations,
            output_column_locations: model.output_column_locations,
            query_sql: model.query_sql,
            extract_implicit_alias_columns,
        },
    )?;
    let authored = |key: &str| -> PyResult<Option<Bound<'_, PyAny>>> {
        Ok(
            optional_authored_value_to_python(py, pending.header_value(key))?
                .map(|value| value.into_bound(py)),
        )
    };
    let (enums, constants) = (authored(ENUMS_KEY)?, authored(CONSTANTS_KEY)?);
    let declarations: (Py<PyTuple>, Py<PyTuple>) = if enums.is_none() && constants.is_none() {
        (PyTuple::empty(py).unbind(), PyTuple::empty(py).unbind())
    } else {
        match model_header_declarations(
            py,
            (enums, constants),
            pending.name(py)?,
            pending.relative_path(py)?,
        ) {
            Ok((enums, constants)) => (enums.unbind(), constants.unbind()),
            Err(error) if error.is_instance_of::<PyException>(py) => return Ok(Err(error)),
            Err(error) => return Err(error),
        }
    };
    Ok(Ok(pending.finished(declarations).into_py_any(py)?))
}

/// Parse model contents as discovery does: `(header_values, query_sql, column locations)`.
#[pyfunction]
pub(crate) fn parse_model_contents(
    py: Python<'_>,
    request: ModelTextRequest,
    contents: String,
) -> PyResult<(PyObject, String, PyObject)> {
    let options = ModelFileOptions {
        supported_keys: request.supported_keys,
        removed_keys: request.removed_keys,
        extract_implicit_alias_columns: request.extract_implicit_alias_columns,
        extract_output_column_locations: request.extract_output_column_locations,
        path_filter: None,
    };
    let parsed = py
        .compiler_detach(|| Ok(parse_model_text(&request.file_path, contents, &options)))
        .map_err(sqlbuild_python_base::boundary::main::compiler_error::compiler_error)?;
    let model: DiscoveredModelFile = parsed.map_err(|failure| failure_error(py, failure))?;
    let path: Bound<'_, PyAny> = py
        .import("pathlib")?
        .getattr("Path")?
        .call1((request.file_path.as_str(),))?;
    Ok((
        map_to_python(py, &model.header_values)?,
        model.query_sql,
        source_locations(py, &path, &model.header_column_locations)?
            .into_any()
            .unbind(),
    ))
}
