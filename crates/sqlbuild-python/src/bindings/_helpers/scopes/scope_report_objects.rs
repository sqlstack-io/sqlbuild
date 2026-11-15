//! Scope report requests read from, and results built as, the Python report models.

use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{
    PyBool, PyBoolMethods, PyDict, PyDictMethods, PyFloat, PyInt, PyList, PyString, PyTuple,
    PyTypeMethods,
};
use sqlbuild_core::json::models::{JsonInteger, JsonValue};
use sqlbuild_scopes::scope_report::main::cached_diagnostic_facts::cached_diagnostic_facts;
use sqlbuild_scopes::scope_report::models::{
    BrowseResult, DeclarationReport, DeclarationTarget, Explanation, ListResult, MovePreview,
    Provenance, QueryTarget, ReportDiagnostic, ReportFilters, ResourceReport, ScopeChainEntry,
    ScopeFolder, ScopeReport, ScopeResult, ScopeSection, SourceLocation,
};

use crate::bindings::_helpers::scopes::scope_objects::{
    ScopeClasses, field, optional_item, read_declaration_identity, read_diagnostic, read_qualified,
};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

type PyObject = Py<PyAny>;

const PATHLIB_MODULE: &str = "pathlib";
/// The error `plain_value` raises for an integer outside 64 bits.
const WIDE_INTEGER: &str = "Integer exceeds 64-bit range";

fn texts(item: &Bound<'_, PyAny>, name: &str) -> PyResult<Vec<String>> {
    native_field(item, name)?
        .try_iter()?
        .map(|value| value?.str()?.extract())
        .collect()
}

/// A query target: an identity model, else its text, read as a path when it is a `PurePath`.
pub(crate) fn read_target(
    classes: &ScopeClasses<'_>,
    target: &Bound<'_, PyAny>,
) -> PyResult<QueryTarget> {
    if target.is_instance(&classes.model(RecordId::ResourceIdentity)?)?
        || target.is_instance(&classes.model(RecordId::DeclarationIdentity)?)?
    {
        return Ok(QueryTarget::Identity(read_qualified(target)?));
    }
    let text: String = target.str()?.extract()?;
    let pure_path: Bound<'_, PyAny> = target.py().import(PATHLIB_MODULE)?.getattr("PurePath")?;
    Ok(if target.is_instance(&pure_path)? {
        QueryTarget::Path(text)
    } else {
        QueryTarget::Text(text)
    })
}

