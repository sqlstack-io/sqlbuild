//! The declarations a model sees: `_VisibleModelDeclarationCache.for_model`, natively.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

use pyo3::FromPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};

use crate::input_loops::_helpers::model::model_loop::LoopContext;
use crate::input_loops::_helpers::model::model_loop::VisibleDeclarations;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::render::types::MacroScopes;
use sqlbuild_python_front::scopes::models::NativeDeclarationContexts;

const NAME_FIELD: &str = "name";
const NATIVE_CONTEXTS_ATTRIBUTE: &str = "native_contexts";
const ENUMS_FIELD: &str = "enums";
const CONSTANTS_FIELD: &str = "constants";
const ENUM_VISIBILITY_FIELD: &str = "enum_visibility";
const CONSTANT_VISIBILITY_FIELD: &str = "constant_visibility";
/// `_VisibleModelDeclarations` fields taken unchanged from the resolved context.
const SHARED_FIELDS: [&str; 5] = [
    "inaccessible_enums",
    "inaccessible_constants",
    "macros",
    "macro_records",
    "inaccessible_macros",
];
/// Every `_VisibleModelDeclarations` field, in declaration order.
const VISIBLE_FIELDS: [&str; 11] = [
    "local_enums",
    "local_constants",
    ENUMS_FIELD,
    CONSTANTS_FIELD,
    "inaccessible_enums",
    "inaccessible_constants",
    ENUM_VISIBILITY_FIELD,
    CONSTANT_VISIBILITY_FIELD,
    "macros",
    "macro_records",
    "inaccessible_macros",
];

/// The scope resolver, project declarations and Python classes model visibility reads.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct VisibilityClasses<'py> {
    /// The `DeclarationScopeResolver`, or `None`.
    pub(crate) resolver: Bound<'py, PyAny>,
    /// Whether the resolver indexes declarations, so contexts are projected from it.
    pub(crate) projects: bool,
    /// Models whose private declarations or grants make their context their own.
    resource_specific: Bound<'py, PyAny>,
    pub(crate) public_enums: Bound<'py, PyDict>,
    pub(crate) public_constants: Bound<'py, PyDict>,
    visible_declarations: Bound<'py, PyAny>,
    pub(crate) resource_identity: Bound<'py, PyAny>,
    declaration_identity: Bound<'py, PyAny>,
    model_kind: Bound<'py, PyAny>,
    enum_kind: Bound<'py, PyAny>,
    constant_kind: Bound<'py, PyAny>,
    private_owner: Bound<'py, PyAny>,
    visibility_record: Bound<'py, PyAny>,
}

/// The model whose declarations are resolved: name, file path, and its own enums and constants.
pub(crate) struct VisibilityModel<'a, 'py> {
    pub(crate) name: &'a str,
    pub(crate) file_path: &'a str,
    pub(crate) local: (&'a Bound<'py, PyAny>, &'a Bound<'py, PyAny>),
}

/// The per-folder declarations of one compile and the contexts projected for them.
#[derive(Default)]
pub(crate) struct ModelVisibility<'py> {
    by_parent: RefCell<HashMap<String, Bound<'py, PyAny>>>,
}

impl<'py> ModelVisibility<'py> {
    /// `for_model`: the model's visible declarations and the identity its usages record.
    pub(crate) fn for_model(
        &self,
        context: &LoopContext<'_, 'py>,
        model: &VisibilityModel<'_, 'py>,
    ) -> PyResult<VisibleDeclarations<'py>> {
        let classes: &VisibilityClasses<'py> = &context.classes.visibility;
        let consumer: Bound<'py, PyAny> = classes
            .resource_identity
            .call1((&classes.model_kind, model.name))?;
        let parent: String = Path::new(model.file_path)
            .parent()
            .map(|parent| parent.to_string_lossy().into_owned())
            .unwrap_or_default();
        let reusable: bool = !classes.resolver.is_none();
        let declarations: Bound<'py, PyAny> = if classes.resource_specific.contains(&consumer)? {
            self.built(context, model, &consumer)?
        } else if let Some(cached) = self.by_parent.borrow().get(&parent).filter(|_| reusable) {
            rebound(context, cached, &consumer)?
        } else {
            let built: Bound<'py, PyAny> = self.built(context, model, &consumer)?;
            if reusable {
                self.by_parent.borrow_mut().insert(parent, built.clone());
            }
            built
        };
        Ok(VisibleDeclarations {
            declarations,
            consumer,
        })
    }

    /// `_build_visible_declaration_indexes`.
    fn built(
        &self,
        context: &LoopContext<'_, 'py>,
        model: &VisibilityModel<'_, 'py>,
        consumer: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let classes: &VisibilityClasses<'py> = &context.classes.visibility;
        let py: Python<'py> = consumer.py();
        let local_enums: Bound<'py, PyDict> = by_name(model.local.0)?;
        let local_constants: Bound<'py, PyDict> = by_name(model.local.1)?;
        let fields: Bound<'py, PyDict> = PyDict::new(py);
        fields.set_item("local_enums", &local_enums)?;
        fields.set_item("local_constants", &local_constants)?;
        let (enums, constants) = if classes.projects {
            let resolved: Bound<'py, PyAny> = self.resolved(context, model.file_path)?;
            for field in SHARED_FIELDS {
                fields.set_item(field, native_field(&resolved, field)?)?;
            }
            fields.set_item(
                ENUM_VISIBILITY_FIELD,
                native_field(&resolved, ENUM_VISIBILITY_FIELD)?,
            )?;
            fields.set_item(
                CONSTANT_VISIBILITY_FIELD,
                native_field(&resolved, CONSTANT_VISIBILITY_FIELD)?,
            )?;
            (
                native_field(&resolved, ENUMS_FIELD)?.cast_into::<PyDict>()?,
                native_field(&resolved, CONSTANTS_FIELD)?.cast_into::<PyDict>()?,
            )
        } else {
            for field in [
                SHARED_FIELDS[0],
                SHARED_FIELDS[1],
                ENUM_VISIBILITY_FIELD,
                CONSTANT_VISIBILITY_FIELD,
                SHARED_FIELDS[2],
                SHARED_FIELDS[3],
                SHARED_FIELDS[4],
            ] {
                fields.set_item(field, PyDict::new(py))?;
            }
            (
                classes.public_enums.clone(),
                classes.public_constants.clone(),
            )
        };
        fields.set_item(ENUMS_FIELD, merged(&enums, &local_enums)?)?;
        fields.set_item(CONSTANTS_FIELD, merged(&constants, &local_constants)?)?;
        for (field, kind, local) in [
            (ENUM_VISIBILITY_FIELD, &classes.enum_kind, &local_enums),
            (
                CONSTANT_VISIBILITY_FIELD,
                &classes.constant_kind,
                &local_constants,
            ),
        ] {
            let inherited: Bound<'py, PyDict> = fields.get_item(field)?.map_or_else(
                || Ok(PyDict::new(py)),
                pyo3::Bound::<pyo3::PyAny>::cast_into::<PyDict>,
            )?;
            let visibility: Bound<'py, PyDict> = inherited.copy()?;
            for name in local.keys() {
                let record: Bound<'py, PyAny> = private_record(context, (kind, &name), consumer)?;
                visibility.set_item(name, PyTuple::new(py, [record])?)?;
            }
            fields.set_item(field, visibility)?;
        }
        classes.visible_declarations.call((), Some(&fields))
    }

    /// `resolve_declaration_context` for an authored file.
    pub(crate) fn resolved(
        &self,
        context: &LoopContext<'_, 'py>,
        file_path: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let resolver: &Bound<'py, PyAny> = &context.classes.visibility.resolver;
        let contexts: Bound<'py, NativeDeclarationContexts> =
            native_field(resolver, "native_contexts")?.cast_into()?;
        let (resolved, _) = contexts.get().resolved_context(resolver, file_path, None)?;
        Ok(resolved)
    }
}

