//! Finished session outcomes and pivot proof rows as Python's analysis objects.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_python_base::positional_record;

use crate::bindings::_helpers::lineage::lineage_facts::{confidence_member, transform_kind_member};
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::types::RecordId;

const LINEAGE_NATIVE: &str = "native";

type Object<'py> = Bound<'py, PyAny>;
positional_record! {
    extract;
    /// A session outcome row: its analysis, lineage, binding diagnostics and cleaned SQL.
    struct OutcomeObjects<'py> {
        succeeded: Object<'py>,
        columns: Object<'py>,
        source: String,
        rows: Object<'py>,
        has_star: Object<'py>,
        star_resolved: Object<'py>,
        raw: Object<'py>,
        validated: Object<'py>,
        cleaned: Object<'py>,
    }
}

positional_record! {
    extract;
    /// A dynamic pivot proof row.
    struct ProofObjects<'py> {
        output_proven: Object<'py>,
        columns: Object<'py>,
        families: Object<'py>,
        inputs: Object<'py>,
        failure_reason: Object<'py>,
        bare: Object<'py>,
    }
}

positional_record! {
    extract;
    /// A lineage row: output column, transform and confidence codes and its sources.
    struct LineageObjects<'py> {
        output_column: Object<'py>,
        transform_code: Object<'py>,
        confidence_code: Object<'py>,
        sources: Object<'py>,
    }
}

/// The outcome at `position` of a finished session as `ModelSqlAnalysis`, for hand-built inputs.
#[pyfunction]
#[pyo3(signature = (**request))]
pub(crate) fn session_outcome_analysis<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let request = Request::new("session_outcome_analysis", request)?;
    let py: Python<'py> = request.py();
    let (outcome, contract): (Bound<'py, PyAny>, Bound<'py, PyAny>) = request
        .get("session")?
        .call_method1("outcome_rows", (request.get("position")?,))?
        .extract()?;
    let OutcomeObjects {
        succeeded,
        columns,
        source,
        rows,
        has_star,
        star_resolved,
        raw,
        validated,
        cleaned,
    } = outcome.extract()?;
    let lineage: Bound<'py, PyAny> = if source != LINEAGE_NATIVE {
        lineage_facts(&rows)?
    } else if succeeded.is_truthy()? {
        compact_lineage(&rows)?
    } else {
        PyTuple::empty(py).into_any()
    };
    let mut bindings: Vec<Bound<'py, PyAny>> = Vec::new();
    for row in raw.try_iter()? {
        let row: Bound<'py, PyTuple> =
            PyTuple::new(py, row?.try_iter()?.collect::<PyResult<Vec<_>>>()?)?;
        bindings.push(new_record(py, RecordId::SqlBindingDiagnostic, row, None)?);
    }
    let result: Bound<'py, PyAny> = call(
        &record_class(py, RecordId::PolyglotAnalysisResult)?,
        &[
            ("analysis_succeeded", succeeded),
            ("columns", inferred_columns(&columns)?),
            ("lineage_columns", lineage),
            ("has_star", has_star),
            ("star_resolved", star_resolved),
            (
                "binding_diagnostics",
                PyTuple::new(py, bindings)?.into_any(),
            ),
            ("binding_validated", validated),
        ],
    )?;
    call(
        &record_class(py, RecordId::ModelSqlAnalysis)?,
        &[
            ("polyglot_analysis", result),
            ("placeholders", request.get("placeholders")?),
            ("fused_binding_validated", true.into_bound_py_any(py)?),
            ("cleaned_sql", cleaned),
            ("dynamic_column_contract", proof(&contract.get_item(1)?)?),
        ],
    )
}

/// A native dynamic pivot proof row as Python's proof, None when absent.
#[pyfunction]
pub(crate) fn dynamic_contract_proof<'py>(row: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    proof(row)
}

fn proof<'py>(row: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = row.py();
    if row.is_none() {
        return Ok(row.clone());
    }
    let ProofObjects {
        output_proven,
        columns,
        families,
        inputs,
        failure_reason,
        bare,
    } = row.extract()?;
    let family_class: Bound<'py, PyAny> = record_class(py, RecordId::DynamicColumnFamilyProof)?;
    let mut proofs: Vec<Bound<'py, PyAny>> = Vec::new();
    for family in families.try_iter()? {
        let (name, inferred_type): (Bound<'py, PyAny>, Bound<'py, PyAny>) = family?.extract()?;
        proofs.push(call(
            &family_class,
            &[("name", name), ("inferred_type", inferred_type)],
        )?);
    }
    let fixed: Bound<'py, PyAny> = inferred_columns(&columns)?;
    let fixed: Bound<'py, PyAny> = if fixed.is_truthy()? {
        fixed
    } else {
        PyTuple::empty(py).into_any()
    };
    call(
        &record_class(py, RecordId::DynamicColumnContractProof)?,
        &[
            ("output_proven", output_proven),
            ("fixed_columns", fixed),
            ("families", PyTuple::new(py, proofs)?.into_any()),
            ("input_relations", tuple_of(&inputs)?),
            ("failure_reason", failure_reason),
            ("bare_dynamic_pivot", bare),
        ],
    )
}