/// An optional query target.
pub(crate) fn read_optional_target(
    classes: &ScopeClasses<'_>,
    target: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<QueryTarget>> {
    target
        .filter(|target| !target.is_none())
        .map(|target| read_target(classes, target))
        .transpose()
}

/// A declaration to explain: a `DeclarationIdentity`, else its text.
pub(crate) fn read_declaration_target(
    classes: &ScopeClasses<'_>,
    declaration: &Bound<'_, PyAny>,
) -> PyResult<DeclarationTarget> {
    if declaration.is_instance(&classes.model(RecordId::DeclarationIdentity)?)? {
        return Ok(DeclarationTarget::Identity(read_declaration_identity(
            declaration,
        )?));
    }
    Ok(DeclarationTarget::Text(declaration.str()?.extract()?))
}

/// `ScopeReportFilters`, with each kind as its text.
pub(crate) fn read_filters(filters: &Bound<'_, PyAny>) -> PyResult<ReportFilters> {
    Ok(ReportFilters {
        sections: field(filters, "sections")?,
        include_nearby: field(filters, "include_nearby")?,
        defined_under: field(filters, "defined_under")?,
        kinds: texts(filters, "kinds")?,
        glob: field(filters, "glob")?,
        used_only: field(filters, "used_only")?,
        dependency_depth: field(filters, "dependency_depth")?,
        sort: field(filters, "sort")?,
        cursor: field(filters, "cursor")?,
        page_size: field(filters, "page_size")?,
        nearby_depth: field(filters, "nearby_depth")?,
        globals: field(filters, "globals")?,
    })
}

/// A JSON value as Python builds it; arrays become tuples or lists.
pub(crate) fn json_to_python(py: Python<'_>, value: JsonValue, tuples: bool) -> PyResult<PyObject> {
    match value {
        JsonValue::Null => Ok(py.None()),
        JsonValue::Bool(flag) => flag.into_py_any(py),
        JsonValue::Integer(integer) => match integer.to_i128() {
            Some(number) => number.into_py_any(py),
            None => Ok(py
                .import("builtins")?
                .getattr("int")?
                .call1((integer.as_str(),))?
                .unbind()),
        },
        JsonValue::Float(number) => number.into_py_any(py),
        JsonValue::String(text) => text.into_py_any(py),
        JsonValue::Utf16(units) => String::from_utf16_lossy(&units).into_py_any(py),
        JsonValue::Array(items) => {
            let values: Vec<PyObject> = items
                .into_iter()
                .map(|item| json_to_python(py, item, tuples))
                .collect::<PyResult<_>>()?;
            if tuples {
                PyTuple::new(py, values)?.into_py_any(py)
            } else {
                PyList::new(py, values)?.into_py_any(py)
            }
        }
        JsonValue::Object(entries) => {
            let dictionary: Bound<'_, PyDict> = PyDict::new(py);
            for (key, item) in entries {
                dictionary.set_item(key, json_to_python(py, item, tuples)?)?;
            }
            dictionary.into_py_any(py)
        }
    }
}

/// `plain_value(int(integer))`: a 64-bit signed or unsigned number, else the wide-integer error.
fn plain_integer(integer: &JsonInteger) -> PyResult<serde_json::Value> {
    let Some(number) = integer.to_i128() else {
        return Err(PyValueError::new_err(WIDE_INTEGER));
    };
    if let Ok(signed) = i64::try_from(number) {
        return Ok(serde_json::Value::from(signed));
    }
    match u64::try_from(number) {
        Ok(unsigned) => Ok(serde_json::Value::from(unsigned)),
        Err(_) => Err(PyValueError::new_err(WIDE_INTEGER)),
    }
}

/// `plain_value(json_to_python(value, False))` computed without building the Python value.
pub(crate) fn json_to_plain(value: JsonValue) -> PyResult<serde_json::Value> {
    Ok(match value {
        JsonValue::Null => serde_json::Value::Null,
        JsonValue::Bool(flag) => serde_json::Value::Bool(flag),
        JsonValue::Integer(integer) => plain_integer(&integer)?,
        JsonValue::Float(number) => serde_json::Number::from_f64(number)
            .map_or(serde_json::Value::Null, serde_json::Value::Number),
        JsonValue::String(text) => serde_json::Value::String(text),
        JsonValue::Utf16(units) => serde_json::Value::String(String::from_utf16_lossy(&units)),
        JsonValue::Array(items) => serde_json::Value::Array(
            items
                .into_iter()
                .map(json_to_plain)
                .collect::<PyResult<_>>()?,
        ),
        JsonValue::Object(entries) => {
            let mut object: std::collections::BTreeMap<String, serde_json::Value> =
                std::collections::BTreeMap::new();
            for (key, item) in entries {
                let _replaced = object.insert(key, json_to_plain(item)?);
            }
            serde_json::Value::Object(object.into_iter().collect())
        }
    })
}

/// `_value` of one metadata value; an unsupported type is the error Python's serializer raises.
pub(crate) fn python_to_json(value: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    if value.is_none() {
        return Ok(JsonValue::Null);
    }
    if let Ok(flag) = value.downcast::<PyBool>() {
        return Ok(JsonValue::Bool(flag.is_true()));
    }
    let py: Python<'_> = value.py();
    if py
        .import("enum")?
        .getattr("Enum")
        .and_then(|enum_type| value.is_instance(&enum_type))?
    {
        return python_to_json(&native_field(value, "value")?);
    }
    if value.is_instance_of::<PyInt>() {
        let decimal: String = value.str()?.extract()?;
        return JsonInteger::parse(&decimal)
            .map(JsonValue::Integer)
            .ok_or_else(|| PyValueError::new_err("Unsupported scope report value: int"));
    }
    if value.is_instance_of::<PyFloat>() {
        return Ok(JsonValue::Float(value.extract::<f64>()?));
    }
    if value.is_instance_of::<PyString>() {
        return Ok(JsonValue::String(value.extract()?));
    }
    let pure_path = py.import("pathlib")?.getattr("PurePath")?;
    if value.is_instance(&pure_path)? {
        return Ok(JsonValue::String(
            value.call_method0("as_posix")?.extract()?,
        ));
    }
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        let mut items: Vec<JsonValue> = Vec::new();
        for item in value.try_iter()? {
            items.push(python_to_json(&item?)?);
        }
        return Ok(JsonValue::Array(items));
    }
    if let Ok(mapping) = value.downcast::<PyDict>() {
        let mut entries: Vec<(String, JsonValue)> = Vec::new();
        for (key, item) in mapping.iter() {
            entries.push((key.str()?.extract()?, python_to_json(&item)?));
        }
        return Ok(JsonValue::Object(entries));
    }
    let name: String = value.get_type().name()?.extract()?;
    Err(PyValueError::new_err(format!(
        "Unsupported scope report value: {name}"
    )))
}

