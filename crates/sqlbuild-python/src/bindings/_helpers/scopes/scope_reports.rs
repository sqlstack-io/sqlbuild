//! `sqb scope` reports over a `ScopeLookup`, their serialization and the metadata projection.

use pyo3::exceptions::{PyIndexError, PyValueError};
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::PyDict;
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::json::models::JsonValue;
use sqlbuild_scopes::scope_index::main::identity_failure_message::identity_failure_message;
use sqlbuild_scopes::scope_index::main::rejected_path_message::rejected_path_message;
use sqlbuild_scopes::scope_report::main::browse_scope_folders::browse_scope_folders;
use sqlbuild_scopes::scope_report::main::build_scope_report::build_scope_report;
use sqlbuild_scopes::scope_report::main::cached_completeness::cached_completeness;
use sqlbuild_scopes::scope_report::main::cached_declaration_values::cached_declaration_values;
use sqlbuild_scopes::scope_report::main::cached_projection_index::cached_projection_index;
use sqlbuild_scopes::scope_report::main::explain_scope_declaration::explain_scope_declaration;
use sqlbuild_scopes::scope_report::main::list_scope_declarations::list_scope_declarations;
use sqlbuild_scopes::scope_report::main::preview_scope_move::preview_scope_move;
use sqlbuild_scopes::scope_report::main::rebound_scope_usages::rebound_scope_usages;
use sqlbuild_scopes::scope_report::main::scope_metadata_projection::scope_metadata_projection;
use sqlbuild_scopes::scope_report::main::serialize_scope_result::serialize_scope_result;
use sqlbuild_scopes::scope_report::models::{
    DeclarationValues, DiagnosticFacts, ExplainRequest, FolderRequest, MoveRequest,
    ProjectionIndex, ReportDiagnostic, ReportFailure, ReportRequest, ReportSource, ReportValues,
    ScopeResult,
};

use crate::bindings::_helpers::scopes::scope_lookup::NativeScopeLookup;
use crate::bindings::_helpers::scopes::scope_objects::{
    ScopeClasses, read_completeness, read_declaration, read_index,
};
use crate::bindings::_helpers::scopes::scope_report_objects::{
    browse_object, diagnostics_object, explanation_object, json_to_plain, json_to_python,
    list_object, move_object, read_declaration_target, read_filters, read_optional_target,
    read_result, read_target, report_object,
};
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

type PyObject = Py<PyAny>;

/// Target, location, whether it is a directory, and the filters.
pub(crate) struct ReportArguments<'py> {
    pub(crate) target: Option<Bound<'py, PyAny>>,
    pub(crate) at: Option<Bound<'py, PyAny>>,
    pub(crate) directory: bool,
    pub(crate) filters: Option<Bound<'py, PyAny>>,
}
/// Declaration, target, location and whether it is a directory.
pub(crate) struct ExplainArguments<'py> {
    pub(crate) declaration: Bound<'py, PyAny>,
    pub(crate) target: Option<Bound<'py, PyAny>>,
    pub(crate) at: Option<Bound<'py, PyAny>>,
    pub(crate) directory: bool,
}
/// Folder, filters, target and whether the target is prospective.
pub(crate) struct ListArguments<'py> {
    pub(crate) folder: String,
    pub(crate) filters: Option<Bound<'py, PyAny>>,
    pub(crate) target: Option<Bound<'py, PyAny>>,
    pub(crate) target_is_prospective: bool,
}

const CURRENT_FOLDER: &str = ".";

/// One lookup's native lookup and every declaration's values in input order.
struct LookupReports {
    lookup: Py<NativeScopeLookup>,
    values: ReportValues,
}

impl LookupReports {
    fn source<'a>(&'a self, lookup: &'a NativeScopeLookup) -> ReportSource<'a> {
        ReportSource {
            records: &lookup.records,
            lookup: &lookup.lookup,
            values: &self.values,
        }
    }
}

