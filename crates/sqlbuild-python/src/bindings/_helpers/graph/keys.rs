//! `CompiledObjectKey`s for native `(resource type, name)` pairs, converted once per pair.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::bindings::_helpers::records::construct::record_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};

/// One `CompiledObjectKey` per native pair, so shared keys convert once.
pub(crate) struct KeyCache<'py> {
    key_type: Bound<'py, PyAny>,
    resource_type: Bound<'py, PyAny>,
    keys: RefCell<HashMap<(String, String), Py<PyAny>>>,
}

impl<'py> KeyCache<'py> {
    pub(crate) fn new(py: Python<'py>) -> PyResult<Self> {
        Ok(Self {
            key_type: record_class(py, RecordId::CompiledObjectKey)?,
            resource_type: record_class(py, RecordId::CompiledResourceType)?,
            keys: RefCell::new(HashMap::new()),
        })
    }

    pub(crate) fn py(&self) -> Python<'py> {
        self.key_type.py()
    }

    /// The `CompiledObjectKey` for one `(resource type, name)` pair.
    pub(crate) fn key(
        &self,
        (resource_type, name): &(String, String),
    ) -> PyResult<Bound<'py, PyAny>> {
        let pair = (resource_type.clone(), name.clone());
        if let Some(existing) = self.keys.borrow().get(&pair) {
            return Ok(existing.bind(self.py()).clone());
        }
        let created: Bound<'py, PyAny> = self
            .key_type
            .call1((self.resource_type.call1((resource_type,))?, name))?;
        self.keys
            .borrow_mut()
            .insert(pair, created.clone().unbind());
        Ok(created)
    }

    /// `CompiledObjectKey`s for `pairs`, in order.
    pub(crate) fn keys(&self, pairs: &[(String, String)]) -> PyResult<Vec<Bound<'py, PyAny>>> {
        pairs.iter().map(|pair| self.key(pair)).collect()
    }
}

/// The native `(resource type, name)` pair of a `CompiledObjectKey`.
pub(crate) fn native_key(key: &Bound<'_, PyAny>) -> PyResult<(String, String)> {
    Ok((
        native_field(key, "resource_type")?.str()?.extract()?,
        native_field(key, "name")?.extract()?,
    ))
}
