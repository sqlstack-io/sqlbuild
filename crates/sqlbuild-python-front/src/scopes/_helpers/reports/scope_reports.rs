//! `sqb scope` reports over a `ScopeLookup`, their serialization and the metadata projection.

use crate::scopes::main::reports::json_to_python::json_to_python;
use crate::scopes::main::reports::read_result::read_result;
use crate::scopes::main::reports::scope_browse::scope_browse;
use crate::scopes::main::reports::scope_explain::scope_explain;
use crate::scopes::main::reports::scope_list::scope_list;
use crate::scopes::main::reports::scope_report::scope_report;
use crate::scopes::models::ExplainArguments;
use crate::scopes::models::ListArguments;
use crate::scopes::models::ReportArguments;
use pyo3::exceptions::{PyIndexError, PyValueError};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_core::json::models::JsonValue;
use sqlbuild_scopes::scope_index::main::identity_failure_message::identity_failure_message;
use sqlbuild_scopes::scope_index::main::rejected_path_message::rejected_path_message;
use sqlbuild_scopes::scope_report::main::cached_completeness::cached_completeness;
use sqlbuild_scopes::scope_report::main::cached_declaration_values::cached_declaration_values;
use sqlbuild_scopes::scope_report::main::cached_projection_index::cached_projection_index;
use sqlbuild_scopes::scope_report::main::scope_metadata_projection::scope_metadata_projection;
use sqlbuild_scopes::scope_report::main::serialize_scope_result::serialize_scope_result;
use sqlbuild_scopes::scope_report::models::{
    DeclarationValues, DiagnosticFacts, FolderRequest, ProjectionIndex, ReportDiagnostic,
    ReportFailure, ReportSource, ReportValues, ScopeResult,
};

use crate::scopes::_helpers::index::scope_lookup::NativeScopeLookup;
use crate::scopes::_helpers::index::scope_objects::{
    read_completeness, read_declaration, read_index,
};
use crate::scopes::_helpers::reports::scope_report_objects::{read_filters, read_optional_target};
use crate::scopes::models::ScopeClasses;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

pub(crate) type PyObject = Py<PyAny>;

const CURRENT_FOLDER: &str = ".";

/// One lookup's native lookup and every declaration's values in input order.
pub(crate) struct LookupReports {
    pub(crate) lookup: Py<NativeScopeLookup>,
    values: ReportValues,
}

impl LookupReports {
    pub(crate) fn source<'a>(&'a self, lookup: &'a NativeScopeLookup) -> ReportSource<'a> {
        ReportSource {
            records: &lookup.records,
            lookup: &lookup.lookup,
            values: &self.values,
        }
    }
}

/// The reports of a `ScopeLookup`: its native lookup with its canonical declarations' values.
pub(crate) fn lookup_reports(lookup: &Bound<'_, PyAny>) -> PyResult<LookupReports> {
    let index: Bound<'_, PyAny> = native_field(lookup, "index")?;
    let native: Py<NativeScopeLookup> = native_field(lookup, "native")?.extract()?;
    let rows: Vec<DeclarationValues> = native_field(&index, "declarations")?
        .try_iter()?
        .map(|record| Ok(cached_declaration_values(&read_declaration(&record?)?)))
        .collect::<PyResult<_>>()?;
    let canonical: &[usize] = &native.get().lookup.declarations;
    if rows.len() != canonical.len() {
        return Err(PyValueError::new_err(
            "scope report values must match the lookup's declarations",
        ));
    }
    let mut declarations = vec![Default::default(); canonical.len()];
    for (position, row) in canonical.iter().zip(rows) {
        declarations[*position] = row;
    }
    Ok(LookupReports {
        lookup: native,
        values: ReportValues {
            declarations,
            completeness: cached_completeness(read_completeness(&index)?),
        },
    })
}

pub(crate) fn scope_error(py: Python<'_>, class: ErrorId, message: String) -> PyResult<PyErr> {
    Ok(PyErr::from_value(
        error_class(py, class)?.call1((message,))?,
    ))
}

/// The error Python raises for a failed query.
pub(crate) fn failure_error(py: Python<'_>, failure: &ReportFailure) -> PyResult<PyErr> {
    match failure {
        ReportFailure::Path(rejected) => scope_error(
            py,
            ErrorId::InvalidScopePath,
            rejected_path_message(rejected),
        ),
        ReportFailure::Identity(failure, text) => scope_error(
            py,
            ErrorId::InvalidQualifiedIdentity,
            identity_failure_message(failure, text),
        ),
        ReportFailure::Serialization(message) => Ok(PyValueError::new_err(message.clone())),
        ReportFailure::IndexOutOfRange => Ok(PyIndexError::new_err("tuple index out of range")),
    }
}

