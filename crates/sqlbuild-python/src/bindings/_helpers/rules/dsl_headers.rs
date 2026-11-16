//! DSL header scanning, header lint rules and header formatting fixes over Python lint models.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods, PyString, PyTuple};
use pyo3::{IntoPyObject, pyfunction, wrap_pyfunction};
use sqlbuild_rules::dsl_headers::constants::RULE_HEADER_PARSE;
use sqlbuild_rules::dsl_headers::main::format_headers::format_headers;
use sqlbuild_rules::dsl_headers::main::lint_headers::lint_headers;
use sqlbuild_rules::dsl_headers::main::measurement_body_ranges::measurement_body_ranges as measurement_ranges;
use sqlbuild_rules::dsl_headers::main::scan_headers::scan_headers as scan;
use sqlbuild_rules::dsl_headers::main::sql_body_ranges::sql_body_ranges as body_ranges;
use sqlbuild_rules::dsl_headers::models::{HeaderRulesConfig, HeaderSpan, HeaderViolation};

use crate::bindings::_helpers::rules::lint_pipeline::{LintViolationRow, ViolationClasses};
use crate::bindings::_helpers::rules::native_header_parser::NativeHeaderParser;
use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

const LINE_FEED: &str = "\n";
const CARRIAGE_RETURN_LINE_FEED: &str = "\r\n";

/// Header violations against `file_path` as `LintViolation` records.
fn violation_objects<'py>(
    file_path: &Bound<'py, PyAny>,
    violations: Vec<HeaderViolation>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let classes: ViolationClasses<'py> = ViolationClasses::new(file_path.py())?;
    violations
        .into_iter()
        .map(|violation| classes.violation(file_path, &LintViolationRow::header(0, violation)))
        .collect()
}

fn text(contents: &Bound<'_, PyAny>) -> PyResult<Vec<char>> {
    Ok(contents.extract::<String>()?.chars().collect())
}

pub(crate) fn rules_config(config: &Bound<'_, PyAny>) -> PyResult<HeaderRulesConfig> {
    Ok(HeaderRulesConfig {
        line_width: native_field(config, "line_width")?.extract()?,
        max_description_lines: native_field(config, "max_description_lines")?.extract()?,
    })
}

fn spans(headers: &Bound<'_, PyAny>) -> PyResult<Vec<HeaderSpan>> {
    headers
        .try_iter()?
        .map(|header| {
            let header: Bound<'_, PyAny> = header?;
            Ok(HeaderSpan {
                kind: native_field(&header, "kind")?.extract()?,
                start: native_field(&header, "start")?.extract()?,
                end: native_field(&header, "end")?.extract()?,
            })
        })
        .collect()
}

fn ranges<'py>(py: Python<'py>, ranges: Vec<(usize, usize)>) -> PyResult<Bound<'py, PyTuple>> {
    PyTuple::new(py, ranges)
}

/// Every DSL header span in `contents`, or only the first.
#[pyfunction]
#[pyo3(signature = (*, contents, first_only = false))]
fn scan_headers<'py>(
    contents: &Bound<'py, PyAny>,
    first_only: bool,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = contents.py();
    let headers: Vec<Bound<'py, PyAny>> = scan(&text(contents)?, first_only)
        .into_iter()
        .map(|header| {
            new_record_with(
                py,
                RecordId::HeaderSpan,
                &[
                    ("kind", PyString::new(py, &header.kind).into_any()),
                    ("start", header.start.into_pyobject(py)?.into_any()),
                    ("end", header.end.into_pyobject(py)?.into_any()),
                ],
            )
        })
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, headers)
}

/// Non-empty SQL body ranges around and between the headers.
#[pyfunction]
#[pyo3(signature = (*, contents, headers))]
fn sql_body_ranges<'py>(
    contents: &Bound<'py, PyAny>,
    headers: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    ranges(
        contents.py(),
        body_ranges(&text(contents)?, &spans(headers)?),
    )
}

/// The authored query ranges inside MEASURE and EVIDENCE blocks.
#[pyfunction]
#[pyo3(signature = (*, contents))]
fn measurement_body_ranges<'py>(contents: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    ranges(contents.py(), measurement_ranges(&text(contents)?))
}

/// The header rule violations of `headers` in `contents`.
pub(crate) fn header_violations(
    contents: &[char],
    headers: &[HeaderSpan],
    config: HeaderRulesConfig,
) -> Result<Vec<HeaderViolation>, String> {
    lint_headers(contents, headers, config, &NativeHeaderParser)
}

