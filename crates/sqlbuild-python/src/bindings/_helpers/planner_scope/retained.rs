//! Native planner scope ownership, with projection only at public record boundaries.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyclass;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyTuple};
use sqlbuild_analysis::assembly::project::types::ObjectKey;

use crate::bindings::_helpers::graph::keys::KeyCache;
use crate::bindings::_helpers::graph::views::edge_dict;
use crate::bindings::_helpers::native_host::model_lookup::ModelHandle;
use crate::bindings::_helpers::planner_scope::values::{edges, keys};
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::{field, mapping_items};
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::records::models::NativeRecord;

/// The graph and selection used throughout planning; model values remain compiled-resource views.
#[derive(Debug)]
pub(crate) struct ScopeData {
    pub(crate) upstream: Vec<(ObjectKey, Vec<ObjectKey>)>,
    pub(crate) downstream: Vec<(ObjectKey, Vec<ObjectKey>)>,
    pub(crate) names: Vec<(String, ObjectKey)>,
    pub(crate) models: HashMap<String, ModelHandle>,
    pub(crate) model_order: Vec<String>,
    pub(crate) selected: HashSet<ObjectKey>,
    pub(crate) order: Vec<ObjectKey>,
    pub(crate) user_selected: HashSet<ObjectKey>,
    pub(crate) python_reads: HashSet<String>,
    pub(crate) tests: Py<NativeRecord>,
    pub(crate) load_sources: bool,
    pub(crate) end_audit_sources: HashSet<ObjectKey>,
}

/// Internal carrier, never returned in place of a supported public PlannerScope record.
#[pyclass(frozen)]
pub(crate) struct RetainedScope {
    pub(crate) data: Arc<ScopeData>,
    projection: PyOnceLock<Py<NativeRecord>>,
}

impl ScopeData {
    pub(crate) fn object(self, py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
        Ok(Bound::new(
            py,
            RetainedScope {
                data: Arc::new(self),
                projection: PyOnceLock::new(),
            },
        )?
        .into_any())
    }

    pub(crate) fn model<'py>(&self, py: Python<'py>, name: &str) -> Option<Bound<'py, PyAny>> {
        self.models.get(name).map(|model| model.bind(py))
    }

    pub(crate) fn with_selected(&self, py: Python<'_>, selected: HashSet<ObjectKey>) -> Self {
        Self {
            upstream: self.upstream.clone(),
            downstream: self.downstream.clone(),
            names: self.names.clone(),
            model_order: self.model_order.clone(),
            models: self
                .models
                .iter()
                .map(|(name, model)| (name.clone(), model.clone_ref(py)))
                .collect(),
            selected,
            order: self.order.clone(),
            user_selected: self.user_selected.clone(),
            python_reads: self.python_reads.clone(),
            tests: self.tests.clone_ref(py),
            load_sources: self.load_sources,
            end_audit_sources: self.end_audit_sources.clone(),
        }
    }

    /// Public field projection; internal graph consumers read the native fields above instead.
    pub(crate) fn field<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        let cache = KeyCache::new(py)?;
        let key_set = |keys: &HashSet<ObjectKey>| -> PyResult<Bound<'py, PyAny>> {
            Ok(
                PyFrozenSet::new(py, cache.keys(&keys.iter().cloned().collect::<Vec<_>>())?)?
                    .into_any(),
            )
        };
        match name {
            "upstream_deps" => Ok(edge_dict(&cache, &self.upstream)?.into_any()),
            "downstream_deps" => Ok(edge_dict(&cache, &self.downstream)?.into_any()),
            "selected_keys" => key_set(&self.selected),
            "user_selected_keys" => key_set(&self.user_selected),
            "end_audit_source_keys" => key_set(&self.end_audit_sources),
            "execution_order" => Ok(PyTuple::new(py, cache.keys(&self.order)?)?.into_any()),
            "all_keys" => {
                let values = PyDict::new(py);
                for (name, key) in &self.names {
                    values.set_item(name, cache.key(key)?)?;
                }
                Ok(values.into_any())
            }
            "models_by_name" => {
                let values = PyDict::new(py);
                for name in &self.model_order {
                    if let Some(model) = self.models.get(name) {
                        values.set_item(name, model.bind(py))?;
                    }
                }
                Ok(values.into_any())
            }
            "python_read_source_names" => Ok(PyFrozenSet::new(py, &self.python_reads)?.into_any()),
            "sql_test_selection" => Ok(self.tests.bind(py).clone().into_any()),
            "load_sources" => self.load_sources.into_bound_py_any(py),
            _ => Err(pyo3::exceptions::PyAttributeError::new_err(name.to_owned())),
        }
    }
}

/// Read native ownership directly, or import a caller-authored public scope at the API edge.
pub(crate) fn scope_data(value: &Bound<'_, PyAny>) -> PyResult<Arc<ScopeData>> {
    if let Ok(scope) = value.cast::<RetainedScope>() {
        return Ok(scope.get().data.clone());
    }
    let mut models: HashMap<String, ModelHandle> = HashMap::new();
    let mut model_order: Vec<String> = Vec::new();
    for item in mapping_items(&field(value, "models_by_name")?)?.try_iter()? {
        let item = item?;
        let name: String = item.get_item(0)?.extract()?;
        model_order.push(name.clone());
        models.insert(name, ModelHandle::read(&item.get_item(1)?));
    }
    Ok(Arc::new(ScopeData {
        upstream: edges(&field(value, "upstream_deps")?)?,
        downstream: edges(&field(value, "downstream_deps")?)?,
        names: mapping_items(&field(value, "all_keys")?)?
            .try_iter()?
            .map(|item| {
                let item = item?;
                Ok((
                    item.get_item(0)?.extract()?,
                    crate::bindings::_helpers::graph::keys::native_key(&item.get_item(1)?)?,
                ))
            })
            .collect::<PyResult<_>>()?,
        models,
        model_order,
        selected: keys(&field(value, "selected_keys")?)?.into_iter().collect(),
        order: keys(&field(value, "execution_order")?)?,
        user_selected: keys(&field(value, "user_selected_keys")?)?
            .into_iter()
            .collect(),
        python_reads: field(value, "python_read_source_names")?.extract()?,
        tests: field(value, "sql_test_selection")?.extract()?,
        load_sources: field(value, "load_sources")?.is_truthy()?,
        end_audit_sources: keys(&field(value, "end_audit_source_keys")?)?
            .into_iter()
            .collect(),
    }))
}

/// Preserve public record identity and dataclass behavior on explicit projection.
pub(crate) fn project_scope<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let Ok(scope) = value.cast::<RetainedScope>() else {
        return Ok(value.clone());
    };
    scope
        .get()
        .projection
        .get_or_try_init(value.py(), || {
            let names = [
                "upstream_deps",
                "downstream_deps",
                "all_keys",
                "models_by_name",
                "selected_keys",
                "execution_order",
                "user_selected_keys",
                "python_read_source_names",
                "sql_test_selection",
                "load_sources",
                "end_audit_source_keys",
            ];
            let fields = names
                .iter()
                .map(|name| Ok((*name, scope.get().data.field(value.py(), name)?)))
                .collect::<PyResult<Vec<_>>>()?;
            Ok::<_, pyo3::PyErr>(
                new_record_with(value.py(), RecordId::PlannerScope, &fields)?
                    .cast_into::<NativeRecord>()
                    .map(Bound::unbind)?,
            )
        })
        .map(|value| value.bind(scope.py()).clone().into_any())
}
