//! Declaration scope resolvers, per-path declaration contexts and adapter-rendered constants.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyList, PyString, PyTuple};
use pyo3::{IntoPyObject, pyfunction};

use crate::scopes::models::NativeDeclarationContexts;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_base::records::main::fields::mapping_values::mapping_values;
use sqlbuild_python_base::records::main::registry::native_attr::native_attr;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::NativeAttr;
use sqlbuild_python_base::records::types::RecordId;

const NO_DECLARATION_KIND: u8 = 255;
/// Declaration kinds by their native code: enum 0, constant 1, macro 2.
const DECLARATION_KINDS: [&str; 3] = ["ENUM", "CONSTANT", "MACRO"];
pub(crate) const TYPES_MODULE: &str = "types";
const SCALAR_KINDS: [&str; 6] = ["string", "integer", "boolean", "float", "decimal", "null"];
const COLLECTION_KINDS: [&str; 2] = ["list", "set"];
const ARRAY_RENDERING: &str = "array";
const MAX_SQL_VALUE_SIZE: usize = 1_000_000;

/// `(kind, name, owner)` identity of one declaration, built as `DeclarationIdentity`.
fn declaration_identity<'py>(
    classes: &(Bound<'py, PyAny>, Bound<'py, PyAny>),
    kind: &str,
    name: &Bound<'py, PyAny>,
    owner: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let (identity_class, kinds) = classes;
    let kind: Bound<'py, PyAny> = native_field(kinds, kind)?;
    match owner {
        Some(owner) => identity_class.call1((kind, name, owner)),
        None => identity_class.call1((kind, name)),
    }
}

/// Each project declaration's identity mapped to its process-local value.
pub(crate) fn runtime_declarations<'py>(
    discovered: &Bound<'py, PyAny>,
    loaded_macros: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = discovered.py();
    let classes = (
        record_class(py, RecordId::DeclarationIdentity)?,
        record_class(py, RecordId::DeclarationKind)?,
    );
    let declarations: Bound<'py, PyDict> = PyDict::new(py);
    for (files, kind) in [("enum_files", "ENUM"), ("constant_files", "CONSTANT")] {
        for file in native_field(discovered, files)?.try_iter()? {
            for declaration in native_field(&file?, "declarations")?.try_iter()? {
                let declaration: Bound<'py, PyAny> = declaration?;
                let identity = declaration_identity(
                    &classes,
                    kind,
                    &native_field(&declaration, "name")?,
                    None,
                )?;
                declarations.set_item(identity, declaration)?;
            }
        }
    }
    let model_kind: Bound<'py, PyAny> = enum_member(py, RecordId::ResourceKind, "MODEL")?;
    for model_file in native_field(discovered, "model_files")?.try_iter()? {
        let model_file: Bound<'py, PyAny> = model_file?;
        let name: Bound<'py, PyAny> =
            native_field(&model_file, "header_values")?.call_method1("get", ("name",))?;
        let name: Bound<'py, PyAny> = if name.is_instance_of::<PyString>() && name.is_truthy()? {
            name
        } else {
            native_field(&native_field(&model_file, "relative_path")?, "stem")?
        };
        let owner: Bound<'py, PyAny> =
            new_record(py, RecordId::ResourceIdentity, (&model_kind, name), None)?;
        for (attribute, kind) in [
            ("enum_declarations", "ENUM"),
            ("constant_declarations", "CONSTANT"),
        ] {
            for declaration in native_field(&model_file, attribute)?.try_iter()? {
                let declaration: Bound<'py, PyAny> = declaration?;
                let identity = declaration_identity(
                    &classes,
                    kind,
                    &native_field(&declaration, "name")?,
                    Some(&owner),
                )?;
                declarations.set_item(identity, declaration)?;
            }
        }
    }
    if !loaded_macros.is_none() {
        for name_and_macro in mapping_values(loaded_macros)?.try_iter()? {
            let declared_macro: Bound<'py, PyAny> = name_and_macro?;
            let identity = declaration_identity(
                &classes,
                "MACRO",
                &native_field(&declared_macro, "name")?,
                None,
            )?;
            declarations.set_item(identity, declared_macro)?;
        }
    }
    Ok(declarations)
}

/// Resources whose declarations differ from their directory's: private owners and grantees.
pub(crate) fn resource_specific<'py>(
    scope_index: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let py: Python<'py> = scope_index.py();
    let private: Bound<'py, PyAny> = enum_member(py, RecordId::ScopeKind, "PRIVATE")?;
    let mut resources: Vec<Bound<'py, PyAny>> = Vec::new();
    for declaration in native_field(scope_index, "declarations")?.try_iter()? {
        let declaration: Bound<'py, PyAny> = declaration?;
        let owner: Bound<'py, PyAny> =
            native_field(&native_field(&declaration, "identity")?, "owner")?;
        if native_field(&declaration, "scope")?.is(&private) && !owner.is_none() {
            resources.push(owner);
        }
    }
    for grant in native_field(scope_index, "grants")?.try_iter()? {
        resources.push(native_field(&grant?, "resource")?);
    }
    PyFrozenSet::new(py, resources)
}

