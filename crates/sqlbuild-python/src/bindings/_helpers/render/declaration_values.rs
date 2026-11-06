//! Declaration scope resolvers, per-path declaration contexts and adapter-rendered constants.

use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyInt, PyList, PyString, PyTuple};
use pyo3::{IntoPyObject, pyclass, pyfunction, wrap_pyfunction};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::boundary::python_calls::{call_with, imported};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::construct::{
    enum_member, new_record, new_record_with, record_class,
};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::fields::mapping_items;
use crate::bindings::_helpers::records::fields::mapping_values;
use crate::bindings::_helpers::records::ids::{ErrorId, NativeAttr, RecordId};
use crate::bindings::_helpers::records::registry;
use crate::bindings::_helpers::scopes::declaration_contexts::NativeDeclarationContexts;
use crate::bindings::_helpers::scopes::scope_lookup::build_scope_lookup;

const NO_DECLARATION_KIND: u8 = 255;
/// Declaration kinds by their native code: enum 0, constant 1, macro 2.
const DECLARATION_KINDS: [&str; 3] = ["ENUM", "CONSTANT", "MACRO"];
const TYPES_MODULE: &str = "types";
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
fn runtime_declarations<'py>(
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
fn resource_specific<'py>(scope_index: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyFrozenSet>> {
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

/// Pair the serializable static index with original process-local declaration values.
pub(crate) fn declaration_scope_resolver<'py>(
    discovered: &Bound<'py, PyAny>,
    scope_index: &Bound<'py, PyAny>,
    (loaded_macros, lookup): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered.py();
    let declarations: Bound<'py, PyDict> = runtime_declarations(discovered, loaded_macros)?;
    let lookup: Bound<'py, PyAny> = if lookup.is_none() {
        build_scope_lookup(py, scope_index)?.into_bound(py)
    } else {
        lookup.clone()
    };
    let projection: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::DeclarationRuntimeProjection,
        &[(
            "declarations",
            imported(py, TYPES_MODULE, "MappingProxyType")?.call1((&declarations,))?,
        )],
    )?;
    let native_contexts: Bound<'py, PyAny> =
        native_declaration_contexts(&lookup, declarations.as_any())?;
    new_record_with(
        py,
        RecordId::DeclarationScopeResolver,
        &[
            ("project_dir", native_field(discovered, "project_dir")?),
            ("lookup", lookup),
            ("projection", projection),
            (
                "resource_specific",
                resource_specific(scope_index)?.into_any(),
            ),
            ("native_contexts", native_contexts),
        ],
    )
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
fn build_native_declaration_contexts<'py>(
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
fn rendered_constant<'py>(
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
            registry::native(declaration.py(), NativeAttr::SqlValueValidationError)?
            .call1((format!(
                "{context} rendered value is {size} bytes; maximum is {MAX_SQL_VALUE_SIZE} bytes"
            ),))?,
        ));
    }
    Ok(rendered)
}

/// Render one validated constant with the active adapter's typed-value contract.
pub(crate) fn constant_declaration_sql<'py>(
    declaration: &Bound<'py, PyAny>,
    value_renderer: &Bound<'py, PyAny>,
    (collection_rendering, file_path): (&Bound<'py, PyAny>, Option<&str>),
) -> PyResult<String> {
    let py: Python<'py> = declaration.py();
    let render_as: Bound<'py, PyAny> = native_field(declaration, "render_as")?;
    let selected: Bound<'py, PyAny> = if render_as.is_truthy()? {
        render_as
    } else {
        collection_rendering.clone()
    };
    let error: PyErr = match rendered_constant(declaration, value_renderer, &selected) {
        Ok(rendered) => return Ok(rendered),
        Err(error) => error,
    };
    let rendering_error: Bound<'py, PyAny> = host::entry(py, ExecutorHost::SqlValueRenderingError)?;
    let validation_error: Bound<'py, PyAny> =
        registry::native(py, NativeAttr::SqlValueValidationError)?;
    let raised: Bound<'py, PyAny> = error.value(py).clone().into_any();
    if !(raised.is_instance(&rendering_error)? || raised.is_instance(&validation_error)?) {
        return Err(error);
    }
    let relative_path: Bound<'py, PyAny> = native_field(declaration, "relative_path")?;
    let location: String = match file_path {
        Some(file_path) => file_path.to_owned(),
        None => relative_path.str()?.to_string(),
    };
    let compile_error: Bound<'py, PyAny> =
        error_class(py, ErrorId::CompileInput)?.call1((format!(
            "{} constant '{}' could not be rendered in '{}' by adapter '{}' as {}: {}",
            relative_path.str()?,
            native_field(declaration, "name")?.str()?,
            location,
            native_field(value_renderer, "adapter_name")?.str()?,
            native_field(&selected, "value")?.str()?,
            raised.str()?,
        ),))?;
    compile_error.setattr("__cause__", &raised)?;
    Err(PyErr::from_value(compile_error))
}

/// The adapter's constant rendering a compile's declaration expansions share.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct ConstantRendering {
    value_renderer: Py<PyAny>,
    collection_rendering: Py<PyAny>,
}

impl ConstantRendering {
    /// Rendering with the adapter's value renderer and the project's collection rendering.
    pub(crate) fn new(value_renderer: Py<PyAny>, collection_rendering: Py<PyAny>) -> Self {
        Self {
            value_renderer,
            collection_rendering,
        }
    }

    /// The adapter name the value renderer renders for, empty when it names none.
    pub(crate) fn adapter_name(&self, py: Python<'_>) -> String {
        native_field(self.value_renderer.bind(py), "adapter_name")
            .and_then(|name| name.extract::<String>())
            .unwrap_or_default()
    }

    /// One visible constant as SQL, its failures located at `file_path`.
    pub(crate) fn render(
        &self,
        declaration: &Bound<'_, PyAny>,
        file_path: &str,
    ) -> PyResult<String> {
        let py: Python<'_> = declaration.py();
        constant_declaration_sql(
            declaration,
            self.value_renderer.bind(py),
            (self.collection_rendering.bind(py), Some(file_path)),
        )
    }
}

/// One enum member value as a SQL scalar literal.
#[pyfunction]
#[pyo3(signature = (*, value))]
pub(crate) fn render_enum_member_value(value: &Bound<'_, PyAny>) -> PyResult<String> {
    if value.is_instance_of::<PyInt>() {
        return value.str()?.extract();
    }
    let text: String = value.extract()?;
    Ok(format!("'{}'", text.replace('\'', "''")))
}

/// Canonical visibility for one authored path, projected onto runtime declaration values.
#[pyfunction]
#[pyo3(signature = (*, resolver, file_path, resource=None))]
fn resolve_declaration_context<'py>(
    resolver: &Bound<'py, PyAny>,
    file_path: &Bound<'py, PyAny>,
    resource: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    resolved_declaration_context(resolver, file_path, resource.as_ref())
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_native_declaration_contexts, module)?)?;
    module.add_function(wrap_pyfunction!(render_enum_member_value, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_declaration_context, module)?)?;
    Ok(())
}