/// `(name, type, nullability)` rows as `InferredColumn`s, None for None.
fn inferred_columns<'py>(rows: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = rows.py();
    if rows.is_none() {
        return Ok(rows.clone());
    }
    let column_class: Bound<'py, PyAny> = record_class(py, RecordId::InferredColumn)?;
    let mut columns: Vec<Bound<'py, PyAny>> = Vec::new();
    for row in rows.try_iter()? {
        let (name, data_type, nullability): (
            Bound<'py, PyAny>,
            Bound<'py, PyAny>,
            Bound<'py, PyAny>,
        ) = row?.extract()?;
        columns.push(call(
            &column_class,
            &[
                ("name", name),
                ("type", data_type),
                (
                    "nullability",
                    enum_from_value(py, RecordId::InferredNullability, nullability)?,
                ),
            ],
        )?);
    }
    Ok(PyTuple::new(py, columns)?.into_any())
}

/// Native lineage rows as Python's compact facts over a per-model string pool.
fn compact_lineage<'py>(rows: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = rows.py();
    let pool: Bound<'py, PyDict> = PyDict::new(py);
    let index = |value: Bound<'py, PyAny>| -> PyResult<Bound<'py, PyAny>> {
        if let Some(existing) = pool.get_item(&value)? {
            return Ok(existing);
        }
        let position = pool.len().into_bound_py_any(py)?;
        pool.set_item(&value, &position)?;
        Ok(position)
    };
    let mut compact: Vec<Bound<'py, PyAny>> = Vec::new();
    for row in rows.try_iter()? {
        let LineageObjects {
            output_column,
            transform_code,
            confidence_code,
            sources,
        } = row?.extract()?;
        let mut source_indexes: Vec<Bound<'py, PyAny>> = Vec::new();
        for source in sources.try_iter()? {
            let (resource_type, resource_name, column_name): (
                Bound<'py, PyAny>,
                Bound<'py, PyAny>,
                Bound<'py, PyAny>,
            ) = source?.extract()?;
            let indexes = [
                index(resource_type)?,
                index(resource_name)?,
                index(column_name)?,
            ];
            source_indexes.push(PyTuple::new(py, indexes)?.into_any());
        }
        let output: Bound<'py, PyAny> = index(output_column)?;
        compact.push(
            PyTuple::new(
                py,
                [
                    output,
                    transform_code,
                    confidence_code,
                    PyTuple::new(py, source_indexes)?.into_any(),
                ],
            )?
            .into_any(),
        );
    }
    let keys: Bound<'py, PyAny> = PyTuple::new(py, pool.keys())?.into_any();
    call(
        &record_class(py, RecordId::CompactLineageFacts)?,
        &[
            ("string_pool", keys),
            ("rows", PyTuple::new(py, compact)?.into_any()),
        ],
    )
}

/// Native lineage rows as the plain facts Python's re-analysis returns.
fn lineage_facts<'py>(rows: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = rows.py();
    let column_class: Bound<'py, PyAny> = record_class(py, RecordId::CompiledLineageColumnFact)?;
    let source_class: Bound<'py, PyAny> = record_class(py, RecordId::CompiledLineageSourceFact)?;
    let mut facts: Vec<Bound<'py, PyAny>> = Vec::new();
    for row in rows.try_iter()? {
        let LineageObjects {
            output_column,
            transform_code,
            confidence_code,
            sources,
        } = row?.extract()?;
        let mut upstream: Vec<Bound<'py, PyAny>> = Vec::new();
        for source in sources.try_iter()? {
            let (resource_type, resource_name, column_name): (
                Bound<'py, PyAny>,
                Bound<'py, PyAny>,
                Bound<'py, PyAny>,
            ) = source?.extract()?;
            upstream.push(call(
                &source_class,
                &[
                    ("resource_type", resource_type),
                    ("resource_name", resource_name),
                    ("column_name", column_name),
                ],
            )?);
        }
        facts.push(call(
            &column_class,
            &[
                ("output_column", output_column),
                ("upstream_columns", PyTuple::new(py, upstream)?.into_any()),
                (
                    "transform_kind",
                    transform_kind_member(py, transform_code.extract()?)?.into_bound(py),
                ),
                (
                    "confidence",
                    confidence_member(py, confidence_code.extract()?)?.into_bound(py),
                ),
            ],
        )?);
    }
    Ok(PyTuple::new(py, facts)?.into_any())
}

fn tuple_of<'py>(items: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyTuple::new(items.py(), items.try_iter()?.collect::<PyResult<Vec<_>>>()?)?.into_any())
}

fn call<'py>(
    callable: &Bound<'py, PyAny>,
    pairs: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: Bound<'py, PyDict> = PyDict::new(callable.py());
    for (name, value) in pairs {
        arguments.set_item(*name, value)?;
    }
    callable.call((), Some(&arguments))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(session_outcome_analysis, module)?)?;
    module.add_function(wrap_pyfunction!(dynamic_contract_proof, module)?)
}