/// The native code of declaration kind `kind`, or the no-kind code.
fn kind_code(kinds: &[Bound<'_, PyAny>], kind: &Bound<'_, PyAny>) -> u8 {
    for (code, known) in (0_u8..).zip(kinds.iter()) {
        if known.is(kind) {
            return code;
        }
    }
    NO_DECLARATION_KIND
}

/// The native kind code of a runtime declaration value, by its class.
fn value_kind_code(classes: &[Bound<'_, PyAny>], value: &Bound<'_, PyAny>) -> PyResult<u8> {
    for (code, class) in (0_u8..).zip(classes.iter()) {
        if value.is_instance(class)? {
            return Ok(code);
        }
    }
    Ok(NO_DECLARATION_KIND)
}

/// `{owner: list(items)}` of an index position mapping.
fn listed_positions<'py>(positions: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let listed: Bound<'py, PyDict> = PyDict::new(positions.py());
    for item in mapping_items(positions)?.try_iter()? {
        let (owner, items): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        listed.set_item(
            owner,
            PyList::new(
                positions.py(),
                items.try_iter()?.collect::<PyResult<Vec<_>>>()?,
            )?,
        )?;
    }
    Ok(listed)
}

/// Hand the lookup's declaration positions and runtime values to the native classifier.
pub(crate) fn native_declaration_contexts<'py>(
    lookup: &Bound<'py, PyAny>,
    declarations: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = lookup.py();
    let kind_type: Bound<'py, PyAny> = record_class(py, RecordId::DeclarationKind)?;
    let kinds: Vec<Bound<'py, PyAny>> = DECLARATION_KINDS
        .iter()
        .map(|kind| enum_member(py, RecordId::DeclarationKind, kind))
        .collect::<PyResult<_>>()?;
    let value_classes: Vec<Bound<'py, PyAny>> = vec![
        record_class(py, RecordId::EnumDeclaration)?,
        record_class(py, RecordId::ConstantDeclaration)?,
        record_class(py, RecordId::LoadedMacro)?,
    ];
    let index: Bound<'py, PyAny> = native_field(lookup, "visibility_index")?;
    let lookup_declarations: Bound<'py, PyAny> = native_field(lookup, "declarations")?;
    let identity_keys: Bound<'py, PyDict> = PyDict::new(py);
    let mut positions: Vec<Bound<'py, PyTuple>> = Vec::new();
    for identity in native_field(&index, "identities")?.try_iter()? {
        let identity: Bound<'py, PyAny> = identity?;
        let key: Bound<'py, PyAny> =
            identity_keys.call_method1("setdefault", (&identity, identity_keys.len()))?;
        let value: Bound<'py, PyAny> = mapping_get(declarations, &identity)?;
        positions.push(PyTuple::new(
            py,
            [
                identity.clone(),
                lookup_declarations.get_item(&identity)?.get_item(0)?,
                native_field(&identity, "name")?,
                kind_code(&kinds, &native_field(&identity, "kind")?)
                    .into_pyobject(py)?
                    .into_any(),
                value.clone(),
                value_kind_code(&value_classes, &value)?
                    .into_pyobject(py)?
                    .into_any(),
                key,
            ],
        )?);
    }
    let reasons: Bound<'py, PyDict> = PyDict::new(py);
    for reason in record_class(py, RecordId::VisibilityReason)?.try_iter()? {
        let reason: Bound<'py, PyAny> = reason?;
        reasons.set_item(native_field(&reason, "value")?, reason)?;
    }
    let classes: Bound<'py, PyDict> = PyDict::new(py);
    classes.set_item(
        "visibility_record",
        record_class(py, RecordId::VisibilityRecord)?,
    )?;
    classes.set_item(
        "resolution_context",
        record_class(py, RecordId::DeclarationResolutionContext)?,
    )?;
    classes.set_item("reasons", reasons)?;
    let keys: Bound<'py, PyDict> = PyDict::new(py);
    keys.set_item("identity_keys", identity_keys)?;
    keys.set_item(
        "private_positions",
        native_field(&index, "private_positions")?,
    )?;
    keys.set_item(
        "grants_by_resource",
        native_field(lookup, "grants_by_resource")?,
    )?;
    let lookups: Bound<'py, PyDict> = PyDict::new(py);
    lookups.set_item(
        "resources_by_path",
        native_field(lookup, "resources_by_path")?,
    )?;
    lookups.set_item("resources", native_field(lookup, "resources")?)?;
    lookups.set_item("declarations", &lookup_declarations)?;
    lookups.set_item(
        "index_declarations",
        native_field(&native_field(lookup, "index")?, "declarations")?,
    )?;
    for (name, class) in [
        ("resource_record", RecordId::ResourceRecord),
        ("ownership_root", RecordId::OwnershipRoot),
        ("resource_identity", RecordId::ResourceIdentity),
        ("declaration_identity", RecordId::DeclarationIdentity),
    ] {
        lookups.set_item(name, record_class(py, class)?)?;
    }
    lookups.set_item("resource_kind", record_class(py, RecordId::ResourceKind)?)?;
    lookups.set_item("declaration_kind", kind_type)?;
    lookups.set_item(
        "private_scope",
        enum_member(py, RecordId::ScopeKind, "PRIVATE")?,
    )?;
    lookups.set_item(
        "identity_error",
        error_class(py, ErrorId::InvalidQualifiedIdentity)?,
    )?;
    lookups.set_item("path_error", error_class(py, ErrorId::InvalidScopePath)?)?;
    let global_positions: Bound<'py, PyList> = PyList::new(
        py,
        native_field(&index, "global_positions")?
            .try_iter()?
            .collect::<PyResult<Vec<_>>>()?,
    )?;
    let contexts: NativeDeclarationContexts = NativeDeclarationContexts::new(
        PyList::new(py, positions)?.extract()?,
        (
            global_positions.extract()?,
            listed_positions(&native_field(&index, "local_positions")?)?.extract()?,
            listed_positions(&native_field(&index, "inherited_positions")?)?.extract()?,
        ),
        classes.extract()?,
        (keys.extract()?, lookups.extract()?),
    );
    Ok(Bound::new(py, contexts)?.into_any())
}

