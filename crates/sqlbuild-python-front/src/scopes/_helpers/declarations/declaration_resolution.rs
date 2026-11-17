//! `resolve_declaration_context`: a path's or resource's declaration context, cached per folder.

use std::path::Path;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use pyo3::{FromPyObject, intern};
use sqlbuild_scopes::scope_index::main::normalize_scope_path::normalize_scope_path;
use sqlbuild_scopes::scope_index::main::parse_identity::parse_identity;
use sqlbuild_scopes::scope_index::main::posix_parent_text::posix_parent_text;
use sqlbuild_scopes::scope_index::main::posix_path_text::posix_path_text;
use sqlbuild_scopes::scope_index::models::{
    DeclarationIdentity, IdentityFailure, QualifiedIdentity, ResourceIdentity, ScopePathFailure,
};

use crate::scopes::models::NativeDeclarationContexts;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const PATH_CACHE_PREFIX: &str = "<path>";
const IDENTITY_SEPARATOR: char = ':';
const POSIX_SEPARATOR: char = '/';
const CURRENT_FOLDER: &str = ".";
const DECLARATION_STEM: &str = "__declaration__.sql";
const PATH_RESOURCE_PREFIX: &str = "<path:";
const PATH_RESOURCE_SUFFIX: &str = ">";
const MODEL_KIND: &str = "model";

/// The lookup mappings and classes resolution reads beyond the classifier's own.
#[derive(Debug, FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct ResolutionInputs {
    resources_by_path: Py<PyAny>,
    resources: Py<PyAny>,
    declarations: Py<PyAny>,
    index_declarations: Py<PyAny>,
    resource_record: Py<PyAny>,
    ownership_root: Py<PyAny>,
    resource_identity: Py<PyAny>,
    declaration_identity: Py<PyAny>,
    resource_kind: Py<PyAny>,
    declaration_kind: Py<PyAny>,
    private_scope: Py<PyAny>,
    identity_error: Py<PyAny>,
    path_error: Py<PyAny>,
}

/// The resources a context is classified for and the consumer it is recorded for.
type Matches<'py> = (Vec<Bound<'py, PyAny>>, Bound<'py, PyAny>);

