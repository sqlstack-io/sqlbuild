//! `named_declaration_usages`: a named declaration's usage, reporting S006 when it is hidden.

use std::path::Path;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_scopes::scope_index::main::path_visible::path_visible;
use sqlbuild_scopes::scope_index::models::{
    DeclaredScope, ResourceIdentity, ResourceKind, ScopeKind,
};

use crate::input_loops::_helpers::model::model_loop::LoopContext;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const NAME_FIELD: &str = "name";
const PATH_FIELD: &str = "path";
const SCOPE_FIELD: &str = "scope";
const VALUE_FIELD: &str = "value";
const IDENTITY_FIELD: &str = "identity";
const OWNER_FIELD: &str = "owner";
const KIND_FIELD: &str = "kind";
const OWNING_PATH_FIELD: &str = "owning_path";
const OWNERSHIP_ROOT_FIELD: &str = "ownership_root";
const CONSUMER_FIELD: &str = "consumer";
const DECLARATION_FIELD: &str = "declaration";
const INACCESSIBLE_CODE: &str = "S006";
const MODEL_RESOURCE: &str = "model";

/// A named declaration kind: its enum value, diagnostic label and declaration role.
pub(crate) struct NamedKind {
    pub(crate) value: &'static str,
    pub(crate) label: &'static str,
    pub(crate) role: &'static str,
}

/// Reusable model schemas.
pub(crate) const SCHEMA_DECLARATION: NamedKind = NamedKind {
    value: "schema",
    label: "Schema",
    role: "schemas",
};
/// Named SQL lifecycle hooks.
pub(crate) const SQL_HOOK_DECLARATION: NamedKind = NamedKind {
    value: "sql_hook",
    label: "SQL hook",
    role: "hooks/sql",
};
/// Python lifecycle hooks.
pub(crate) const PYTHON_HOOK_DECLARATION: NamedKind = NamedKind {
    value: "python_hook",
    label: "Python hook",
    role: "hooks/python",
};

/// Generic audits attached to models, sources and seeds.
pub(crate) const AUDIT_DECLARATION: NamedKind = NamedKind {
    value: "audit",
    label: "Generic audit",
    role: "audits/generic",
};
use sqlbuild_python_front::scopes::constants::CURRENT_FOLDER;
use sqlbuild_scopes::scope_index::main::declared_scope_path_message::declared_scope_path_message;

/// The usage of the indexed declaration `name`, if any, reporting S006 when it is hidden.
pub(crate) fn named_usages<'py>(
    context: &LoopContext<'_, 'py>,
    declaration: (&NamedKind, &str),
    consumer: &Bound<'py, PyAny>,
    model: (&str, &str),
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let (model_name, relative_path) = model;
    resource_named_usages(
        context,
        declaration,
        consumer,
        (MODEL_RESOURCE, model_name, relative_path),
    )
}

/// `named_usages` for a consumer of `(resource type, name, relative path)`.
pub(crate) fn resource_named_usages<'py>(
    context: &LoopContext<'_, 'py>,
    declaration: (&NamedKind, &str),
    consumer: &Bound<'py, PyAny>,
    resource: (&str, &str, &str),
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let (kind, name) = declaration;
    let (resource_type, model_name, relative_path) = resource;
    let py: Python<'py> = consumer.py();
    let key: Bound<'py, PyTuple> = PyTuple::new(py, [kind.value, name])?;
    let Some(record) = context.classes.named_records.get_item(key)? else {
        return Ok(Vec::new());
    };
    let label: String = relative_path.replace(std::path::MAIN_SEPARATOR, "/");
    let declared: DeclaredScope = declared_scope(&record)?;
    let Ok(visible) = path_visible(&declared, &label) else {
        return Err(path_error(context, &declared, &label));
    };
    if !visible {
        report_inaccessible(
            context,
            &record,
            (kind, name),
            (resource_type, model_name, &label),
        )?;
    }
    let fields: Bound<'py, PyDict> = PyDict::new(py);
    fields.set_item(CONSUMER_FIELD, consumer)?;
    fields.set_item(DECLARATION_FIELD, native_field(&record, IDENTITY_FIELD)?)?;
    Ok(vec![context.classes.usage_record.call((), Some(&fields))?])
}

