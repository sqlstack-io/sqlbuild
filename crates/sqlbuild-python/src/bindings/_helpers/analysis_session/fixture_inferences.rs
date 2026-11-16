//! Fixture column inference computed once per fixture body and inference profile.

use crate::bindings::_helpers::analysis_session::fixture_columns::infer_fixture_column_facts;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{pyclass, pymethods};
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use std::sync::Mutex;

/// Reuse inference for identical fixture SQL under an equal, catalog-free profile.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct FixtureColumnInferences {
    by_profile: Mutex<Vec<(Py<PyAny>, Py<PyDict>)>>,
}

#[pymethods]
impl FixtureColumnInferences {
    #[new]
    pub(crate) fn new() -> Self {
        Self {
            by_profile: Mutex::new(Vec::new()),
        }
    }

    /// The inference for one fixture query, computing each distinct query once.
    #[pyo3(signature = (*, query_sql, inference_profile))]
    fn infer<'py>(
        &self,
        query_sql: &Bound<'py, PyAny>,
        inference_profile: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = query_sql.py();
        let compute = || {
            infer_fixture_column_facts(Some(&keywords(
                py,
                &[
                    ("query_sql", query_sql.clone()),
                    ("inference_profile", inference_profile.clone()),
                ],
            )?))
        };
        if !native_field(inference_profile, "binding_catalog")?.is_none() {
            return compute();
        }
        let inferred = self.profile_results(py, inference_profile)?;
        if let Some(found) = inferred.get_item(query_sql)? {
            return Ok(found);
        }
        let computed = compute()?;
        inferred.set_item(query_sql, &computed)?;
        Ok(computed)
    }
}

impl FixtureColumnInferences {
    fn profile_results<'py>(
        &self,
        py: Python<'py>,
        inference_profile: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyDict>> {
        let known: Vec<(Py<PyAny>, Py<PyDict>)> = self
            .by_profile
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("fixture inference cache"))?
            .iter()
            .map(|(profile, results)| (profile.clone_ref(py), results.clone_ref(py)))
            .collect();
        for (profile, results) in known {
            if profile.bind(py).eq(inference_profile)? {
                return Ok(results.into_bound(py));
            }
        }
        let results = PyDict::new(py);
        self.by_profile
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("fixture inference cache"))?
            .push((inference_profile.clone().unbind(), results.clone().unbind()));
        Ok(results)
    }
}
