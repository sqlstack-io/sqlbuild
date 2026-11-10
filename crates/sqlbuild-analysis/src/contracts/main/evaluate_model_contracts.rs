//! Python's `evaluate_model_contracts`: every model's column contract diagnostics, in order.

use crate::contracts::_helpers::columns::{
    declared_shape_validation_is_active, model_contract_diagnostics, requires_contract_evaluation,
};
use crate::contracts::_helpers::type_comparison::TypeComparer;
use crate::contracts::models::{ContractDiagnostic, ContractRequest};
use crate::type_system::errors::TypeNormalizationError;

/// Each requested model's diagnostics; models that need no evaluation have none.
pub fn evaluate_model_contracts(
    request: &ContractRequest,
) -> Result<Vec<Vec<ContractDiagnostic>>, TypeNormalizationError> {
    let implicit: bool = request.implicit_column_contracts;
    let types = TypeComparer::new(&request.dialect);
    request
        .models
        .iter()
        .map(|model| {
            if !requires_contract_evaluation(model, implicit) {
                return Ok(Vec::new());
            }
            model_contract_diagnostics(
                model,
                (
                    declared_shape_validation_is_active(model, implicit),
                    request.staged_promotion,
                ),
                &types,
            )
        })
        .collect()
}
