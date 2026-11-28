//! Public records of analysis.

use crate::analysis::_helpers::binding_catalog::QUOTED_IDENTIFIER_DELIMITER;
use crate::analysis::_helpers::conversions;
use crate::analysis::_helpers::native_classes::CatalogInput;
use crate::analysis::constants::CASE_SENSITIVE_BINDING_DIALECTS;
use pyo3::FromPyObject;
use pyo3::exceptions::PyTypeError;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::Bound;
use pyo3::prelude::IntoPyObject;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyDictMethods;
use pyo3::prelude::PyErr;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::types::PyDict;
use pyo3::types::PyMapping;
use pyo3::types::PyTuple;
use sqlbuild_analysis::semantic_validation::models as validation;
use sqlbuild_analysis::semantic_validation::types::BindingRequest;
use sqlbuild_analysis::semantic_validation::types::Relations;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_base::records::main::fields::mapping_values::mapping_values;
use sqlbuild_python_base::types::CompilerDetach;
use sqlbuild_python_base::types::FromPositional;
use sqlbuild_python_base::types::IntoPositional;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

/// One native project catalog and the relation shapes the compile has told it about.
#[pyclass(module = "sqlbuild._native")]
#[derive(Debug)]
pub struct BindingCatalog {
    #[pyo3(get)]
    pub schemas: Py<PyDict>,
    #[pyo3(get)]
    pub dialect: String,
    #[pyo3(get)]
    pub quoted_ignore_case: bool,
    #[pyo3(get)]
    pub analysis_shapes: Py<PyDict>,
    #[pyo3(get)]
    pub expression_shapes: Py<PyDict>,
    #[pyo3(get)]
    pub native: Py<ProjectCatalog>,
    /// Whether binding preparation has added relations to `schemas`.
    pub extended: AtomicBool,
}

/// A catalog's dialect, case mode, known functions and known types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogKey {
    pub dialect: String,
    pub quoted_ignore_case: bool,
    pub known_functions: Vec<String>,
    pub known_types: Vec<String>,
}

impl BindingCatalog {
    /// Whether `schemas` holds exactly the relations it was created with and later given.
    pub fn unextended(&self) -> bool {
        !self.extended.load(Ordering::Relaxed)
    }

    /// A catalog for `key` that starts with `relations`.
    pub fn create(py: Python<'_>, key: CatalogKey, relations: &Bound<'_, PyAny>) -> PyResult<Self> {
        let CatalogKey {
            dialect,
            quoted_ignore_case,
            known_functions,
            known_types,
        } = key;
        let input: CatalogInput = CatalogInput {
            dialect: dialect.clone(),
            quoted_ignore_case,
            known_functions,
            known_types,
            relations: conversions::relations(relations)?,
        };
        let native: ProjectCatalog = validation::ProjectCatalog::new(input.into())
            .map(|inner| ProjectCatalog { inner })
            .map_err(PyValueError::new_err)?;
        let schemas: Bound<'_, PyDict> = PyDict::new(py);
        schemas.update(relations.cast::<PyMapping>()?)?;
        Ok(Self {
            schemas: schemas.unbind(),
            dialect,
            quoted_ignore_case,
            analysis_shapes: PyDict::new(py).unbind(),
            expression_shapes: PyDict::new(py).unbind(),
            extended: AtomicBool::new(false),
            native: Py::new(py, native)?,
        })
    }