/// Every header rule violation of `headers` in `contents`.
#[pyfunction]
#[pyo3(signature = (*, contents, file_path, headers, config))]
fn lint_native_headers<'py>(
    contents: &Bound<'py, PyAny>,
    file_path: &Bound<'py, PyAny>,
    headers: &Bound<'py, PyAny>,
    config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let violations: Vec<HeaderViolation> =
        header_violations(&text(contents)?, &spans(headers)?, rules_config(config)?)
            .map_err(value_error)?;
    PyTuple::new(contents.py(), violation_objects(file_path, violations)?)
}

/// `contents` with header formatting fixes applied, and the faults relocation reported.
#[pyfunction]
#[pyo3(signature = (*, contents, file_path, config))]
pub(crate) fn format_native_headers<'py>(
    contents: &Bound<'py, PyAny>,
    file_path: &Bound<'py, PyAny>,
    config: &Bound<'py, PyAny>,
) -> PyResult<(String, Bound<'py, PyTuple>)> {
    let py: Python<'py> = contents.py();
    let (formatted, faults) =
        format_headers(&text(contents)?, rules_config(config)?, &NativeHeaderParser)
            .map_err(value_error)?;
    Ok((
        formatted.into_iter().collect(),
        PyTuple::new(py, violation_objects(file_path, faults)?)?,
    ))
}

/// The header-parse faults of one file.
fn parse_faults<'py>(
    (file_path, contents): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    config: &Bound<'py, PyAny>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let contents: Vec<char> = text(contents)?;
    let violations: Vec<HeaderViolation> =
        header_violations(&contents, &scan(&contents, false), rules_config(config)?)
            .map_err(value_error)?
            .into_iter()
            .filter(|violation| violation.code == RULE_HEADER_PARSE)
            .collect();
    violation_objects(file_path, violations)
}

/// Parse-safe files, and every file's authored newline convention.
#[pyfunction]
#[pyo3(signature = (*, files, config))]
pub(crate) fn safe_format_files<'py>(
    files: &Bound<'py, PyDict>,
    config: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyDict>, Bound<'py, PyDict>)> {
    let py: Python<'py> = files.py();
    let safe: Bound<'py, PyDict> = PyDict::new(py);
    let newlines: Bound<'py, PyDict> = PyDict::new(py);
    for (file_path, contents) in files.iter() {
        if parse_faults((&file_path, &contents), config)?.is_empty() {
            safe.set_item(&file_path, &contents)?;
        }
    }
    for (file_path, contents) in files.iter() {
        let crlf: bool = contents
            .extract::<String>()?
            .contains(CARRIAGE_RETURN_LINE_FEED);
        newlines.set_item(
            file_path,
            if crlf {
                CARRIAGE_RETURN_LINE_FEED
            } else {
                LINE_FEED
            },
        )?;
    }
    Ok((safe, newlines))
}

/// Every rewrite whose header still parses, with the faults of the discarded ones appended.
#[pyfunction]
#[pyo3(signature = (*, updated, config, faults))]
pub(crate) fn reject_unparseable_header_rewrites<'py>(
    updated: &Bound<'py, PyDict>,
    config: &Bound<'py, PyAny>,
    faults: &Bound<'py, PyList>,
) -> PyResult<(Bound<'py, PyDict>, Bound<'py, PyList>)> {
    let accepted: Bound<'py, PyDict> = updated.copy()?;
    for (file_path, contents) in updated.iter() {
        let parse: Vec<Bound<'py, PyAny>> = parse_faults((&file_path, &contents), config)?;
        if parse.is_empty() {
            continue;
        }
        if accepted.contains(&file_path)? {
            accepted.del_item(&file_path)?;
        }
        for fault in parse {
            faults.append(fault)?;
        }
    }
    Ok((accepted, faults.clone()))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(scan_headers, module)?)?;
    module.add_function(wrap_pyfunction!(sql_body_ranges, module)?)?;
    module.add_function(wrap_pyfunction!(measurement_body_ranges, module)?)?;
    module.add_function(wrap_pyfunction!(lint_native_headers, module)?)?;
    module.add_function(wrap_pyfunction!(format_native_headers, module)?)?;
    module.add_function(wrap_pyfunction!(safe_format_files, module)?)?;
    module.add_function(wrap_pyfunction!(
        reject_unparseable_header_rewrites,
        module
    )?)?;
    Ok(())
}
