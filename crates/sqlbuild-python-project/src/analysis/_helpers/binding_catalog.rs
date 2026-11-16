//! The compile-lifetime binding catalog: known relation shapes around one native project catalog.

use crate::analysis::constants::NATIVE_DIALECT_ALIASES;
use crate::analysis::models::BindingCatalog;
use crate::analysis::models::CatalogKey;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyDictMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_analysis::semantic_validation::models as validation;
use sqlbuild_analysis::semantic_validation::models::DiagnosticRow;

use sqlbuild_analysis::semantic_validation::types::BindingRequest;
use sqlbuild_python_base::types::IntoPositional;

use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_base::types::CompilerDetach;

pub(crate) const QUOTED_IDENTIFIER_DELIMITER: char = '"';
pub(crate) const SNOWFLAKE_DIALECT: &str = "snowflake";
pub(crate) const QUOTED_CASE_PARAMETER: &str = "QUOTED_IDENTIFIERS_IGNORE_CASE";
pub(crate) const SESSION_PARAMETERS: &str = "session_parameters";
const GENERIC_DIALECT: &str = "generic";

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