/// `{str(key): value}` with later duplicate keys replacing earlier values in place.
fn unique_entries(entries: Vec<(String, JsonValue)>) -> Vec<(String, JsonValue)> {
    let mut unique: Vec<(String, JsonValue)> = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        match unique.iter_mut().find(|(existing, _)| *existing == key) {
            Some(entry) => entry.1 = value,
            None => unique.push((key, value)),
        }
    }
    unique
}

fn tuple_of<'py, T>(
    py: Python<'py>,
    values: Vec<T>,
    build: impl Fn(T) -> PyResult<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let built: Vec<Bound<'py, PyAny>> = values.into_iter().map(build).collect::<PyResult<_>>()?;
    Ok(PyTuple::new(py, built)?.into_any())
}

fn declaration_object<'py>(
    classes: &ScopeClasses<'py>,
    report: DeclarationReport,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = classes.py();
    let metadata: Bound<'py, PyAny> = tuple_of(py, report.metadata, |(key, value)| {
        Ok(PyTuple::new(py, [key.into_py_any(py)?, json_to_python(py, value, true)?])?.into_any())
    })?;
    let visibility: Option<Bound<'py, PyAny>> = report
        .visibility
        .map(|provenance| {
            classes
                .model(RecordId::VisibilityProvenance)?
                .call1((provenance.reason, provenance.through))
        })
        .transpose()?;
    let head: [Bound<'py, PyAny>; 7] = [
        report.identity.into_bound_py_any(py)?,
        report.kind.into_bound_py_any(py)?,
        report.name.into_bound_py_any(py)?,
        report.owner.into_bound_py_any(py)?,
        classes.model(RecordId::ScopeSourceLocation)?.call1((
            report.definition.path,
            report.definition.line,
            report.definition.column,
        ))?,
        report.scope.into_bound_py_any(py)?,
        report.owning_path.into_bound_py_any(py)?,
    ];
    let standing: [Bound<'py, PyAny>; 3] = [
        visibility.into_bound_py_any(py)?,
        report.inaccessible_reason.into_bound_py_any(py)?,
        metadata,
    ];
    let links: [Bound<'py, PyAny>; 6] = [
        PyTuple::new(py, report.consumers)?.into_any(),
        PyTuple::new(py, report.dependencies)?.into_any(),
        PyTuple::new(py, report.grants)?.into_any(),
        report.required_scope.into_bound_py_any(py)?,
        report.required_path.into_bound_py_any(py)?,
        PyTuple::new(py, report.promotion_impact)?.into_any(),
    ];
    let arguments: Vec<Bound<'py, PyAny>> = head.into_iter().chain(standing).chain(links).collect();
    classes
        .model(RecordId::DeclarationReport)?
        .call1(PyTuple::new(py, arguments)?)
}

fn declarations_object<'py>(
    classes: &ScopeClasses<'py>,
    reports: Vec<DeclarationReport>,
) -> PyResult<Bound<'py, PyAny>> {
    tuple_of(classes.py(), reports, |report| {
        declaration_object(classes, report)
    })
}

fn section_object<'py>(
    classes: &ScopeClasses<'py>,
    section: ScopeSection,
) -> PyResult<Bound<'py, PyAny>> {
    classes.model(RecordId::ScopeSection)?.call1((
        section.name,
        section.total,
        section.returned,
        section.collapsed,
        section.collapsed_count,
        section.truncated,
        section.complete,
        section.next_cursor,
        section.cursor,
        section.page_size,
    ))
}