/// The reports of a `ScopeLookup`: its native lookup with its canonical declarations' values.
fn lookup_reports(lookup: &Bound<'_, PyAny>) -> PyResult<LookupReports> {
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

fn scope_error(py: Python<'_>, class: ErrorId, message: String) -> PyResult<PyErr> {
    Ok(PyErr::from_value(
        error_class(py, class)?.call1((message,))?,
    ))
}

/// The error Python raises for a failed query.
fn failure_error(py: Python<'_>, failure: &ReportFailure) -> PyResult<PyErr> {
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

fn path_text(value: Option<Bound<'_, PyAny>>) -> PyResult<Option<String>> {
    value
        .filter(|value| !value.is_none())
        .map(|value| value.str()?.extract())
        .transpose()
}

fn raised(diagnostics: Vec<DiagnosticFacts>) -> Vec<ReportDiagnostic> {
    diagnostics
        .into_iter()
        .map(ReportDiagnostic::Raised)
        .collect()
}

/// `query_scope_report`: one existing target or prospective path; invalid paths raise.
pub(crate) fn scope_report(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    request: ReportArguments<'_>,
) -> PyResult<PyObject> {
    let ReportArguments {
        target,
        at,
        directory,
        filters,
    } = request;
    let classes: ScopeClasses<'_> = ScopeClasses::new(py)?;
    let selected: Bound<'_, PyAny> = match filters.filter(|filters| !filters.is_none()) {
        Some(filters) => filters,
        None => classes.model(RecordId::ScopeReportFilters)?.call0()?,
    };
    let reports: LookupReports = lookup_reports(lookup)?;
    let request: ReportRequest = ReportRequest {
        target: read_optional_target(&classes, target.as_ref())?,
        at: path_text(at)?,
        directory,
        filters: read_filters(&selected)?,
    };
    let native: &NativeScopeLookup = reports.lookup.get();
    match build_scope_report(reports.source(native), &request) {
        Ok(report) => {
            let diagnostics: Bound<'_, PyAny> =
                native_field(&native_field(lookup, "index")?, "diagnostics")?;
            Ok(report_object(&classes, report, (&selected, &diagnostics))?.unbind())
        }
        Err(failure) => Err(failure_error(py, &failure)?),
    }
}

/// `explain_scope_declaration`: one qualified declaration in an optional context.
pub(crate) fn scope_explain(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    request: ExplainArguments<'_>,
) -> PyResult<(PyObject, PyObject)> {
    let ExplainArguments {
        declaration,
        target,
        at,
        directory,
    } = request;
    let classes: ScopeClasses<'_> = ScopeClasses::new(py)?;
    let reports: LookupReports = lookup_reports(lookup)?;
    let request: ExplainRequest = ExplainRequest {
        declaration: read_declaration_target(&classes, &declaration)?,
        target: read_optional_target(&classes, target.as_ref())?,
        at: path_text(at)?,
        directory,
    };
    let native: &NativeScopeLookup = reports.lookup.get();
    match explain_scope_declaration(reports.source(native), &request) {
        Ok((explanation, diagnostics)) => {
            let index_diagnostics: Bound<'_, PyAny> =
                native_field(&native_field(lookup, "index")?, "diagnostics")?;
            Ok((
                explanation_object(&classes, explanation)?.unbind(),
                diagnostics_object(&classes, &index_diagnostics, raised(diagnostics))?.unbind(),
            ))
        }
        Err(failure) => Err(failure_error(py, &failure)?),
    }
}

fn folder_request(
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

/// `browse_scope_folders`: the direct child declaration-definition folders of `folder`.
pub(crate) fn scope_browse(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    request: (String, Option<Bound<'_, PyAny>>, bool),
) -> PyResult<PyObject> {
    let classes: ScopeClasses<'_> = ScopeClasses::new(py)?;
    let reports: LookupReports = lookup_reports(lookup)?;
    let request: FolderRequest = folder_request(&classes, request, None)?;
    let native: &NativeScopeLookup = reports.lookup.get();
    let result = browse_scope_folders(reports.source(native), &request);
    let diagnostics: Bound<'_, PyAny> =
        native_field(&native_field(lookup, "index")?, "diagnostics")?;
    Ok(browse_object(&classes, result, &diagnostics)?.unbind())
}

/// `list_scope_declarations`: declarations recursively beneath one definition folder.
pub(crate) fn scope_list(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    request: ListArguments<'_>,
) -> PyResult<PyObject> {
    let ListArguments {
        folder,
        filters,
        target,
        target_is_prospective,
    } = request;
    let classes: ScopeClasses<'_> = ScopeClasses::new(py)?;
    let reports: LookupReports = lookup_reports(lookup)?;
    let request: FolderRequest =
        folder_request(&classes, (folder, target, target_is_prospective), filters)?;
    let native: &NativeScopeLookup = reports.lookup.get();
    match list_scope_declarations(reports.source(native), &request) {
        Ok(result) => {
            let diagnostics: Bound<'_, PyAny> =
                native_field(&native_field(lookup, "index")?, "diagnostics")?;
            Ok(list_object(&classes, result, &diagnostics)?.unbind())
        }
        Err(failure) => Err(failure_error(py, &failure)?),
    }
}

/// `preview_scope_move`: visibility deltas of a resource move, without mutating the lookup.
#[pyfunction]
#[pyo3(name = "preview_scope_move", signature = (*, lookup, resource, destination))]
pub(crate) fn scope_move_preview(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    resource: &Bound<'_, PyAny>,
    destination: &Bound<'_, PyAny>,
) -> PyResult<(Option<PyObject>, PyObject)> {
    let classes: ScopeClasses<'_> = ScopeClasses::new(py)?;
    let reports: LookupReports = lookup_reports(lookup)?;
    let request: MoveRequest = MoveRequest {
        resource: read_target(&classes, resource)?,
        destination: destination.str()?.extract()?,
    };
    let native: &NativeScopeLookup = reports.lookup.get();
    match preview_scope_move(reports.source(native), &request) {
        Ok((preview, diagnostics)) => {
            let index_diagnostics: Bound<'_, PyAny> =
                native_field(&native_field(lookup, "index")?, "diagnostics")?;
            Ok((
                preview
                    .map(|preview| Ok::<_, PyErr>(move_object(&classes, preview)?.unbind()))
                    .transpose()?,
                diagnostics_object(&classes, &index_diagnostics, raised(diagnostics))?.unbind(),
            ))
        }
        Err(failure) => Err(failure_error(py, &failure)?),
    }
}

/// Scoped declarations the resources at `path` use that resolve differently from `destination`.
pub(crate) fn scope_rebound_usages(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    paths: (&str, &str),
) -> PyResult<Option<Vec<String>>> {
    let reports: LookupReports = lookup_reports(lookup)?;
    let native: &NativeScopeLookup = reports.lookup.get();
    match rebound_scope_usages(reports.source(native), paths) {
        Ok(rebound) => Ok(rebound),
        Err(failure) => Err(failure_error(py, &failure)?),
    }
}

fn serialized(classes: &ScopeClasses<'_>, result: &Bound<'_, PyAny>) -> PyResult<String> {
    let result: ScopeResult = read_result(classes, result)?;
    serialize_scope_result(&result).map_err(|failure| match failure {
        ReportFailure::Serialization(message) => PyValueError::new_err(message),
        other => PyValueError::new_err(format!("{other:?}")),
    })
}

/// `serialize_scope_report`: deterministic schema-version-two ASCII JSON of one result.
#[pyfunction]
#[pyo3(signature = (*, report))]
pub(crate) fn serialize_scope_report(
    py: Python<'_>,
    report: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let classes: ScopeClasses<'_> = ScopeClasses::new(py)?;
    match serialized(&classes, report) {
        Err(error) if error.is_instance_of::<PyValueError>(py) => {
            let message: String = error.value(py).str()?.extract()?;
            let raised: PyErr = scope_error(py, ErrorId::Scope, message)?;
            raised.set_cause(py, Some(error));
            Err(raised)
        }
        other => other,
    }
}

/// `scope_metadata_projection` of one `ScopeIndex`, as the dictionary Python builds.
#[pyfunction]
#[pyo3(name = "scope_metadata_projection", signature = (*, index))]
pub(crate) fn metadata_projection(py: Python<'_>, index: &Bound<'_, PyAny>) -> PyResult<PyObject> {
    json_to_python(py, projection_json(index)?, false)
}

/// `plain_value` of `scope_metadata_projection(index=index)`, without a Python round trip.
pub(crate) fn metadata_projection_value(index: &Bound<'_, PyAny>) -> PyResult<serde_json::Value> {
    json_to_plain(projection_json(index)?)
}

fn projection_json(index: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
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
fn query_scope_report_binding<'py>(
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
fn explain_scope_declaration_binding<'py>(
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
fn browse_scope_folders_binding<'py>(
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
fn list_scope_declarations_binding<'py>(
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(query_scope_report_binding, module)?)?;
    module.add_function(wrap_pyfunction!(explain_scope_declaration_binding, module)?)?;
    module.add_function(wrap_pyfunction!(browse_scope_folders_binding, module)?)?;
    module.add_function(wrap_pyfunction!(list_scope_declarations_binding, module)?)?;
    module.add_function(wrap_pyfunction!(scope_move_preview, module)?)?;
    module.add_function(wrap_pyfunction!(serialize_scope_report, module)?)?;
    module.add_function(wrap_pyfunction!(metadata_projection, module)?)?;
    Ok(())
}