    /// `BindingCatalog.prepare`: add unknown closed relations (overrides while in use), then changed ones.
    pub(crate) fn prepared(
        &self,
        py: Python<'_>,
        requests: &[Bound<'_, PyAny>],
    ) -> PyResult<Vec<BindingRequest>> {
        let schemas: &Bound<'_, PyDict> = self.schemas.bind(py);
        let additions: Bound<'_, PyDict> = PyDict::new(py);
        for request in requests {
            let schema: Bound<'_, PyAny> = native_field(request, "schema")?;
            for item in mapping_items(&schema)?.try_iter()? {
                let (name, columns): (Bound<'_, PyAny>, Bound<'_, PyAny>) = item?.extract()?;
                if columns.is_truthy()?
                    && !schemas.contains(&name)?
                    && !additions.contains(&name)?
                {
                    additions.set_item(&name, &columns)?;
                }
            }
        }
        if !additions.is_empty()
            && let Ok(mut native) = self.native.try_borrow_mut(py)
        {
            self.extended.store(true, Ordering::Relaxed);
            schemas.update(additions.as_mapping())?;
            let relations: Relations = conversions::relations(additions.as_any())?;
            native.inner.update_relations(relations);
        }
        let mut prepared: Vec<BindingRequest> = Vec::with_capacity(requests.len());
        for request in requests {
            let schema: Bound<'_, PyAny> = native_field(request, "schema")?;
            let mut names: Vec<String> = Vec::new();
            for name in schema.try_iter()? {
                names.push(name?.extract()?);
            }
            names.sort();
            let mut closed: Vec<(String, bool)> = Vec::with_capacity(names.len());
            let overrides: Bound<'_, PyDict> = PyDict::new(py);
            for name in names {
                let columns: Bound<'_, PyAny> = schema.get_item(&name)?;
                let truthy: bool = columns.is_truthy()?;
                if truthy
                    && !schemas
                        .get_item(&name)?
                        .is_some_and(|known| known.eq(&columns).unwrap_or(false))
                {
                    overrides.set_item(&name, &columns)?;
                }
                closed.push((name, truthy));
            }
            let sql: String = native_field(request, "sql")?.extract()?;
            prepared.push((sql, closed, conversions::relations(overrides.as_any())?));
        }
        Ok(prepared)
    }
}

#[pymethods]
impl BindingCatalog {
    /// `BindingCatalog(dialect=, quoted_ignore_case=, known_functions=, known_types=, relations=)`.
    #[new]
    #[pyo3(signature = (**request))]
    pub(crate) fn new(py: Python<'_>, request: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let request: Bound<'_, PyDict> = request.cloned().unwrap_or_else(|| PyDict::new(py));
        let field = |name: &str| -> PyResult<Bound<'_, PyAny>> {
            request.get_item(name)?.ok_or_else(|| {
                PyTypeError::new_err(format!(
                    "BindingCatalog() missing keyword argument '{name}'"
                ))
            })
        };
        let key: CatalogKey = CatalogKey {
            dialect: field("dialect")?.extract()?,
            quoted_ignore_case: field("quoted_ignore_case")?.extract()?,
            known_functions: field("known_functions")?.extract()?,
            known_types: field("known_types")?.extract()?,
        };
        Self::create(py, key, &field("relations")?)
    }

    /// Send the native catalog only the analysis shapes that changed.
    #[pyo3(signature = (*, types, nullability))]
    pub(crate) fn prepare_analysis(
        &self,
        py: Python<'_>,
        types: &Bound<'_, PyAny>,
        nullability: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let shapes: &Bound<'_, PyDict> = self.analysis_shapes.bind(py);
        let updates: Bound<'_, PyDict> = PyDict::new(py);
        for item in mapping_items(nullability)?.try_iter()? {
            let (name, columns): (Bound<'_, PyAny>, Bound<'_, PyAny>) = item?.extract()?;
            let typed: Bound<'_, PyAny> = mapping_get_or(types, &name, PyDict::new(py))?;
            let shape: Bound<'_, PyTuple> = PyTuple::new(py, [typed, columns])?;
            let unchanged: bool = match shapes.get_item(&name)? {
                Some(known) => known.eq(&shape)?,
                None => false,
            };
            if !unchanged {
                updates.set_item(&name, &shape)?;
                shapes.set_item(&name, &shape)?;
            }
        }
        if !updates.is_empty() {
            self.native
                .bind(py)
                .call_method1("update_analysis", (updates,))?;
        }
        Ok(())
    }

    /// A copy whose relations also include `relations`.
    pub(crate) fn with_relations(
        &self,
        py: Python<'_>,
        relations: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let schemas: Bound<'_, PyDict> = self.schemas.bind(py).copy()?;
        schemas.update(relations.cast::<PyMapping>()?)?;
        let native: ProjectCatalog = ProjectCatalog {
            inner: self
                .native
                .borrow(py)
                .inner
                .with_relations(conversions::relations(relations)?),
        };
        Ok(Self {
            schemas: schemas.unbind(),
            dialect: self.dialect.clone(),
            quoted_ignore_case: self.quoted_ignore_case,
            analysis_shapes: self.analysis_shapes.bind(py).copy()?.unbind(),
            expression_shapes: self.expression_shapes.bind(py).copy()?.unbind(),
            native: Py::new(py, native)?,
            extended: AtomicBool::new(true),
        })
    }