/// The report's diagnostics: an index diagnostic of `index_diagnostics`, else a raised one.
pub(crate) fn diagnostics_object<'py>(
    classes: &ScopeClasses<'py>,
    index_diagnostics: &Bound<'py, PyAny>,
    diagnostics: Vec<ReportDiagnostic>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = classes.py();
    tuple_of(py, diagnostics, |diagnostic| match diagnostic {
        ReportDiagnostic::Index(position) => index_diagnostics.get_item(position),
        ReportDiagnostic::Raised(facts) => {
            let keywords: Bound<'py, PyDict> = PyDict::new(py);
            keywords.set_item("path", facts.path)?;
            classes.model(RecordId::ScopeDiagnostic)?.call(
                (
                    classes.member(RecordId::ScopeDiagnosticCode, &facts.code)?,
                    facts.message,
                ),
                Some(&keywords),
            )
        }
    })
}

/// A `DeclarationExplanation`.
pub(crate) fn explanation_object<'py>(
    classes: &ScopeClasses<'py>,
    explanation: Explanation,
) -> PyResult<Bound<'py, PyAny>> {
    let declaration: Option<Bound<'py, PyAny>> = explanation
        .declaration
        .map(|report| declaration_object(classes, report))
        .transpose()?;
    classes
        .model(RecordId::DeclarationExplanation)?
        .call1((declaration, explanation.complete))
}

/// A `MovePreview`.
pub(crate) fn move_object<'py>(
    classes: &ScopeClasses<'py>,
    preview: MovePreview,
) -> PyResult<Bound<'py, PyAny>> {
    classes.model(RecordId::MovePreview)?.call1((
        preview.resource,
        preview.destination,
        preview.new_ownership_root,
        declarations_object(classes, preview.retained)?,
        declarations_object(classes, preview.gained)?,
        declarations_object(classes, preview.lost)?,
        PyTuple::new(classes.py(), preview.invalidated_usages)?,
        declarations_object(classes, preview.private_retained)?,
        declarations_object(classes, preview.relationship_retained)?,
        preview.complete,
    ))
}

/// A `ScopeReport` carrying the caller's `filters` object.
pub(crate) fn report_object<'py>(
    classes: &ScopeClasses<'py>,
    report: ScopeReport,
    context: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let (filters, index_diagnostics) = context;
    let py: Python<'py> = classes.py();
    let resource: ResourceReport = report.resource;
    let chain_class: Bound<'py, PyAny> = classes.model(RecordId::ScopeChainEntry)?;
    let explanation: Option<Bound<'py, PyAny>> = report
        .explanation
        .map(|explanation| explanation_object(classes, explanation))
        .transpose()?;
    let move_preview: Option<Bound<'py, PyAny>> = report
        .move_preview
        .map(|preview| move_object(classes, preview))
        .transpose()?;
    classes.model(RecordId::ScopeReport)?.call1((
        classes.model(RecordId::ScopeResourceReport)?.call1((
            resource.target,
            resource.identity,
            resource.path,
            resource.prospective,
            resource.directory,
            resource.duplicate_count,
        ))?,
        tuple_of(py, report.scope_chain, |entry| {
            chain_class.call1((entry.kind, entry.path, entry.declaration_count))
        })?,
        declarations_object(classes, report.available)?,
        declarations_object(classes, report.used)?,
        declarations_object(classes, report.relationship_scope)?,
        declarations_object(classes, report.nearby_unavailable)?,
        filters,
        tuple_of(py, report.sections, |section| {
            section_object(classes, section)
        })?,
        explanation,
        move_preview,
        diagnostics_object(classes, index_diagnostics, report.diagnostics)?,
        report.complete,
    ))
}

/// A `ScopeBrowseResult`.
pub(crate) fn browse_object<'py>(
    classes: &ScopeClasses<'py>,
    result: BrowseResult,
    index_diagnostics: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = classes.py();
    let folder_class: Bound<'py, PyAny> = classes.model(RecordId::ScopeFolder)?;
    classes.model(RecordId::ScopeBrowseResult)?.call1((
        result.folder,
        tuple_of(py, result.folders, |folder| {
            folder_class.call1((
                folder.path,
                folder.name,
                folder.descendant_count,
                folder.child_count,
                folder.used_count,
                PyTuple::new(py, folder.kind_counts)?,
            ))
        })?,
        diagnostics_object(classes, index_diagnostics, result.diagnostics)?,
        result.complete,
    ))
}

