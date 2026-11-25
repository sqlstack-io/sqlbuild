//! Public records of planner scope.

use crate::graph::main::project::edge_dict::edge_dict;
use crate::graph::models::KeyCache;
use crate::native_host::models::ModelHandle;
use crate::planner_scope::_helpers::retained::project_scope;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyFrozenSet;
use pyo3::types::PyTuple;
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_python_base::records::models::NativeRecord;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

/// The graph and selection used throughout planning; model values remain compiled-resource views.
#[derive(Debug)]
pub struct ScopeData {
    pub upstream: Vec<(ObjectKey, Vec<ObjectKey>)>,
    pub downstream: Vec<(ObjectKey, Vec<ObjectKey>)>,
    pub names: Vec<(String, ObjectKey)>,
    pub models: HashMap<String, ModelHandle>,
    pub model_order: Vec<String>,
    pub selected: HashSet<ObjectKey>,
    pub order: Vec<ObjectKey>,
    pub user_selected: HashSet<ObjectKey>,
    pub python_reads: HashSet<String>,
    pub tests: Py<NativeRecord>,
    pub load_sources: bool,
    pub end_audit_sources: HashSet<ObjectKey>,
    /// Audits selected by name, with `audit:` or as a past run's failed audits, whatever their targets.
    pub audits: HashSet<String>,
}

/// Internal carrier, never returned in place of a supported public PlannerScope record.
#[pyclass(frozen)]
pub struct RetainedScope {
    pub data: Arc<ScopeData>,
    pub(crate) projection: PyOnceLock<Py<NativeRecord>>,
}

impl std::fmt::Debug for RetainedScope {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RetainedScope")
            .finish_non_exhaustive()
    }
}

impl ScopeData {
    pub fn object(self, py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
        Ok(Bound::new(
            py,
            RetainedScope {
                data: Arc::new(self),
                projection: PyOnceLock::new(),
            },
        )?
        .into_any())
    }

    pub fn model<'py>(&self, py: Python<'py>, name: &str) -> Option<Bound<'py, PyAny>> {
        self.models.get(name).map(|model| model.bind(py))
    }

    pub fn with_selected(&self, py: Python<'_>, selected: HashSet<ObjectKey>) -> Self {
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
            audits: self.audits.clone(),
        }
    }

    /// Public field projection; internal graph consumers read the native fields above instead.
    pub fn field<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
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
            "selected_audit_names" => Ok(PyFrozenSet::new(py, &self.audits)?.into_any()),
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

/// Resolved planner scopes for one plan build.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct PlannerScopeResolution {
    pub selected_scope: Py<PyAny>,
    pub stale_warning_scope: Py<PyAny>,
    pub inspection_scope: Py<PyAny>,
}

#[pymethods]
impl PlannerScopeResolution {
    #[new]
    #[pyo3(signature = (*, selected_scope, stale_warning_scope, inspection_scope))]
    fn new(
        selected_scope: Py<PyAny>,
        stale_warning_scope: Py<PyAny>,
        inspection_scope: Py<PyAny>,
    ) -> Self {
        Self {
            selected_scope,
            stale_warning_scope,
            inspection_scope,
        }
    }

    #[getter]
    fn selected_scope<'py>(&self, py: pyo3::Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        project_scope(self.selected_scope.bind(py))
    }

    #[getter]
    fn stale_warning_scope<'py>(&self, py: pyo3::Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        project_scope(self.stale_warning_scope.bind(py))
    }

    #[getter]
    fn inspection_scope<'py>(&self, py: pyo3::Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        project_scope(self.inspection_scope.bind(py))
    }
}
