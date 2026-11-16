//! Python methods of the compile-owned binding catalog and its compact analysis jobs.

use crate::analysis::_helpers::conversions::analysis_relations;
use crate::analysis::_helpers::conversions::binding_requests;
use crate::analysis::_helpers::conversions::columns;
use crate::analysis::_helpers::conversions::relations;
use crate::analysis::_helpers::native_classes::CatalogInput;
use crate::analysis::_helpers::normalization_results::normalization_results;
use crate::analysis::models::Positional;
use crate::analysis::models::{CompactAnalysisJob, ProjectCatalog};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, Py, PyAny, PyResult, Python};
use pyo3::pymethods;
use pyo3::types::PyBytes;
use sqlbuild_analysis::semantic_validation::models as validation;
use sqlbuild_analysis::semantic_validation::models::DiagnosticRow;
use sqlbuild_analysis::semantic_validation::types::{
    BindingRequest, NormalizationRequest, Relations,
};
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::types::CompilerDetach;
use std::collections::HashMap;

#[pymethods]
impl ProjectCatalog {
    fn inferred_schema(
        &self,
        py: Python<'_>,
        sql: &str,
        #[pyo3(from_py_with = columns)] columns: validation::Columns,
        #[pyo3(from_py_with = relations)] inputs: Relations,
    ) -> PyResult<Vec<(String, Option<String>)>> {
        py.compiler_detach(|| self.inner.inferred_schema(sql, columns, inputs))
            .map_err(compiler_error)
    }

    #[new]
    fn new(request: CatalogInput) -> PyResult<Self> {
        validation::ProjectCatalog::new(request.into())
            .map(|inner| Self { inner })
            .map_err(PyValueError::new_err)
    }

    fn update_relations(&mut self, #[pyo3(from_py_with = relations)] relations: Relations) {
        self.inner.update_relations(relations);
    }

    fn with_relations(&self, #[pyo3(from_py_with = relations)] relations: Relations) -> Self {
        Self {
            inner: self.inner.with_relations(relations),
        }
    }

    fn update_analysis(
        &mut self,
        #[pyo3(from_py_with = analysis_relations)] relations: HashMap<
            String,
            (validation::Columns, validation::Columns),
        >,
    ) {
        self.inner.update_analysis(relations);
    }

    fn register_override(
        &mut self,
        #[pyo3(from_py_with = relations)] relations: Relations,
    ) -> usize {
        self.inner.register_override(relations)
    }

    fn validation_payload(
        &self,
        #[pyo3(from_py_with = binding_requests)] requests: Vec<BindingRequest>,
    ) -> PyResult<String> {
        self.inner
            .validation_payload(requests)
            .map_err(compiler_error)
    }

    /// Resolve a compact batch now; the job runs later while the catalog keeps changing.
    fn prepare_compact(&self, py: Python<'_>, payload: &[u8]) -> PyResult<CompactAnalysisJob> {
        let text = std::str::from_utf8(payload)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        py.compiler_detach(|| self.inner.prepare_compact(text))
            .map(|inner| CompactAnalysisJob { inner })
            .map_err(compiler_error)
    }

    fn binding_results(
        &self,
        py: Python<'_>,
        #[pyo3(from_py_with = binding_requests)] requests: Vec<BindingRequest>,
    ) -> PyResult<Vec<Vec<Positional<DiagnosticRow>>>> {
        let rows: Vec<Vec<DiagnosticRow>> = py
            .compiler_detach(|| self.inner.binding_results(requests))
            .map_err(compiler_error)?;
        Ok(rows.into_iter().map(Positional::wrap_all).collect())
    }

    /// Normalize a preparation batch on the idle analysis pool, else in turn, in one detached call.
    fn normalize_analysis_sqls(
        &self,
        py: Python<'_>,
        dialect: &str,
        requests: Vec<NormalizationRequest>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let results = py
            .compiler_detach(|| self.inner.normalize_analysis_sqls(dialect, requests))
            .map_err(compiler_error)?;
        normalization_results(py, results)
    }
}

#[pymethods]
impl CompactAnalysisJob {
    fn run<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let analysis = self
            .inner
            .take_analysis()
            .ok_or_else(|| PyValueError::new_err("compact analysis job already ran"))?;
        let result = py
            .compiler_detach(|| self.inner.run(analysis))
            .map_err(compiler_error)?;
        Ok(PyBytes::new(py, result.as_bytes()))
    }
}