    /// The output schema with exact-case names where quoted identifiers bind case-sensitively.
    #[pyo3(signature = (*, sql, columns, inputs))]
    pub(crate) fn inferred_schema<'py>(
        &self,
        py: Python<'py>,
        sql: &str,
        columns: &Bound<'py, PyAny>,
        inputs: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if self.quoted_ignore_case
            || !CASE_SENSITIVE_BINDING_DIALECTS.contains(&self.dialect.as_str())
        {
            return Ok(columns.clone());
        }
        let mut exact_input: bool = false;
        for shape in mapping_values(inputs)?.try_iter()? {
            for name in shape?.try_iter()? {
                let name: String = name?.extract()?;
                exact_input |= name.contains(QUOTED_IDENTIFIER_DELIMITER);
            }
            if exact_input {
                break;
            }
        }
        if !sql.contains(QUOTED_IDENTIFIER_DELIMITER) && !exact_input {
            return Ok(columns.clone());
        }
        let columns = conversions::columns(columns)?;
        let inputs: Relations = conversions::relations(inputs)?;
        let native = self.native.borrow(py);
        let inner: &validation::ProjectCatalog = &native.inner;
        let rows: Vec<(String, Option<String>)> = py
            .compiler_detach(|| inner.inferred_schema(sql, columns, inputs))
            .map_err(compiler_error)?;
        let inferred: Bound<'py, PyDict> = PyDict::new(py);
        for (name, type_name) in rows {
            inferred.set_item(name, type_name)?;
        }
        Ok(inferred.into_any())
    }
}

/// The query, relation schemas and binding context of one schema-validation request.
#[derive(Debug)]
pub struct SchemaRequestParts<'py> {
    pub query_sql: String,
    pub placeholders: Bound<'py, PyAny>,
    pub dialect: Bound<'py, PyAny>,
    pub schema: Bound<'py, PyAny>,
    pub known_functions: Bound<'py, PyAny>,
    pub known_types: Bound<'py, PyAny>,
    pub catalog: Bound<'py, PyAny>,
    pub quoted_identifiers_ignore_case: bool,
}

#[derive(FromPyObject, Debug)]
#[pyo3(from_item_all)]
pub struct NormalizationInput {
    pub sql: String,
    pub dialect: String,
    pub stubs: HashMap<String, String>,
    pub placeholders: HashMap<String, String>,
}

#[pyclass(module = "sqlbuild._native")]
#[derive(Debug)]
pub struct ProjectCatalog {
    pub inner: validation::ProjectCatalog,
}

/// One resolved compact analysis batch that runs without borrowing its project catalog.
#[pyclass(module = "sqlbuild._native", frozen)]
pub struct CompactAnalysisJob {
    pub inner: validation::CompactAnalysisJob,
}

impl std::fmt::Debug for CompactAnalysisJob {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompactAnalysisJob")
            .finish_non_exhaustive()
    }
}

#[pyclass(module = "sqlbuild._native")]
#[derive(Debug)]
pub struct BindingPositions {
    pub inner: validation::BindingPositions,
}

impl From<NormalizationInput> for validation::NormalizationInput {
    fn from(input: NormalizationInput) -> Self {
        Self {
            sql: input.sql,
            dialect: input.dialect,
            stubs: input.stubs,
            placeholders: input.placeholders,
        }
    }
}

/// A record of another crate exchanged with Python as its positional tuple.
#[derive(Clone, Debug)]
pub struct Positional<T>(pub T);

impl<T> Positional<T> {
    /// Each record of `items`, for Python.
    pub fn wrap_all(items: Vec<T>) -> Vec<Self> {
        items.into_iter().map(Self).collect()
    }

    /// Each record Python sent in `items`.
    pub fn unwrap_all(items: Vec<Self>) -> Vec<T> {
        items.into_iter().map(|item| item.0).collect()
    }
}

impl<'py, T: FromPositional> FromPyObject<'_, 'py> for Positional<T> {
    type Error = pyo3::PyErr;

    fn extract(value: pyo3::Borrowed<'_, 'py, pyo3::PyAny>) -> PyResult<Self> {
        let value: &Bound<'py, PyAny> = &value;
        T::from_positional(value).map(Self)
    }
}

impl<'py, T: IntoPositional> IntoPyObject<'py> for Positional<T> {
    type Target = PyTuple;
    type Output = Bound<'py, PyTuple>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        self.0.into_positional(py)
    }
}
