//! The compile-lifetime binding catalog: known relation shapes around one native project catalog.

use std::sync::atomic::{AtomicBool, Ordering};

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyDictMethods, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyBool, PyDict, PyMapping, PyTuple};
use pyo3::{pyclass, pyfunction, pymethods, wrap_pyfunction};
use sqlbuild_analysis::semantic_validation::models as validation;
use sqlbuild_analysis::semantic_validation::models::DiagnosticRow;

use crate::bindings::types::IntoPositional;
use sqlbuild_analysis::semantic_validation::types::{BindingRequest, Relations};

use crate::bindings::_helpers::analysis::conversions;
use crate::bindings::_helpers::boundary::panics::compiler_error;
use crate::bindings::_helpers::records::construct::new_record;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::fields::mapping_get_or;
use crate::bindings::_helpers::records::fields::mapping_items;
use crate::bindings::_helpers::records::fields::mapping_values;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::models::{CatalogInput, ProjectCatalog};
use crate::bindings::types::CompilerDetach;

/// Dialects whose quoted identifiers bind case-sensitively unless the session says otherwise.
pub(crate) const CASE_SENSITIVE_BINDING_DIALECTS: [&str; 2] = ["snowflake", "postgresql"];
/// Adapter dialect names and the native dialect each validates as.
pub(crate) const NATIVE_DIALECT_ALIASES: [(&str, &str); 3] = [
    ("postgres", "postgresql"),
    ("motherduck", "duckdb"),
    ("sqlserver", "tsql"),
];
const QUOTED_IDENTIFIER_DELIMITER: char = '"';
const SNOWFLAKE_DIALECT: &str = "snowflake";
const QUOTED_CASE_PARAMETER: &str = "QUOTED_IDENTIFIERS_IGNORE_CASE";
const SESSION_PARAMETERS: &str = "session_parameters";
const GENERIC_DIALECT: &str = "generic";

/// One native project catalog and the relation shapes the compile has told it about.
#[pyclass(module = "sqlbuild._native")]
pub(crate) struct BindingCatalog {
    #[pyo3(get)]
    pub(crate) schemas: Py<PyDict>,
    #[pyo3(get)]
    pub(crate) dialect: String,
    #[pyo3(get)]
    pub(crate) quoted_ignore_case: bool,
    #[pyo3(get)]
    pub(crate) analysis_shapes: Py<PyDict>,
    #[pyo3(get)]
    pub(crate) expression_shapes: Py<PyDict>,
    #[pyo3(get)]
    pub(crate) native: Py<ProjectCatalog>,
    /// Whether binding preparation has added relations to `schemas`.
    pub(crate) extended: AtomicBool,
}

/// A catalog's dialect, case mode, known functions and known types.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct CatalogKey {
    pub(crate) dialect: String,
    pub(crate) quoted_ignore_case: bool,
    pub(crate) known_functions: Vec<String>,
    pub(crate) known_types: Vec<String>,
}

impl BindingCatalog {
    /// Whether `schemas` holds exactly the relations it was created with and later given.
    pub(crate) fn unextended(&self) -> bool {
        !self.extended.load(Ordering::Relaxed)
    }