impl NativeDeclarationContexts {
    /// The context of `file_path` or `resource`, and whether it was projected rather than cached.
    pub fn resolved_context<'py>(
        &self,
        resolver: &Bound<'py, PyAny>,
        file_path: &str,
        resource: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<(Bound<'py, PyAny>, bool)> {
        let py: Python<'py> = resolver.py();
        let inputs: &ResolutionInputs = &self.resolution;
        let target: String = target_path(resolver, file_path)?;
        let empty: Bound<'py, PyTuple> = PyTuple::empty(py);
        let resources: Vec<Bound<'py, PyAny>> = match resource {
            Some(resource) => inputs
                .resources
                .bind(py)
                .call_method1(intern!(py, "get"), (resource, &empty))?,
            None => inputs
                .resources_by_path
                .bind(py)
                .call_method1(intern!(py, "get"), (&target, &empty))?,
        }
        .try_iter()?
        .collect::<PyResult<_>>()?;
        let cache: Bound<'py, PyDict> =
            native_field(resolver, "contexts_by_directory")?.downcast_into()?;
        let specific: Bound<'py, PyAny> = native_field(resolver, "resource_specific")?;
        let mut cache_key: Option<Bound<'py, PyTuple>> = None;
        if let [record] = resources.as_slice()
            && !specific.is_none()
            && !specific.contains(native_field(record, "identity")?)?
        {
            let path: String = native_field(record, "path")?.extract()?;
            let root: Bound<'py, PyAny> =
                native_field(record, "ownership_root")?.getattr(intern!(py, "path"))?;
            let key: Bound<'py, PyTuple> = PyTuple::new(
                py,
                [
                    PyString::new(py, &posix_parent_text(&path)).into_any(),
                    root,
                ],
            )?;
            if let Some(cached) = cache.get_item(&key)? {
                let consumer: Bound<'py, PyAny> = native_field(record, "identity")?;
                return Ok((self.rebound_context(&cached, &consumer)?, false));
            }
            cache_key = Some(key);
        } else if resources.is_empty() && resource.is_none() {
            let key: Bound<'py, PyTuple> = PyTuple::new(py, [PATH_CACHE_PREFIX, target.as_str()])?;
            if let Some(cached) = cache.get_item(&key)? {
                return Ok((cached, false));
            }
            cache_key = Some(key);
        }
        let parses_identity: bool = resource.is_none() && target.contains(IDENTITY_SEPARATOR);
        let (matches, consumer): Matches<'py> = if resources.is_empty() || parses_identity {
            self.queried_matches(py, &target, resource)?
        } else {
            let consumer: Bound<'py, PyAny> = match resource {
                Some(resource) => resource.clone(),
                None => native_field(&resources[0], "identity")?,
            };
            (resources, consumer)
        };
        let context: Bound<'py, PyAny> = self.classified_context(py, matches, consumer)?;
        if let Some(key) = cache_key {
            cache.set_item(key, &context)?;
        }
        Ok((context, true))
    }

    /// `_queried_matches`: the resources a scope query matches, or the path's lexical record.
    fn queried_matches<'py>(
        &self,
        py: Python<'py>,
        target: &str,
        resource: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Matches<'py>> {
        let inputs: &ResolutionInputs = &self.resolution;
        let empty: Bound<'py, PyTuple> = PyTuple::empty(py);
        let lookup =
            |mapping: &Py<PyAny>, key: &Bound<'py, PyAny>| -> PyResult<Vec<Bound<'py, PyAny>>> {
                mapping
                    .bind(py)
                    .call_method1(intern!(py, "get"), (key, &empty))?
                    .try_iter()?
                    .collect()
            };
        let (matches, declaration_matches): (Vec<Bound<'py, PyAny>>, usize) = match resource {
            Some(resource) => (lookup(&inputs.resources, resource)?, 0),
            None if target.contains(IDENTITY_SEPARATOR) => match self.identity(py, target)? {
                QualifiedIdentity::Resource(identity) => (
                    lookup(&inputs.resources, &self.resource_object(py, &identity)?)?,
                    0,
                ),
                QualifiedIdentity::Declaration(identity) => {
                    let declaration: Bound<'py, PyAny> = self.declaration_object(py, &identity)?;
                    (
                        Vec::new(),
                        lookup(&inputs.declarations, &declaration)?.len(),
                    )
                }
            },
            None => {
                let normalized: String = normalize_scope_path(target)
                    .map_err(|failure| self.path_error(py, failure, target))?;
                let key: Bound<'py, PyAny> = PyString::new(py, &normalized).into_any();
                (lookup(&inputs.resources_by_path, &key)?, 0)
            }
        };
        if !matches.is_empty() || declaration_matches > 0 {
            let consumer: Bound<'py, PyAny> = match (resource, matches.first()) {
                (Some(resource), _) => resource.clone(),
                (None, Some(first)) => native_field(first, "identity")?,
                (None, None) => py.None().into_bound(py),
            };
            return Ok((matches, consumer));
        }
        let lexical: String = self.lexical_path(py, target)?;
        let identity: Bound<'py, PyAny> = inputs.resource_identity.bind(py).call1((
            inputs.resource_kind.bind(py).call1((MODEL_KIND,))?,
            format!("{PATH_RESOURCE_PREFIX}{lexical}{PATH_RESOURCE_SUFFIX}"),
        ))?;
        let root: Bound<'py, PyAny> = inputs.ownership_root.bind(py).call1((CURRENT_FOLDER,))?;
        let record: Bound<'py, PyAny> = inputs
            .resource_record
            .bind(py)
            .call1((identity, lexical, root))?;
        Ok((vec![record], py.None().into_bound(py)))
    }

    /// The lexical path of the first public declaration defined at `target`, or `target`.
    fn lexical_path(&self, py: Python<'_>, target: &str) -> PyResult<String> {
        let inputs: &ResolutionInputs = &self.resolution;
        let private: &Bound<'_, PyAny> = inputs.private_scope.bind(py);
        for record in inputs.index_declarations.bind(py).try_iter()? {
            let record: Bound<'_, PyAny> = record?;
            let path: String = native_field(&record, "path")?.extract()?;
            if path != target || native_field(&record, "scope")?.is(private) {
                continue;
            }
            let owning_path: Option<String> = native_field(&record, "owning_path")?.extract()?;
            let owner: String = owning_path
                .filter(|path| !path.is_empty())
                .unwrap_or_else(|| CURRENT_FOLDER.to_owned());
            return Ok(posix_path_text(&format!(
                "{owner}{POSIX_SEPARATOR}{DECLARATION_STEM}"
            )));
        }
        Ok(target.to_owned())
    }

    fn identity(&self, py: Python<'_>, value: &str) -> PyResult<QualifiedIdentity> {
        parse_identity(value).map_err(|failure| {
            let shown = |text: &str| -> String {
                PyString::new(py, text)
                    .repr()
                    .map_or_else(|_| text.to_owned(), |repr| repr.to_string())
            };
            let message: String = match failure {
                IdentityFailure::NoName => {
                    format!("Qualified identity has no name: {}", shown(value))
                }
                IdentityFailure::UnknownKind(kind) => {
                    format!("Unknown qualified identity kind: {}", shown(&kind))
                }
                IdentityFailure::InvalidPrivate => {
                    format!("Invalid private identity: {}", shown(value))
                }
                IdentityFailure::Unqualified => format!(
                    "Expected a kind-qualified identity such as 'model:name': {}",
                    shown(value)
                ),
            };
            raised(self.resolution.identity_error.bind(py), &message)
        })
    }

    fn path_error(&self, py: Python<'_>, failure: ScopePathFailure, path: &str) -> PyErr {
        let message: String = match failure {
            ScopePathFailure::NotProjectRelative => {
                format!("Scope path must be project-relative: {path}")
            }
            ScopePathFailure::EscapesProject => format!("Scope path escapes the project: {path}"),
        };
        raised(self.resolution.path_error.bind(py), &message)
    }

    fn resource_object<'py>(
        &self,
        py: Python<'py>,
        identity: &ResourceIdentity,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inputs: &ResolutionInputs = &self.resolution;
        inputs.resource_identity.bind(py).call1((
            inputs
                .resource_kind
                .bind(py)
                .call1((identity.kind.as_str(),))?,
            identity.name.as_str(),
        ))
    }

    fn declaration_object<'py>(
        &self,
        py: Python<'py>,
        identity: &DeclarationIdentity,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inputs: &ResolutionInputs = &self.resolution;
        let owner: Bound<'py, PyAny> = match &identity.owner {
            Some(owner) => self.resource_object(py, owner)?,
            None => py.None().into_bound(py),
        };
        inputs.declaration_identity.bind(py).call1((
            inputs
                .declaration_kind
                .bind(py)
                .call1((identity.kind.as_str(),))?,
            identity.name.as_str(),
            owner,
        ))
    }

    /// `_rebind_declaration_context`: fresh mappings, visibility rebound to `consumer`.
    pub fn rebound_context<'py>(
        &self,
        context: &Bound<'py, PyAny>,
        consumer: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = context.py();
        let fields: Bound<'py, PyDict> = PyDict::new(py);
        for field in COPIED_FIELDS {
            let mapping: Bound<'py, PyDict> = native_field(context, field)?.downcast_into()?;
            fields.set_item(field, mapping.copy()?)?;
        }
        for field in VISIBILITY_FIELDS {
            let mapping: Bound<'py, PyDict> = native_field(context, field)?.downcast_into()?;
            fields.set_item(field, self.rebound_visibility(&mapping, consumer)?)?;
        }
        fields.set_item(intern!(py, "consumer"), consumer)?;
        self.classes
            .resolution_context
            .bind(py)
            .call((), Some(&fields))
    }

    /// `_rebind_visibility`: each record with `consumer` as its resource.
    pub fn rebound_visibility<'py>(
        &self,
        records_by_name: &Bound<'py, PyDict>,
        consumer: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyDict>> {
        let py: Python<'py> = records_by_name.py();
        let rebound: Bound<'py, PyDict> = PyDict::new(py);
        for (name, records) in records_by_name.iter() {
            let mut items: Vec<Bound<'py, PyAny>> = Vec::new();
            for record in records.try_iter()? {
                let record: Bound<'py, PyAny> = record?;
                items.push(self.visibility_record_for(
                    consumer,
                    &native_field(&record, "declaration")?,
                    &native_field(&record, "reason")?,
                    &native_field(&record, "through")?,
                )?);
            }
            rebound.set_item(name, PyTuple::new(py, items)?)?;
        }
        Ok(rebound)
    }
}

