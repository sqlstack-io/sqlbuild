//! The partial scope index of a project that failed strict discovery, with its fault diagnostics.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple, PyTupleMethods};
use sqlbuild_scopes::placement::main::diagnostic_order::diagnostic_order;
use sqlbuild_scopes::placement::models::DiagnosticOrderKey;
use sqlbuild_scopes::scope_index::models::{
    OwnershipRoot, OwnershipRootKind, ResourceIdentity, ResourceKind, ResourceRecord,
};

use crate::scopes::_helpers::checks::scope_validation::replace;
use crate::scopes::_helpers::index::scope_objects::{
    enum_text, field, optional_item, read_resource, root_object, tuple_attribute,
};
use crate::scopes::models::ScopeClasses;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// Each authored resource root, in the order a prospective path is matched against them.
const PROSPECTIVE_ROOTS: [(ResourceKind, &str); 5] = [
    (ResourceKind::Model, "sql/models"),
    (ResourceKind::Test, "tests/unit"),
    (ResourceKind::Scenario, "tests/scenarios"),
    (ResourceKind::Function, "sql/functions/sql"),
    (ResourceKind::Source, "sql/sources"),
];
const RESOURCE_PARSE_ERROR: &str = "S019";
const DECLARATION_PARSE_ERROR: &str = "S020";
const MACRO_PARSE_ERROR: &str = "S021";
const RELATIONSHIP_PARSE_ERROR: &str = "S022";
const CONFIG_PARSE_ERROR: &str = "S023";

fn posix(path: &Bound<'_, PyAny>) -> PyResult<String> {
    path.call_method0("as_posix")?.extract()
}

fn items<'py>(item: &Bound<'py, PyAny>, name: &str) -> PyResult<Vec<Bound<'py, PyAny>>> {
    native_field(item, name)?.try_iter()?.collect()
}

/// New diagnostics with their order keys.
struct Diagnostics<'py> {
    classes: ScopeClasses<'py>,
    objects: Vec<Bound<'py, PyAny>>,
    keys: Vec<DiagnosticOrderKey>,
}

impl<'py> Diagnostics<'py> {
    fn push(&mut self, code: &str, message: String, path: Option<String>) -> PyResult<()> {
        let keywords: Bound<'py, PyDict> = PyDict::new(self.classes.py());
        keywords.set_item("path", path.as_deref())?;
        self.objects
            .push(self.classes.model(RecordId::ScopeDiagnostic)?.call(
                (
                    self.classes.member(RecordId::ScopeDiagnosticCode, code)?,
                    message.as_str(),
                ),
                Some(&keywords),
            )?);
        self.keys.push(DiagnosticOrderKey {
            path,
            line: None,
            column: None,
            code: code.to_owned(),
            message,
        });
        Ok(())
    }

    /// One `ScopeDiagnostic` per discovery fault, at the fault's POSIX path.
    fn discovery_faults(&mut self, faults: &[Bound<'py, PyAny>], code: &str) -> PyResult<()> {
        for fault in faults {
            let path: Option<String> = optional_item(fault, "path")?
                .map(|path| posix(&path))
                .transpose()?;
            self.push(code, field(fault, "message")?, path)?;
        }
        Ok(())
    }
}

fn order_key(diagnostic: &Bound<'_, PyAny>) -> PyResult<DiagnosticOrderKey> {
    Ok(DiagnosticOrderKey {
        path: field(diagnostic, "path")?,
        line: field(diagnostic, "line")?,
        column: field(diagnostic, "column")?,
        code: enum_text(&native_field(diagnostic, "code")?)?,
        message: field(diagnostic, "message")?,
    })
}

/// The resource a faulty authored file would declare, when it sits below a resource root.
fn prospective_resource(path: &Bound<'_, PyAny>) -> PyResult<Option<ResourceRecord>> {
    let relative: String = posix(path)?;
    for (kind, root) in PROSPECTIVE_ROOTS {
        if relative.starts_with(&format!("{root}/")) {
            return Ok(Some(ResourceRecord {
                identity: ResourceIdentity {
                    kind,
                    name: native_field(path, "stem")?.extract()?,
                },
                path: relative,
                ownership_root: OwnershipRoot {
                    path: root.to_owned(),
                    kind: OwnershipRootKind::Resource,
                    resource_kind: Some(kind),
                },
            }));
        }
    }
    Ok(None)
}

fn resource_object<'py>(
    classes: &ScopeClasses<'py>,
    record: &ResourceRecord,
) -> PyResult<Bound<'py, PyAny>> {
    classes.model(RecordId::ResourceRecord)?.call1((
        classes.model(RecordId::ResourceIdentity)?.call1((
            classes.member(RecordId::ResourceKind, record.identity.kind.as_str())?,
            record.identity.name.as_str(),
        ))?,
        record.path.as_str(),
        root_object(classes, &record.ownership_root)?,
    ))
}