/// `InvalidScopePathError` for the consumer path or owner folder Python normalizes first.
fn path_error(context: &LoopContext<'_, '_>, declared: &DeclaredScope, label: &str) -> PyErr {
    match context
        .classes
        .scope_path_error
        .call1((declared_scope_path_message(declared, label),))
    {
        Ok(instance) => PyErr::from_value(instance),
        Err(failure) => failure,
    }
}

/// The scope facts of a Python `DeclarationRecord`.
fn declared_scope(record: &Bound<'_, PyAny>) -> PyResult<DeclaredScope> {
    let scope: String =
        native_field(&native_field(record, SCOPE_FIELD)?, VALUE_FIELD)?.extract()?;
    let owner: Bound<'_, PyAny> =
        native_field(&native_field(record, IDENTITY_FIELD)?, OWNER_FIELD)?;
    let owner: Option<ResourceIdentity> = if owner.is_none() {
        None
    } else {
        let kind: String =
            native_field(&native_field(&owner, KIND_FIELD)?, VALUE_FIELD)?.extract()?;
        let name: String = native_field(&owner, NAME_FIELD)?.extract()?;
        ResourceKind::parse(&kind).map(|kind| ResourceIdentity { kind, name })
    };
    Ok(DeclaredScope {
        scope: ScopeKind::parse(&scope).unwrap_or(ScopeKind::Global),
        owner,
        owning_path: native_field(record, OWNING_PATH_FIELD)?.extract()?,
    })
}

/// Report Python's S006 diagnostic for a declaration the model's folder cannot see.
fn report_inaccessible(
    context: &LoopContext<'_, '_>,
    record: &Bound<'_, PyAny>,
    declaration: (&NamedKind, &str),
    resource: (&str, &str, &str),
) -> PyResult<()> {
    let py: Python<'_> = record.py();
    let (kind, name) = declaration;
    let (resource_type, model_name, label) = resource;
    let folder: &str = Path::new(label)
        .parent()
        .and_then(Path::to_str)
        .filter(|parent| !parent.is_empty())
        .unwrap_or(CURRENT_FOLDER);
    let owning_path: Bound<'_, PyAny> = native_field(record, OWNING_PATH_FIELD)?;
    let owner: String = if owning_path.is_truthy()? {
        owning_path.str()?.to_string()
    } else {
        native_field(&native_field(record, OWNERSHIP_ROOT_FIELD)?, PATH_FIELD)?
            .str()?
            .to_string()
    };
    let record_path: String = native_field(record, PATH_FIELD)?.str()?.to_string();
    let scope: String =
        native_field(&native_field(record, SCOPE_FIELD)?, VALUE_FIELD)?.extract()?;
    let (label_text, role) = (kind.label, kind.role);
    let fields: Bound<'_, PyDict> = PyDict::new(py);
    fields.set_item("phase", "compile")?;
    fields.set_item("severity", "error")?;
    fields.set_item("code", INACCESSIBLE_CODE)?;
    fields.set_item(
        "message",
        format!(
            "{label_text} '{name}' is not visible from '{label}'. It is defined at {record_path} \
             with {scope} scope owned by '{owner}'"
        ),
    )?;
    fields.set_item("resource_type", resource_type)?;
    fields.set_item("resource_name", model_name)?;
    fields.set_item("path", context.classes.path_class.call1((label,))?)?;
    fields.set_item(
        "help",
        format!(
            "move '{record_path}' to a {role}/ role owned by '{folder}' or one of its parent \
             folders, or to the project-wide {role}/ when consumers span resource trees"
        ),
    )?;
    let diagnostic: Bound<'_, PyAny> = context
        .classes
        .compiler_diagnostic
        .call((), Some(&fields))?;
    let key: Bound<'_, PyTuple> = PyTuple::new(py, [INACCESSIBLE_CODE, label, kind.value, name])?;
    let arguments: Bound<'_, PyDict> = PyDict::new(py);
    arguments.set_item("key", key)?;
    arguments.set_item("diagnostic", diagnostic)?;
    context
        .classes
        .report_diagnostic
        .call((), Some(&arguments))?;
    Ok(())
}