pub(crate) fn path_text(value: Option<Bound<'_, PyAny>>) -> PyResult<Option<String>> {
    value
        .filter(|value| !value.is_none())
        .map(|value| value.str()?.extract())
        .transpose()
}

pub(crate) fn raised(diagnostics: Vec<DiagnosticFacts>) -> Vec<ReportDiagnostic> {
    diagnostics
        .into_iter()
        .map(ReportDiagnostic::Raised)
        .collect()
}

pub(crate) fn folder_request(
    classes: &ScopeClasses<'_>,
    location: (String, Option<Bound<'_, PyAny>>, bool),
    filters: Option<Bound<'_, PyAny>>,
) -> PyResult<FolderRequest> {
    let (folder, target, prospective) = location;
    Ok(FolderRequest {
        folder,
        target: read_optional_target(classes, target.as_ref())?,
        prospective,
        filters: filters
            .filter(|filters| !filters.is_none())
            .map(|filters| read_filters(&filters))
            .transpose()?,
    })
}

pub(crate) fn serialized(
    classes: &ScopeClasses<'_>,
    result: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let result: ScopeResult = read_result(classes, result)?;
    serialize_scope_result(&result).map_err(|failure| match failure {
        ReportFailure::Serialization(message) => PyValueError::new_err(message),
        other => PyValueError::new_err(format!("{other:?}")),
    })
}

/// `scope_metadata_projection` of one `ScopeIndex`, as the dictionary Python builds.
#[pyfunction]
#[pyo3(name = "scope_metadata_projection", signature = (*, index))]
pub(crate) fn metadata_projection(py: Python<'_>, index: &Bound<'_, PyAny>) -> PyResult<PyObject> {
    json_to_python(py, projection_json(index)?, false)
}

pub(crate) fn projection_json(index: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    let index: ProjectionIndex = cached_projection_index(read_index(index)?);
    Ok(scope_metadata_projection(&index))
}

fn optional<'py>(
    arguments: &KeywordArguments<'py>,
    name: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    Ok(Some(arguments.value(name)?).filter(|value| !value.is_none()))
}

/// Python's `query_scope_report`.
#[pyfunction]
#[pyo3(name = "query_scope_report", signature = (**options))]
pub(crate) fn query_scope_report_binding<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<PyObject> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "query_scope_report",
        options,
        (&["lookup"], &["target", "at", "directory", "filters"]),
    )?;
    scope_report(
        py,
        &arguments.value("lookup")?,
        ReportArguments {
            target: optional(&arguments, "target")?,
            at: optional(&arguments, "at")?,
            directory: arguments.flag("directory")?,
            filters: optional(&arguments, "filters")?,
        },
    )
}

/// Python's `explain_scope_declaration`.
#[pyfunction]
#[pyo3(name = "explain_scope_declaration", signature = (**options))]
pub(crate) fn explain_scope_declaration_binding<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<(PyObject, PyObject)> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "explain_scope_declaration",
        options,
        (&["lookup", "declaration"], &["target", "at", "directory"]),
    )?;
    scope_explain(
        py,
        &arguments.value("lookup")?,
        ExplainArguments {
            declaration: arguments.value("declaration")?,
            target: optional(&arguments, "target")?,
            at: optional(&arguments, "at")?,
            directory: arguments.flag("directory")?,
        },
    )
}

/// Python's `browse_scope_folders`.
#[pyfunction]
#[pyo3(name = "browse_scope_folders", signature = (**options))]
pub(crate) fn browse_scope_folders_binding<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<PyObject> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "browse_scope_folders",
        options,
        (&["lookup"], &["folder", "target", "target_is_prospective"]),
    )?;
    let folder: String = match optional(&arguments, "folder")? {
        Some(folder) => folder.extract()?,
        None => CURRENT_FOLDER.to_owned(),
    };
    scope_browse(
        py,
        &arguments.value("lookup")?,
        (
            folder,
            optional(&arguments, "target")?,
            arguments.flag("target_is_prospective")?,
        ),
    )
}

/// Python's `list_scope_declarations`.
#[pyfunction]
#[pyo3(name = "list_scope_declarations", signature = (**options))]
pub(crate) fn list_scope_declarations_binding<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<PyObject> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "list_scope_declarations",
        options,
        (
            &["lookup", "folder"],
            &["filters", "target", "target_is_prospective"],
        ),
    )?;
    scope_list(
        py,
        &arguments.value("lookup")?,
        ListArguments {
            folder: arguments.value("folder")?.extract()?,
            filters: optional(&arguments, "filters")?,
            target: optional(&arguments, "target")?,
            target_is_prospective: arguments.flag("target_is_prospective")?,
        },
    )
}