/// Mapping fields `_rebind_declaration_context` copies unchanged.
const COPIED_FIELDS: [&str; 7] = [
    "enums",
    "constants",
    "macros",
    "macro_records",
    "inaccessible_enums",
    "inaccessible_constants",
    "inaccessible_macros",
];
/// Visibility fields `_rebind_declaration_context` rebinds to the consumer.
const VISIBILITY_FIELDS: [&str; 3] = ["enum_visibility", "constant_visibility", "macro_visibility"];

/// `file_path` relative to the resolver's project directory when it is absolute under it.
fn target_path(resolver: &Bound<'_, PyAny>, file_path: &str) -> PyResult<String> {
    let project_dir: Bound<'_, PyAny> = resolver.getattr(intern!(resolver.py(), "project_dir"))?;
    let path: &Path = Path::new(file_path);
    if project_dir.is_none() || !path.is_absolute() {
        return Ok(file_path.to_owned());
    }
    let project_dir: String = project_dir.str()?.to_string();
    Ok(match path.strip_prefix(&project_dir) {
        Ok(relative) if relative.as_os_str().is_empty() => CURRENT_FOLDER.to_owned(),
        Ok(relative) => relative.to_string_lossy().into_owned(),
        Err(_) => file_path.to_owned(),
    })
}

fn raised(class: &Bound<'_, PyAny>, message: &str) -> PyErr {
    match class.call1((message,)) {
        Ok(instance) => PyErr::from_value(instance),
        Err(error) => error,
    }
}