/// A `ScopeListResult`.
pub(crate) fn list_object<'py>(
    classes: &ScopeClasses<'py>,
    result: ListResult,
    index_diagnostics: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    classes.model(RecordId::ScopeListResult)?.call1((
        result.folder,
        declarations_object(classes, result.declarations)?,
        section_object(classes, result.section)?,
        diagnostics_object(classes, index_diagnostics, result.diagnostics)?,
    ))
}

fn read_declaration_report(report: &Bound<'_, PyAny>) -> PyResult<DeclarationReport> {
    let definition: Bound<'_, PyAny> = native_field(report, "definition")?;
    let mut metadata: Vec<(String, JsonValue)> = Vec::new();
    for entry in native_field(report, "metadata")?.try_iter()? {
        let (key, value): (Bound<'_, PyAny>, Bound<'_, PyAny>) = entry?.extract()?;
        metadata.push((key.str()?.extract()?, python_to_json(&value)?));
    }
    Ok(DeclarationReport {
        identity: field(report, "identity")?,
        kind: field(report, "kind")?,
        name: field(report, "name")?,
        owner: field(report, "owner")?,
        definition: SourceLocation {
            path: field(&definition, "path")?,
            line: field(&definition, "line")?,
            column: field(&definition, "column")?,
        },
        scope: field(report, "scope")?,
        owning_path: field(report, "owning_path")?,
        visibility: optional_item(report, "visibility")?
            .map(|provenance| -> PyResult<Provenance> {
                Ok(Provenance {
                    reason: field(&provenance, "reason")?,
                    through: field(&provenance, "through")?,
                })
            })
            .transpose()?,
        inaccessible_reason: field(report, "inaccessible_reason")?,
        metadata: unique_entries(metadata),
        consumers: field(report, "consumers")?,
        dependencies: field(report, "dependencies")?,
        grants: field(report, "grants")?,
        required_scope: field(report, "required_scope")?,
        required_path: field(report, "required_path")?,
        promotion_impact: field(report, "promotion_impact")?,
    })
}

fn read_declaration_reports(
    item: &Bound<'_, PyAny>,
    name: &str,
) -> PyResult<Vec<DeclarationReport>> {
    native_field(item, name)?
        .try_iter()?
        .map(|report| read_declaration_report(&report?))
        .collect()
}

fn read_section(section: &Bound<'_, PyAny>) -> PyResult<ScopeSection> {
    Ok(ScopeSection {
        name: field(section, "name")?,
        total: field(section, "total")?,
        returned: field(section, "returned")?,
        collapsed: field(section, "collapsed")?,
        collapsed_count: field(section, "collapsed_count")?,
        truncated: field(section, "truncated")?,
        complete: field(section, "complete")?,
        next_cursor: field(section, "next_cursor")?,
        cursor: field(section, "cursor")?,
        page_size: field(section, "page_size")?,
    })
}