/// Hand one scope lookup's positions and runtime values to the native classifier.
#[pyfunction]
#[pyo3(signature = (*, lookup, declarations))]
pub(crate) fn build_native_declaration_contexts<'py>(
    lookup: &Bound<'py, PyAny>,
    declarations: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    native_declaration_contexts(lookup, declarations)
}

/// Canonical visibility for one authored path, projected onto runtime declaration values.
pub(crate) fn resolved_declaration_context<'py>(
    resolver: &Bound<'py, PyAny>,
    file_path: &Bound<'py, PyAny>,
    resource: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = resolver.py();
    let resource: Bound<'py, PyAny> =
        resource.map_or_else(|| py.None().into_bound(py), Clone::clone);
    native_field(resolver, "native_contexts")?
        .call_method1("resolve", (resolver, file_path, resource))?
        .get_item(0)
}

/// The adapter's SQL for one constant under `selected` collection rendering, or its error.
pub(crate) fn rendered_constant<'py>(
    declaration: &Bound<'py, PyAny>,
    value_renderer: &Bound<'py, PyAny>,
    selected: &Bound<'py, PyAny>,
) -> PyResult<String> {
    let value: Bound<'py, PyAny> = native_field(declaration, "value")?;
    let kind: String = native_field(&value, "kind")?.str()?.extract()?;
    let method: &str = if SCALAR_KINDS.contains(&kind.as_str()) {
        "render_typed_scalar"
    } else if COLLECTION_KINDS.contains(&kind.as_str()) {
        if selected.eq(ARRAY_RENDERING)? {
            "render_typed_array"
        } else {
            "render_typed_value_list"
        }
    } else {
        "render_typed_object"
    };
    let rendered: String =
        call_with(&native_field(value_renderer, method)?, &[("value", value)])?.extract()?;
    let size: usize = rendered.len();
    if size > MAX_SQL_VALUE_SIZE {
        let context: String = format!(
            "{} constant '{}'",
            native_field(declaration, "relative_path")?.str()?,
            native_field(declaration, "name")?.str()?
        );
        return Err(PyErr::from_value(
            native_attr(declaration.py(), NativeAttr::SqlValueValidationError)?
            .call1((format!(
                "{context} rendered value is {size} bytes; maximum is {MAX_SQL_VALUE_SIZE} bytes"
            ),))?,
        ));
    }
    Ok(rendered)
}

/// Canonical visibility for one authored path, projected onto runtime declaration values.
#[pyfunction]
#[pyo3(signature = (*, resolver, file_path, resource=None))]
pub(crate) fn resolve_declaration_context<'py>(
    resolver: &Bound<'py, PyAny>,
    file_path: &Bound<'py, PyAny>,
    resource: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    resolved_declaration_context(resolver, file_path, resource.as_ref())
}