/// `_rebind_visible_declarations`: the folder's declarations with visibility rebound.
fn rebound<'py>(
    context: &LoopContext<'_, 'py>,
    cached: &Bound<'py, PyAny>,
    consumer: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = cached.py();
    let enum_visibility: Bound<'py, PyDict> =
        native_field(cached, ENUM_VISIBILITY_FIELD)?.cast_into()?;
    let constant_visibility: Bound<'py, PyDict> =
        native_field(cached, CONSTANT_VISIBILITY_FIELD)?.cast_into()?;
    if enum_visibility.is_empty() && constant_visibility.is_empty() {
        return Ok(cached.clone());
    }
    let resolver: &Bound<'py, PyAny> = &context.classes.visibility.resolver;
    let contexts: Bound<'py, NativeDeclarationContexts> =
        native_field(resolver, "native_contexts")?.cast_into()?;
    let fields: Bound<'py, PyDict> = PyDict::new(py);
    for field in VISIBLE_FIELDS {
        fields.set_item(field, native_field(cached, field)?)?;
    }
    fields.set_item(
        ENUM_VISIBILITY_FIELD,
        contexts
            .get()
            .rebound_visibility(&enum_visibility, consumer)?,
    )?;
    fields.set_item(
        CONSTANT_VISIBILITY_FIELD,
        contexts
            .get()
            .rebound_visibility(&constant_visibility, consumer)?,
    )?;
    context
        .classes
        .visibility
        .visible_declarations
        .call((), Some(&fields))
}

/// A model-private declaration's `VisibilityRecord`, visible to its owner.
fn private_record<'py>(
    context: &LoopContext<'_, 'py>,
    declaration: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    consumer: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let classes: &VisibilityClasses<'py> = &context.classes.visibility;
    let (kind, name) = declaration;
    let identity: Bound<'py, PyAny> = classes.declaration_identity.call1((kind, name, consumer))?;
    let fields: Bound<'py, PyDict> = PyDict::new(consumer.py());
    fields.set_item("reason", &classes.private_owner)?;
    classes
        .visibility_record
        .call((consumer, identity), Some(&fields))
}

/// Declarations by name, later ones replacing earlier ones.
fn by_name<'py>(declarations: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let named: Bound<'py, PyDict> = PyDict::new(declarations.py());
    for declaration in declarations.try_iter()? {
        let declaration: Bound<'py, PyAny> = declaration?;
        named.set_item(native_field(&declaration, NAME_FIELD)?, declaration)?;
    }
    Ok(named)
}

/// `base | overlay`.
fn merged<'py>(
    base: &Bound<'py, PyDict>,
    overlay: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyDict>> {
    let merged: Bound<'py, PyDict> = base.copy()?;
    merged.update(overlay.as_mapping())?;
    Ok(merged)
}

impl<'py> MacroScopes<'py> for VisibilityClasses<'py> {
    fn file_context(&self, path: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        if !self.projects {
            return Ok(None);
        }
        let contexts: Bound<'py, NativeDeclarationContexts> =
            native_field(&self.resolver, NATIVE_CONTEXTS_ATTRIBUTE)?.cast_into()?;
        let (resolved, _) = contexts
            .get()
            .resolved_context(&self.resolver, path, None)?;
        Ok(Some(resolved))
    }
}