fn read_folder(folder: &Bound<'_, PyAny>) -> PyResult<ScopeFolder> {
    let mut kind_counts: Vec<(String, i64)> = Vec::new();
    for entry in native_field(folder, "kind_counts")?.try_iter()? {
        let (kind, count): (Bound<'_, PyAny>, i64) = entry?.extract()?;
        let kind: String = kind.str()?.extract()?;
        match kind_counts
            .iter_mut()
            .find(|(existing, _)| *existing == kind)
        {
            Some(entry) => entry.1 = count,
            None => kind_counts.push((kind, count)),
        }
    }
    Ok(ScopeFolder {
        path: field(folder, "path")?,
        name: field(folder, "name")?,
        descendant_count: field(folder, "descendant_count")?,
        child_count: field(folder, "child_count")?,
        used_count: field(folder, "used_count")?,
        kind_counts,
    })
}

fn read_diagnostics(item: &Bound<'_, PyAny>) -> PyResult<Vec<ReportDiagnostic>> {
    native_field(item, "diagnostics")?
        .try_iter()?
        .map(|diagnostic| {
            Ok(ReportDiagnostic::Raised(cached_diagnostic_facts(
                read_diagnostic(&diagnostic?)?,
            )))
        })
        .collect()
}

fn read_explanation(explanation: &Bound<'_, PyAny>) -> PyResult<Explanation> {
    Ok(Explanation {
        declaration: optional_item(explanation, "declaration")?
            .map(|report| read_declaration_report(&report))
            .transpose()?,
        complete: field(explanation, "complete")?,
    })
}

fn read_move(preview: &Bound<'_, PyAny>) -> PyResult<MovePreview> {
    Ok(MovePreview {
        resource: field(preview, "resource")?,
        destination: field(preview, "destination")?,
        new_ownership_root: field(preview, "new_ownership_root")?,
        retained: read_declaration_reports(preview, "retained")?,
        gained: read_declaration_reports(preview, "gained")?,
        lost: read_declaration_reports(preview, "lost")?,
        invalidated_usages: field(preview, "invalidated_usages")?,
        private_retained: read_declaration_reports(preview, "private_retained")?,
        relationship_retained: read_declaration_reports(preview, "relationship_retained")?,
        complete: field(preview, "complete")?,
    })
}

fn read_report(report: &Bound<'_, PyAny>) -> PyResult<ScopeReport> {
    let resource: Bound<'_, PyAny> = native_field(report, "resource")?;
    Ok(ScopeReport {
        resource: ResourceReport {
            target: field(&resource, "target")?,
            identity: field(&resource, "identity")?,
            path: field(&resource, "path")?,
            prospective: field(&resource, "prospective")?,
            directory: field(&resource, "directory")?,
            duplicate_count: field(&resource, "duplicate_count")?,
        },
        scope_chain: native_field(report, "scope_chain")?
            .try_iter()?
            .map(|entry| {
                let entry: Bound<'_, PyAny> = entry?;
                Ok(ScopeChainEntry {
                    kind: field(&entry, "kind")?,
                    path: field(&entry, "path")?,
                    declaration_count: field(&entry, "declaration_count")?,
                })
            })
            .collect::<PyResult<_>>()?,
        available: read_declaration_reports(report, "available")?,
        used: read_declaration_reports(report, "used")?,
        relationship_scope: read_declaration_reports(report, "relationship_scope")?,
        nearby_unavailable: read_declaration_reports(report, "nearby_unavailable")?,
        filters: read_filters(&native_field(report, "filters")?)?,
        sections: native_field(report, "sections")?
            .try_iter()?
            .map(|section| read_section(&section?))
            .collect::<PyResult<_>>()?,
        explanation: optional_item(report, "explanation")?
            .map(|explanation| read_explanation(&explanation))
            .transpose()?,
        move_preview: optional_item(report, "move_preview")?
            .map(|preview| read_move(&preview))
            .transpose()?,
        diagnostics: read_diagnostics(report)?,
        complete: field(report, "complete")?,
    })
}

/// A `ScopeReport`, `ScopeBrowseResult` or, otherwise, `ScopeListResult` as its native result.
pub(crate) fn read_result(
    classes: &ScopeClasses<'_>,
    result: &Bound<'_, PyAny>,
) -> PyResult<ScopeResult> {
    if result.is_instance(&classes.model(RecordId::ScopeReport)?)? {
        return Ok(ScopeResult::Report(Box::new(read_report(result)?)));
    }
    if result.is_instance(&classes.model(RecordId::ScopeBrowseResult)?)? {
        return Ok(ScopeResult::Browse(BrowseResult {
            folder: field(result, "folder")?,
            folders: native_field(result, "folders")?
                .try_iter()?
                .map(|folder| read_folder(&folder?))
                .collect::<PyResult<_>>()?,
            diagnostics: read_diagnostics(result)?,
            complete: field(result, "complete")?,
        }));
    }
    Ok(ScopeResult::List(ListResult {
        folder: field(result, "folder")?,
        declarations: read_declaration_reports(result, "declarations")?,
        section: read_section(&native_field(result, "section")?)?,
        diagnostics: read_diagnostics(result)?,
    }))
}
