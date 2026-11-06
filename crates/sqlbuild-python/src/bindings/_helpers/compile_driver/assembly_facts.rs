//! Every model's native facts and the project's resource facts, from the native stages.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyTuple};

use crate::bindings::_helpers::analysis_session::profile_inputs::dialect_or_generic;
use crate::bindings::_helpers::analysis_session::session::{
    FamilyRow, NativeModelAnalysisSession, PivotRequestRow, prove_dynamic_column_contracts,
};
use crate::bindings::_helpers::compile_driver::assembly_analysis::AnalysisContext;
use crate::bindings::_helpers::compile_driver::stage_work::{
    PIVOT_PROOFS, SESSION_PIVOT_PROOFS, count_stage,
};
use crate::bindings::_helpers::model_config::header_families::HeaderFamilies;
use crate::bindings::_helpers::project_assembly::models::{NativeModelFacts, NativeProjectFacts};
use crate::bindings::_helpers::project_assembly::native_resources::assemble_native_project_resources;
use crate::bindings::_helpers::project_assembly::syntax_checks::model_syntax_checks;
use crate::bindings::_helpers::project_assembly::syntax_checks::standalone_pivot_rows;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::construct::record_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use pyo3::IntoPyObjectExt;

/// `NativeProjectFacts` of `inputs`: resources, syntax validity, deps and standalone pivot proofs.
pub(crate) fn native_project_facts<'py>(
    inputs: &Bound<'py, PyAny>,
    context: &AnalysisContext<'py>,
    names: &Bound<'py, PyAny>,
    session: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = inputs.py();
    let model_inputs: Bound<'py, PyAny> = native_field(inputs, "model_inputs")?;
    let dialect: Bound<'py, PyAny> = native_field(&context.profile, "sql_analysis_dialect")?;
    let (succeeded, positions): (Bound<'py, PyAny>, Bound<'py, PyAny>) = match &context.analyses {
        Some(analyses) => (
            analyses.succeeded.clone().into_any(),
            analyses.positions.clone().into_any(),
        ),
        None => (
            PyFrozenSet::empty(py)?.into_any(),
            PyDict::new(py).into_any(),
        ),
    };
    let syntax_checks: Bound<'py, PyAny> =
        model_syntax_checks(&model_inputs.clone(), &names.clone(), &succeeded)
            .and_then(|value| value.into_bound_py_any(py))?;
    let resources: Bound<'py, PyAny> = assemble_native_project_resources(
        &inputs.clone(),
        dialect.clone().extract()?,
        &syntax_checks,
    )
    .and_then(|value| value.into_bound_py_any(py))?;
    let proofs = pivot_proofs(&model_inputs, &positions, context, session)?;
    let deps: Bound<'py, PyAny> = native_field(&resources, "model_deps")?;
    let syntax_valid: Bound<'py, PyAny> = native_field(&resources, "model_syntax_valid")?;
    let mut models: Vec<Bound<'py, PyAny>> = Vec::new();
    for index in 0..model_inputs.len()? {
        let proof: Bound<'py, PyAny> = match proofs.get_item(index)? {
            Some(proof) => proof,
            None => py.None().into_bound(py),
        };
        models.push(
            Bound::new(
                py,
                NativeModelFacts {
                    deps: deps.get_item(index)?.unbind(),
                    dynamic_contract: proof.unbind(),
                    syntax_valid: syntax_valid.get_item(index)?.unbind(),
                },
            )?
            .into_any(),
        );
    }
    Ok(Bound::new(
        py,
        NativeProjectFacts {
            models: PyTuple::new(py, models)?.into_any().unbind(),
            resources: resources.unbind(),
        },
    )?
    .into_any())
}

/// The declared relation facts standalone pivot proofs read.
pub(crate) fn pivot_tables<'py>(context: &AnalysisContext<'py>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = context.profile.py();
    let families = PyDict::new(py);
    let class = record_class(py, RecordId::SchemaDynamicColumnFamily)?;
    for (name, specs) in &context.dynamic_families {
        families.set_item(
            name,
            HeaderFamilies::new(&class, specs.clone())?.project(py)?,
        )?;
    }
    new_record_with(
        py,
        RecordId::NativePivotTables,
        &[
            (
                "dialect",
                native_field(&context.profile, "sql_analysis_dialect")?,
            ),
            ("column_types_by_table", context.column_types.clone()),
            (
                "authoritative_column_types_by_table",
                context.complete_schemas.clone(),
            ),
            ("column_nullability_by_table", context.nullability.clone()),
            ("dynamic_families_by_table", families.into_any()),
        ],
    )
}

/// Each standalone pivot model's dynamic contract proof, by model position.
fn pivot_proofs<'py>(
    model_inputs: &Bound<'py, PyAny>,
    positions: &Bound<'py, PyAny>,
    context: &AnalysisContext<'py>,
    session: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py = model_inputs.py();
    let pivots = standalone_pivot_rows(model_inputs, positions)?;
    let indexes: Vec<_> = pivots.iter().map(|(index, _)| *index).collect();
    let models = pivots
        .into_iter()
        .map(|(_, (sql, families))| (sql, families.into_iter().map(FamilyRow::from).collect()))
        .collect::<Vec<_>>();
    let contracts = if models.is_empty() {
        Vec::new()
    } else if session.is_none() {
        prove_dynamic_column_contracts(
            py,
            PivotRequestRow {
                dialect: dialect_or_generic(&native_field(
                    &context.profile,
                    "sql_analysis_dialect",
                )?)?,
                column_types: context.declared.types.rows().clone(),
                authoritative_types: context.declared.complete.rows().clone(),
                column_nullability: context.declared.nullability.rows().clone(),
                families_by_table: context
                    .dynamic_families
                    .iter()
                    .map(|(name, families)| {
                        (
                            name.clone(),
                            families.iter().cloned().map(FamilyRow::from).collect(),
                        )
                    })
                    .collect(),
                models,
            },
        )?
    } else {
        session
            .cast::<NativeModelAnalysisSession>()?
            .borrow()
            .prove_dynamic_contracts(py, models)?
    };
    let proven = contracts
        .into_iter()
        .map(|(_, proof)| proof.into_bound_py_any(py))
        .collect::<PyResult<Vec<_>>>()?;
    count_stage(py, PIVOT_PROOFS, proven.len() as u64)?;
    if !session.is_none() {
        let session_proofs: usize = proven.iter().filter(|proof| !proof.is_none()).count();
        count_stage(py, SESSION_PIVOT_PROOFS, session_proofs as u64)?;
    }
    if proven.len() != indexes.len() {
        return Err(PyValueError::new_err(
            "native pivot proofs do not match the standalone pivot models",
        ));
    }
    let proofs: Bound<'py, PyDict> = PyDict::new(py);
    for (index, proof) in indexes.into_iter().zip(proven) {
        proofs.set_item(index, proof)?;
    }
    Ok(proofs)
}
