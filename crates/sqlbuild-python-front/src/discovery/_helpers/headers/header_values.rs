//! MODEL-style header values with a bounded parse cache, and authored column locations.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods, PyTuple};

use crate::sqltext::main::header_failure::header_failure;
use crate::sqltext::main::model_output_columns::model_output_columns;
use crate::sqltext::main::parse_model_headers::parse_model_headers;
use crate::sqltext::models::ColumnLocation;
use crate::sqltext::types::ParsedModelHeader;
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

const HEADER_CACHE_SIZE: usize = 4096;

/// Parsed headers by text: `(values or None, error or None)`.
static HEADER_CACHE: PyOnceLock<Py<PyDict>> = PyOnceLock::new();

fn header_cache(py: Python<'_>) -> Bound<'_, PyDict> {
    HEADER_CACHE
        .get_or_init(py, || PyDict::new(py).unbind())
        .bind(py)
        .clone()
}

/// Parse every header the cache misses in one native batch.
#[pyfunction]
pub(crate) fn prepare_model_header_tokens(headers: &Bound<'_, PyAny>) -> PyResult<()> {
    let py: Python<'_> = headers.py();
    let cache: Bound<'_, PyDict> = header_cache(py);
    let missing: Bound<'_, PyDict> = PyDict::new(py);
    for header in headers.try_iter()? {
        let header: Bound<'_, PyAny> = header?;
        if !cache.contains(&header)? {
            missing.set_item(header, py.None())?;
        }
    }
    if missing.is_empty() {
        return Ok(());
    }
    if cache.len() + missing.len() > HEADER_CACHE_SIZE {
        cache.clear();
    }
    let names: Bound<'_, PyList> = missing.keys();
    let parsed: Vec<ParsedModelHeader> = parse_model_headers(py, names.extract()?)?;
    for (header, (values, _offsets, error)) in names.iter().zip(parsed) {
        cache.set_item(header, (values, error))?;
    }
    Ok(())
}

/// `parse_header_values(*, header, file_path, statement_name, header_line, error_class=None)`.
#[pyfunction]
#[pyo3(signature = (**request))]
pub(crate) fn parse_header_values<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyDict>> {
    let request = Request::new("parse_header_values", request)?;
    let py: Python<'py> = request.py();
    let header: Bound<'py, PyAny> = request.get("header")?;
    let cache: Bound<'py, PyDict> = header_cache(py);
    if !cache.contains(&header)? {
        prepare_model_header_tokens(&PyTuple::new(py, [&header])?.into_any())?;
    }
    let (values, error): (Bound<'py, PyAny>, Bound<'py, PyAny>) =
        header_cache(py).as_any().get_item(&header)?.extract()?;
    if !values.is_none() {
        let copied: Bound<'py, PyDict> = PyDict::new(py);
        copied.update(values.cast()?.as_mapping())?;
        return Ok(copied);
    }
    let header_line: Option<usize> = request.get("header_line")?.extract()?;
    let (detail, help, line): (String, Option<String>, Option<usize>) = header_failure(
        &error.str()?.extract::<String>()?,
        &header.extract::<String>()?,
        header_line,
    );
    let file_path: String = request.get("file_path")?.str()?.extract()?;
    let location: String = match line {
        Some(line) => format!("{file_path}:{line}"),
        None => file_path,
    };
    let statement: String = request.get("statement_name")?.extract()?;
    let error_class: Bound<'py, PyAny> = match request.optional("error_class")? {
        Some(class) if !class.is_none() => class,
        _ => error_class(py, ErrorId::ModelSqlParse)?,
    };
    let keywords: Bound<'py, PyDict> = PyDict::new(py);
    keywords.set_item("help", help)?;
    let raised: Bound<'py, PyAny> = error_class.call(
        (format!(
            "{statement}(...) in '{location}' contains invalid SQLBuild header syntax: {detail}"
        ),),
        Some(&keywords),
    )?;
    Err(pyo3::PyErr::from_value(raised))
}

/// `{name: SourceLocation}` from native `(name, line, column, end line, end column)` rows.
fn source_locations<'py>(
    rows: Vec<ColumnLocation>,
    relative_path: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = relative_path.py();
    let locations: Bound<'py, PyDict> = PyDict::new(py);
    for ColumnLocation {
        name,
        line,
        column,
        end_line,
        end_column,
    } in rows
    {
        let arguments: Bound<'py, PyDict> = PyDict::new(py);
        arguments.set_item("path", relative_path)?;
        arguments.set_item("line", line)?;
        arguments.set_item("column", column)?;
        arguments.set_item("end_line", end_line)?;
        arguments.set_item("end_column", end_column)?;
        locations.set_item(
            name,
            new_record(py, RecordId::SourceLocation, (), Some(&arguments))?,
        )?;
    }
    Ok(locations)
}

/// `model_output_column_locations(*, contents, relative_path, extract_implicit_alias_columns=True)`.
#[pyfunction]
#[pyo3(signature = (**request))]
pub(crate) fn model_output_column_locations<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyDict>> {
    let request = Request::new("model_output_column_locations", request)?;
    let implicit: bool = match request.optional("extract_implicit_alias_columns")? {
        Some(value) => value.is_truthy()?,
        None => true,
    };
    let contents: String = request.get("contents")?.extract()?;
    let rows: Vec<ColumnLocation> = model_output_columns(&contents, implicit)?;
    source_locations(rows, &request.get("relative_path")?)
}
