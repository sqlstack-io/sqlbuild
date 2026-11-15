//! `MacroBridge.call_class` and the class ids its memo keys recorded calls by.

use std::collections::HashMap;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};

use sqlbuild_render::macro_calls::main::call_class_store_text::call_class_store_text;

use crate::bindings::_helpers::macro_bridge::store_keys::{context_store_token, macro_store_token};
use crate::bindings::_helpers::render::macro_expansion_facts::ResourceRef;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;

/// `(kind, name, owner (kind, name))` of a `DeclarationIdentity`, compared by value.
type IdentityKey = (String, String, Option<(String, String)>);
/// `(name, id(loaded macro), identity)`.
type NameKey = (String, usize, Option<IdentityKey>);
/// `(constants, enums)` by name and value id.
type DeclarationValues = (Vec<(String, usize)>, Vec<(String, usize)>);
/// `(id(macro context), declaration values)`.
type ContextKey = (usize, Option<DeclarationValues>);
/// A name's class token and its store token, or `None` when the name is not a visible macro.
type NameToken = Option<(NameKey, Option<String>)>;
/// A call class with the prior relations it was looked up with.
type ClassKey = ((Vec<NameKey>, Option<ContextKey>), Option<Vec<ResourceRef>>);

/// What a call's result may depend on besides its text.
pub(crate) struct CallClass {
    key: (Vec<NameKey>, Option<ContextKey>),
    macro_store_tokens: Vec<String>,
    context_store_token: Option<String>,
    persistent: bool,
}

/// The call classes, class ids and tokens one compile's memo has seen.
#[derive(Debug, Default)]
pub(crate) struct CallClassState {
    class_ids: HashMap<ClassKey, u64>,
    name_tokens: HashMap<(usize, String), NameToken>,
    context_tokens: HashMap<(usize, usize), (ContextKey, Option<String>)>,
    /// Owners and contexts kept alive, so the ids in keys are never reused.
    retained: HashMap<usize, Py<PyAny>>,
    /// Whether a persistent store is attached, so store tokens are computed.
    pub(crate) store_tokens: bool,
}

impl CallClassState {
    /// Forget tokens computed before a store was attached; keep class ids.
    pub(crate) fn reset_tokens(&mut self, store_tokens: bool) {
        self.store_tokens = store_tokens;
        self.name_tokens.clear();
        self.context_tokens.clear();
    }
}

/// The macros one expansion resolves names from.
pub(crate) struct ClassInputs<'a, 'py> {
    /// The `DeclarationResolutionContext`, or `None`.
    pub(crate) declarations: &'a Bound<'py, PyAny>,
    pub(crate) loaded_macros: &'a Bound<'py, PyDict>,
    pub(crate) macro_context: &'a Bound<'py, PyAny>,
}

