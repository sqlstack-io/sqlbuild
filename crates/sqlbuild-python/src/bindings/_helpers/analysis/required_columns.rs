//! The external columns a model's query reads.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::required_columns::errors::RequiredColumnsError;
use sqlbuild_analysis::required_columns::main::resolve_required_columns::resolve_required_columns as resolve;
use sqlbuild_analysis::required_columns::models::{ColumnRead, ReferenceInput};

use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_base::types::CompilerDetach;

/// CTE-aware column reads of `query_sql`, resolved to the external resources it references.
#[pyfunction]
#[pyo3(signature = (*, query_sql, references, dialect))]
fn analyze_resolved_column_reads<'py>(
    py: Python<'py>,
    query_sql: &str,
    references: &Bound<'py, PyAny>,
    dialect: Option<&str>,
) -> PyResult<Bound<'py, PyTuple>> {
    let mut inputs: Vec<ReferenceInput> = Vec::new();
    for reference in references.try_iter()? {
        let reference: Bound<'py, PyAny> = reference?;
        inputs.push(ReferenceInput {
            kind: native_field(&reference, "ref_kind")?.str()?.extract()?,
            name: native_field(&reference, "ref_name")?.extract()?,
        });
    }
    let reads: Vec<ColumnRead> = py
        .compiler_detach(|| Ok(resolve(query_sql, &inputs, dialect)))
        .map_err(value_error)?
        .map_err(|error| match error {
            RequiredColumnsError::UnknownDialect(name) => {
                PyValueError::new_err(format!("Unknown dialect: {name}"))
            }
            RequiredColumnsError::Normalization(message) => value_error(message),
        })?;
    let mut facts: Vec<Bound<'py, PyAny>> = Vec::with_capacity(reads.len());
    for read in reads {
        facts.push(new_record_with(
            py,
            RecordId::CompiledLineageSourceFact,
            &[
                (
                    "resource_type",
                    PyString::new(py, &read.resource_type).into_any(),
                ),
                (
                    "resource_name",
                    PyString::new(py, &read.resource_name).into_any(),
                ),
                (
                    "column_name",
                    PyString::new(py, &read.column_name).into_any(),
                ),
            ],
        )?);
    }
    PyTuple::new(py, facts)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(analyze_resolved_column_reads, module)?)
}