    /// A catalog for `key` that starts with `relations`.
    pub(crate) fn create(
        py: Python<'_>,
        key: CatalogKey,
        relations: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
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
        schemas.update(relations.downcast::<PyMapping>()?)?;
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

    /// `BindingCatalog.prepare`: add unknown closed relations, then override changed ones.
    fn prepared(
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
                if columns.is_truthy()? && !schemas.contains(&name)? {
                    additions.set_item(&name, &columns)?;
                    schemas.set_item(&name, &columns)?;
                }
            }
        }
        if !additions.is_empty() {
            self.extended.store(true, Ordering::Relaxed);
            let relations: Relations = conversions::relations(additions.as_any())?;
            self.native.borrow_mut(py).inner.update_relations(relations);
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
    fn new(py: Python<'_>, request: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
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
    fn prepare_analysis(
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
    fn with_relations(&self, py: Python<'_>, relations: &Bound<'_, PyAny>) -> PyResult<Self> {
        let schemas: Bound<'_, PyDict> = self.schemas.bind(py).copy()?;
        schemas.update(relations.downcast::<PyMapping>()?)?;
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
    fn inferred_schema<'py>(
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

/// `ignores_quoted_case`: whether the connection explicitly enables case-insensitive quoting.
pub(crate) fn ignores_quoted_case(
    connection: &Bound<'_, PyAny>,
    dialect: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    if dialect.is_none() || dialect.extract::<String>()? != SNOWFLAKE_DIALECT {
        return Ok(false);
    }
    let parameters: Bound<'_, PyAny> = mapping_get(connection, SESSION_PARAMETERS)?;
    let Ok(parameters) = parameters.downcast::<PyDict>() else {
        return Ok(false);
    };
    for (key, value) in parameters.iter() {
        let upper: Bound<'_, PyAny> = key.str()?.call_method0("upper")?;
        if upper.eq(QUOTED_CASE_PARAMETER)? && value.is(PyBool::new(key.py(), true)) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The native dialect a request's adapter dialect validates as.
fn native_dialect(dialect: &Bound<'_, PyAny>) -> PyResult<String> {
    let name: String = if dialect.is_truthy()? {
        dialect.extract()?
    } else {
        GENERIC_DIALECT.to_owned()
    };
    Ok(NATIVE_DIALECT_ALIASES
        .iter()
        .find(|(alias, _)| *alias == name)
        .map_or(name.clone(), |(_, native)| (*native).to_owned()))
}

/// Binding diagnostics for a batch of complete-schema validation requests, in request order.
#[pyfunction]
#[pyo3(signature = (*, requests))]
pub(crate) fn schema_validations<'py>(
    py: Python<'py>,
    requests: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let requests: Vec<Bound<'py, PyAny>> = requests.try_iter()?.collect::<PyResult<_>>()?;
    let mut created: Vec<(CatalogKey, Py<BindingCatalog>)> = Vec::new();
    let mut groups: Vec<(Py<BindingCatalog>, bool, Vec<usize>)> = Vec::new();
    for (index, request) in requests.iter().enumerate() {
        let key: CatalogKey = CatalogKey {
            dialect: native_dialect(&native_field(request, "dialect")?)?,
            quoted_ignore_case: native_field(request, "quoted_identifiers_ignore_case")?
                .extract()?,
            known_functions: native_field(request, "known_functions")?.extract()?,
            known_types: native_field(request, "known_types")?.extract()?,
        };
        let explicit: Bound<'py, PyAny> = native_field(request, "catalog")?;
        let own: bool = !explicit.is_none();
        let catalog: Py<BindingCatalog> = if own {
            explicit.extract()?
        } else if let Some((_, catalog)) = created.iter().find(|(known, _)| *known == key) {
            catalog.clone_ref(py)
        } else {
            let catalog: Py<BindingCatalog> = Py::new(
                py,
                BindingCatalog::create(py, key.clone(), &native_field(request, "schema")?)?,
            )?;
            created.push((key, catalog.clone_ref(py)));
            catalog
        };
        match groups.iter_mut().find(|(known, _, _)| known.is(&catalog)) {
            Some((_, any_explicit, members)) => {
                *any_explicit |= own;
                members.push(index);
            }
            None => groups.push((catalog, own, vec![index])),
        }
    }
    let empty: Bound<'py, PyAny> = new_record(py, RecordId::SqlBindingResult, (), None)?;
    let mut results: Vec<Bound<'py, PyAny>> = vec![empty; requests.len()];
    for (catalog, any_explicit, members) in groups {
        let catalog = catalog.borrow(py);
        let batch: Vec<Bound<'py, PyAny>> = members
            .iter()
            .map(|index| requests[*index].clone())
            .collect();
        let prepared: Vec<BindingRequest> = catalog.prepared(py, &batch)?;
        let native = catalog.native.borrow(py);
        let inner: &validation::ProjectCatalog = &native.inner;
        let dialect: &str = &catalog.dialect;
        let rows: Vec<Vec<DiagnosticRow>> = if any_explicit {
            py.compiler_detach(|| inner.binding_results(prepared))
        } else {
            py.compiler_detach(|| inner.schema_validation_rows(dialect, prepared))
        }
        .map_err(compiler_error)?;
        if rows.len() != members.len() {
            return Err(compiler_error(
                "native SQL schema validation returned an invalid batch response",
            ));
        }
        for (index, diagnostics) in members.into_iter().zip(rows) {
            let built: Vec<Bound<'py, PyAny>> = diagnostics
                .into_iter()
                .map(|row| {
                    new_record(
                        py,
                        RecordId::SqlBindingDiagnostic,
                        row.into_positional(py)?,
                        None,
                    )
                })
                .collect::<PyResult<_>>()?;
            let keywords: Bound<'py, PyDict> = PyDict::new(py);
            keywords.set_item("diagnostics", PyTuple::new(py, built)?)?;
            results[index] = new_record(py, RecordId::SqlBindingResult, (), Some(&keywords))?;
        }
    }
    PyTuple::new(py, results)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<BindingCatalog>()?;
    module.add_function(wrap_pyfunction!(schema_validations, module)?)?;
    Ok(())
}
