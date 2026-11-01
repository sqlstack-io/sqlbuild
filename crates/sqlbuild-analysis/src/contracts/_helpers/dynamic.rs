//! Python's `_dynamic_column_diagnostics`: proof of a dynamic column contract and its families.

use std::collections::HashMap;

use crate::contracts::_helpers::type_comparison::{TypeComparer, TypeComparison};
use crate::contracts::constants::{
    DYNAMIC_FAMILY_TYPE_MISMATCH_HELP, DYNAMIC_FAMILY_UNKNOWN_TYPE_HELP,
    DYNAMIC_OUTPUT_NOT_PROVEN_CODE, DYNAMIC_OUTPUT_NOT_PROVEN_HELP, NO_COMPILER_EVIDENCE,
    TYPE_MISMATCH_CODE, UNKNOWN_TYPE_CODE,
};
use crate::contracts::models::{
    ContractDiagnostic, ContractLocation, ContractModel, ContractSeverity, DynamicContractProof,
};
use crate::type_system::errors::TypeNormalizationError;

/// The dynamic column contract diagnostics of one model, before its fixed columns.
pub(crate) fn dynamic_column_diagnostics(
    model: &ContractModel,
    types: &TypeComparer<'_>,
) -> Result<Vec<ContractDiagnostic>, TypeNormalizationError> {
    let Some(schema) = model
        .schema
        .as_ref()
        .filter(|schema| !schema.dynamic_columns.is_empty())
    else {
        return Ok(Vec::new());
    };
    let Some(proof) = model
        .dynamic_proof
        .as_ref()
        .filter(|proof| proof.output_proven)
    else {
        return Ok(vec![not_proven(model)]);
    };
    let proof_by_name: HashMap<String, Option<&str>> = families_by_casefold(proof)?;
    let mut diagnostics: Vec<ContractDiagnostic> = Vec::new();
    for family in &schema.dynamic_columns {
        let key: String = casefold(&family.name);
        let declared: &str = &family.declared_type;
        let Some(inferred) = proof_by_name.get(&key).copied().flatten() else {
            diagnostics.push(family_diagnostic(
                UNKNOWN_TYPE_CODE,
                ContractSeverity::Warning,
                format!(
                    "dynamic column family '{}' type could not be proven against declared {declared}",
                    family.name
                ),
                DYNAMIC_FAMILY_UNKNOWN_TYPE_HELP,
            ));
            continue;
        };
        if types.declared_accepts(declared, inferred)? == TypeComparison::Different {
            diagnostics.push(family_diagnostic(
                TYPE_MISMATCH_CODE,
                ContractSeverity::Error,
                format!(
                    "dynamic column family '{}' inferred as {inferred} but declared type is {declared}",
                    family.name
                ),
                DYNAMIC_FAMILY_TYPE_MISMATCH_HELP,
            ));
        }
    }
    Ok(diagnostics)
}

fn not_proven(model: &ContractModel) -> ContractDiagnostic {
    let reason: &str = model
        .dynamic_proof
        .as_ref()
        .and_then(|proof| proof.failure_reason.as_deref())
        .filter(|reason| !reason.is_empty())
        .unwrap_or(NO_COMPILER_EVIDENCE);
    ContractDiagnostic {
        code: DYNAMIC_OUTPUT_NOT_PROVEN_CODE,
        severity: ContractSeverity::Error,
        message: format!(
            "model '{}' dynamic column contract is not proven: {reason}",
            model.name
        ),
        column_name: None,
        location: ContractLocation::None,
        related_output: None,
        help: DYNAMIC_OUTPUT_NOT_PROVEN_HELP.to_owned(),
    }
}

fn families_by_casefold(
    proof: &DynamicContractProof,
) -> Result<HashMap<String, Option<&str>>, TypeNormalizationError> {
    let mut by_name: HashMap<String, Option<&str>> = HashMap::new();
    for (name, inferred_type) in &proof.families {
        let _ = by_name.insert(casefold(name), inferred_type.as_deref());
    }
    Ok(by_name)
}

/// Python's `str.casefold`.
fn casefold(name: &str) -> String {
    str::to_lowercase(name)
}

fn family_diagnostic(
    code: &'static str,
    severity: ContractSeverity,
    message: String,
    help: &str,
) -> ContractDiagnostic {
    ContractDiagnostic {
        code,
        severity,
        message,
        column_name: None,
        location: ContractLocation::None,
        related_output: None,
        help: help.to_owned(),
    }
}