/// The index's resources and each faulty file's prospective resource, distinct and sorted.
fn tolerant_resources<'py>(
    classes: &ScopeClasses<'py>,
    index: &Bound<'py, PyAny>,
    faults: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = classes.py();
    let mut records: Vec<(ResourceRecord, Bound<'py, PyAny>)> = Vec::new();
    for resource in tuple_attribute(index, "resources")?.iter() {
        records.push((read_resource(&resource)?, resource));
    }
    for fault in faults {
        let Some(path) = optional_item(fault, "path")? else {
            continue;
        };
        if let Some(record) = prospective_resource(&path)? {
            let object: Bound<'py, PyAny> = resource_object(classes, &record)?;
            records.push((record, object));
        }
    }
    let mut distinct: Vec<(ResourceRecord, Bound<'py, PyAny>)> = Vec::with_capacity(records.len());
    for (record, object) in records {
        if !distinct.iter().any(|(seen, _)| *seen == record) {
            distinct.push((record, object));
        }
    }
    distinct.sort_by(|(left, _), (right, _)| {
        (&left.path, left.identity.kind.as_str(), &left.identity.name).cmp(&(
            &right.path,
            right.identity.kind.as_str(),
            &right.identity.name,
        ))
    });
    PyTuple::new(py, distinct.into_iter().map(|(_, object)| object))
}

/// The tolerant index: fault diagnostics, prospective resources and partial completeness.
pub(crate) fn scope_tolerant_index(
    py: Python<'_>,
    index: &Bound<'_, PyAny>,
    snapshot: &Bound<'_, PyAny>,
    faults: (Bound<'_, PyAny>, Bound<'_, PyAny>),
) -> PyResult<Py<PyAny>> {
    let (macro_faults, relationship_faults) = faults;
    let resource_faults: Vec<Bound<'_, PyAny>> = items(snapshot, "resource_faults")?;
    let declaration_faults: Vec<Bound<'_, PyAny>> = items(snapshot, "declaration_faults")?;
    let snapshot_relationship_faults: Vec<Bound<'_, PyAny>> =
        items(snapshot, "relationship_faults")?;
    let config_faults: Vec<Bound<'_, PyAny>> = items(snapshot, "config_faults")?;
    let macro_faults: Vec<Bound<'_, PyAny>> = macro_faults.try_iter()?.collect::<PyResult<_>>()?;
    let relationship_faults: Vec<(Bound<'_, PyAny>, String)> = relationship_faults
        .try_iter()?
        .map(|fault| fault?.extract())
        .collect::<PyResult<_>>()?;
    let mut diagnostics: Diagnostics<'_> = Diagnostics {
        classes: ScopeClasses::new(py)?,
        objects: Vec::new(),
        keys: Vec::new(),
    };
    for diagnostic in tuple_attribute(index, "diagnostics")?.iter() {
        diagnostics.keys.push(order_key(&diagnostic)?);
        diagnostics.objects.push(diagnostic);
    }
    diagnostics.discovery_faults(&resource_faults, RESOURCE_PARSE_ERROR)?;
    diagnostics.discovery_faults(&declaration_faults, DECLARATION_PARSE_ERROR)?;
    diagnostics.discovery_faults(&snapshot_relationship_faults, RELATIONSHIP_PARSE_ERROR)?;
    diagnostics.discovery_faults(&config_faults, CONFIG_PARSE_ERROR)?;
    for fault in &macro_faults {
        let path: String = posix(&native_field(fault, "relative_path")?)?;
        diagnostics.push(MACRO_PARSE_ERROR, field(fault, "message")?, Some(path))?;
    }
    for (path, message) in &relationship_faults {
        diagnostics.push(
            RELATIONSHIP_PARSE_ERROR,
            message.clone(),
            Some(posix(path)?),
        )?;
    }
    let mut prospective_faults: Vec<Bound<'_, PyAny>> = resource_faults.clone();
    prospective_faults.extend(snapshot_relationship_faults.iter().cloned());
    let classes: &ScopeClasses<'_> = &diagnostics.classes;
    let resources: Bound<'_, PyTuple> = tolerant_resources(classes, index, &prospective_faults)?;
    let ordered: Vec<Bound<'_, PyAny>> = diagnostic_order(&diagnostics.keys)
        .into_iter()
        .map(|position| diagnostics.objects[position].clone())
        .collect();
    let static_visibility: bool =
        declaration_faults.is_empty() && config_faults.is_empty() && macro_faults.is_empty();
    let relationships: bool =
        snapshot_relationship_faults.is_empty() && relationship_faults.is_empty();
    let completeness: Bound<'_, PyAny> = classes.model(RecordId::ScopeCompleteness)?.call1((
        false,
        static_visibility,
        false,
        relationships,
        false,
        false,
    ))?;
    Ok(replace(
        index,
        &[
            ("resources", resources.into_any()),
            ("diagnostics", PyTuple::new(py, ordered)?.into_any()),
            ("completeness", completeness),
        ],
    )?
    .unbind())
}