impl CallClassState {
    /// `call_class`: `None` when a name in the call is not a visible macro.
    pub(crate) fn call_class(
        &mut self,
        tree_names: &[String],
        inputs: &ClassInputs<'_, '_>,
    ) -> PyResult<Option<CallClass>> {
        let scoped: bool = !inputs.declarations.is_none();
        let owner: &Bound<'_, PyAny> = if scoped {
            inputs.declarations
        } else {
            inputs.loaded_macros.as_any()
        };
        let macros: Bound<'_, PyAny> = if scoped {
            native_field(inputs.declarations, "macros")?
        } else {
            inputs.loaded_macros.clone().into_any()
        };
        let _ = self
            .retained
            .entry(owner.as_ptr() as usize)
            .or_insert_with(|| owner.clone().unbind());
        let _ = self
            .retained
            .entry(inputs.macro_context.as_ptr() as usize)
            .or_insert_with(|| inputs.macro_context.clone().unbind());
        let mut tokens: Vec<NameKey> = Vec::with_capacity(tree_names.len());
        let mut store_tokens: Vec<Option<String>> = Vec::with_capacity(tree_names.len());
        for name in tree_names {
            let Some((token, store_token)) = self.name_token((owner, &macros), inputs, name)?
            else {
                return Ok(None);
            };
            tokens.push(token);
            store_tokens.push(store_token);
        }
        let mut injects_context: bool = false;
        for name in tree_names {
            if native_field(&macros.get_item(name)?, "injects_context")?.is_truthy()? {
                injects_context = true;
                break;
            }
        }
        let context: Option<(ContextKey, Option<String>)> = if injects_context {
            Some(self.context_token(owner, inputs)?)
        } else {
            None
        };
        let persistent_tokens: Vec<String> = store_tokens.iter().flatten().cloned().collect();
        let persistent: bool = persistent_tokens.len() == store_tokens.len()
            && context.as_ref().is_none_or(|(_, token)| token.is_some());
        let (context_key, context_store_token) = match context {
            Some((key, token)) => (Some(key), token),
            None => (None, None),
        };
        Ok(Some(CallClass {
            key: (tokens, context_key),
            macro_store_tokens: persistent_tokens,
            context_store_token,
            persistent,
        }))
    }

    /// The class id of `call_class` with these prior relations; a new persistent class is stored.
    pub(crate) fn class_id(
        &mut self,
        call_class: &CallClass,
        prior_relations: Option<&[ResourceRef]>,
    ) -> (u64, Option<String>) {
        let key: ClassKey = (call_class.key.clone(), prior_relations.map(<[_]>::to_vec));
        if let Some(found) = self.class_ids.get(&key) {
            return (*found, None);
        }
        let id: u64 = self.class_ids.len() as u64;
        let _ = self.class_ids.insert(key, id);
        let class_text: Option<String> = (self.store_tokens && call_class.persistent).then(|| {
            call_class_store_text(
                &call_class.macro_store_tokens,
                call_class.context_store_token.as_deref(),
                prior_relations,
            )
        });
        (id, class_text)
    }

    fn name_token(
        &mut self,
        scope: (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
        inputs: &ClassInputs<'_, '_>,
        name: &str,
    ) -> PyResult<NameToken> {
        let (owner, macros) = scope;
        let key: (usize, String) = (owner.as_ptr() as usize, name.to_owned());
        if let Some(found) = self.name_tokens.get(&key) {
            return Ok(found.clone());
        }
        let py: Python<'_> = owner.py();
        let loaded: Bound<'_, PyAny> = mapping_get(macros, name)?;
        let declarations: &Bound<'_, PyAny> = inputs.declarations;
        let visible: bool = !loaded.is_none()
            && (declarations.is_none()
                || !native_field(declarations, "inaccessible_macros")?.contains(name)?);
        let token: NameToken = if visible {
            let identity: Bound<'_, PyAny> = if declarations.is_none() {
                py.None().into_bound(py)
            } else {
                native_field(declarations, "macro_records")?
                    .get_item(name)?
                    .getattr("identity")?
            };
            let store_token: Option<String> = if self.store_tokens {
                let arguments: Bound<'_, PyDict> = PyDict::new(py);
                arguments.set_item("loaded", &loaded)?;
                arguments.set_item("identity", &identity)?;
                macro_store_token(py, Some(arguments))?
            } else {
                None
            };
            Some((
                (
                    name.to_owned(),
                    loaded.as_ptr() as usize,
                    identity_key(&identity)?,
                ),
                store_token,
            ))
        } else {
            None
        };
        let _ = self.name_tokens.insert(key, token.clone());
        Ok(token)
    }

    fn context_token(
        &mut self,
        owner: &Bound<'_, PyAny>,
        inputs: &ClassInputs<'_, '_>,
    ) -> PyResult<(ContextKey, Option<String>)> {
        let key: (usize, usize) = (
            owner.as_ptr() as usize,
            inputs.macro_context.as_ptr() as usize,
        );
        if let Some(found) = self.context_tokens.get(&key) {
            return Ok(found.clone());
        }
        let declarations: &Bound<'_, PyAny> = inputs.declarations;
        let values: Option<DeclarationValues> = if declarations.is_none() {
            None
        } else {
            Some((
                value_ids(&native_field(declarations, "constants")?.downcast_into()?)?,
                value_ids(&native_field(declarations, "enums")?.downcast_into()?)?,
            ))
        };
        let store_token: Option<String> = if self.store_tokens {
            let arguments: Bound<'_, PyDict> = PyDict::new(owner.py());
            arguments.set_item("macro_context", inputs.macro_context)?;
            arguments.set_item("declarations", declarations)?;
            context_store_token(owner.py(), Some(arguments))?
        } else {
            None
        };
        let token: (ContextKey, Option<String>) = ((key.1, values), store_token);
        let _ = self.context_tokens.insert(key, token.clone());
        Ok(token)
    }
}

/// Each `(name, id(value))` of a declaration mapping, in insertion order.
fn value_ids(mapping: &Bound<'_, PyDict>) -> PyResult<Vec<(String, usize)>> {
    mapping
        .iter()
        .map(|(name, value)| Ok((name.extract()?, value.as_ptr() as usize)))
        .collect()
}

fn identity_key(identity: &Bound<'_, PyAny>) -> PyResult<Option<IdentityKey>> {
    if identity.is_none() {
        return Ok(None);
    }
    let owner: Bound<'_, PyAny> = native_field(identity, "owner")?;
    let owner_key: Option<(String, String)> = if owner.is_none() {
        None
    } else {
        Some((
            native_field(&native_field(&owner, "kind")?, "value")?.extract()?,
            native_field(&owner, "name")?.extract()?,
        ))
    };
    Ok(Some((
        native_field(&native_field(identity, "kind")?, "value")?.extract()?,
        native_field(identity, "name")?.extract()?,
        owner_key,
    )))
}
