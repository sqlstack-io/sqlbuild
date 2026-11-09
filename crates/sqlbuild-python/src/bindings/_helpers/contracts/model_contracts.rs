//! Model column contracts and promotion conflicts.

use pyo3::prelude::{PyResult, Python};
use sqlbuild_analysis::compiled_project::main::project_models::project_models;
use sqlbuild_analysis::compiled_project::models::{
    CompiledModelFacts, DynamicProofFact, ModelAnalysisFact, ModelSchemaFact,
};
use sqlbuild_analysis::contracts::main::evaluate_model_contracts::evaluate_model_contracts;
use sqlbuild_analysis::contracts::main::promotion_conflicts::promotion_conflicts;
use sqlbuild_analysis::contracts::models::{
    ContractDiagnostic, ContractModel, ContractRequest, ContractSchema, ContractSeverity,
    DeclaredColumn, DeclaredColumnFamily, DynamicContractProof, InferredOutputColumn,
    PromotionConflict, PromotionModel, PromotionRequest,
};

use sqlbuild_analysis::sql_macros::main::untyped_call_outputs::untyped_call_outputs;
use sqlbuild_analysis::type_system::errors::TypeNormalizationError;

use crate::bindings::_helpers::boundary::panics::compiler_error;
use crate::bindings::_helpers::compiled_project::project::NativeCompiledProject;
use crate::bindings::_helpers::type_system::normalization::normalization_error;
use crate::bindings::types::CompilerDetach;

/// One contract diagnostic: its code, severity, message, column, location, related output and help.
#[derive(Clone)]
pub(crate) struct DiagnosticRow {
    pub(crate) code: &'static str,
    pub(crate) is_error: bool,
    pub(crate) message: String,
    pub(crate) column_name: Option<String>,
    pub(crate) declared_index: Option<usize>,
    pub(crate) output_column: Option<String>,
    pub(crate) related: Option<(String, String)>,
    pub(crate) help: String,
}

/// Evaluate the column contract of every model the compile's owner recorded, in its order.
pub(crate) fn evaluate_project_model_contracts(
    py: Python<'_>,
    settings: (String, bool, bool),
    project: &NativeCompiledProject,
) -> PyResult<Vec<(String, Vec<DiagnosticRow>)>> {
    let (dialect, implicit_column_contracts, staged_promotion) = settings;
    let models: &[CompiledModelFacts] = project_models(project.facts());
    let request = ContractRequest {
        dialect,
        implicit_column_contracts,
        staged_promotion,
        models: models.iter().map(fact_contract_model).collect(),
    };
    let outcomes: Result<Vec<Vec<ContractDiagnostic>>, TypeNormalizationError> = py
        .compiler_detach(|| Ok(evaluate_model_contracts(&request)))
        .map_err(compiler_error)?;
    let outcomes: Vec<Vec<ContractDiagnostic>> =
        outcomes.map_err(|error| normalization_error(py, &error))?;
    Ok(models
        .iter()
        .zip(outcomes)
        .map(|(model, diagnostics)| {
            (
                model.name.clone(),
                diagnostics.into_iter().map(diagnostic_row).collect(),
            )
        })
        .collect())
}

/// One model's contract inputs, read from its retained facts.
fn fact_contract_model(model: &CompiledModelFacts) -> ContractModel {
    let analysis: Option<&ModelAnalysisFact> = model.analysis.as_ref();
    ContractModel {
        name: model.name.clone(),
        contract: model.config.contract.clone(),
        materialized: model.config.materialized.clone(),
        incremental_mode: model.config.incremental_mode.clone(),
        schema: model.schema.as_ref().map(fact_contract_schema),
        inferred_columns: analysis
            .filter(|analysis| analysis.inferred)
            .map(fact_inferred_columns),
        fast_lineage_has_star: analysis.is_some_and(|analysis| analysis.fast_lineage_has_star),
        dynamic_proof: analysis
            .and_then(|analysis| analysis.dynamic_proof.as_ref())
            .map(fact_dynamic_proof),
        unchecked_output_columns: analysis
            .map(|analysis| analysis.unchecked_output_columns.clone())
            .unwrap_or_default(),
        untyped_call_outputs: untyped_call_outputs(&model.query_sql, &model.typed_calls),
    }
}

fn fact_contract_schema(schema: &ModelSchemaFact) -> ContractSchema {
    let mut columns: Vec<DeclaredColumn> = Vec::with_capacity(schema.columns.len());
    for column in &schema.columns {
        columns.push(DeclaredColumn {
            name: column.name.clone(),
            declared_type: column.data_type.clone(),
            not_null: column.nullable == Some(false),
            declared_in_named_schema: schema.named_schema && column.location_path.is_some(),
        });
    }
    let mut dynamic_columns: Vec<DeclaredColumnFamily> =
        Vec::with_capacity(schema.dynamic_columns.len());
    for family in &schema.dynamic_columns {
        dynamic_columns.push(DeclaredColumnFamily {
            name: family.name.clone(),
            declared_type: family.data_type.clone(),
        });
    }
    ContractSchema {
        columns,
        dynamic_columns,
        type_enforcement: schema.type_enforcement,
        named_schema: schema.named_schema,
    }
}

fn fact_inferred_columns(analysis: &ModelAnalysisFact) -> Vec<InferredOutputColumn> {
    let mut columns: Vec<InferredOutputColumn> =
        Vec::with_capacity(analysis.inferred_columns.len());
    for (index, (name, inferred_type)) in analysis.inferred_columns.iter().enumerate() {
        columns.push(InferredOutputColumn {
            name: name.clone(),
            inferred_type: inferred_type.clone(),
            nullable: analysis
                .nullable_columns
                .get(index)
                .copied()
                .unwrap_or(false),
        });
    }
    columns
}

fn fact_dynamic_proof(proof: &DynamicProofFact) -> DynamicContractProof {
    DynamicContractProof {
        output_proven: proof.output_proven,
        failure_reason: proof.failure_reason.clone(),
        families: proof.named_families.clone(),
    }
}

/// One K011 conflict: the model name and path, and the code, message and help.
pub(crate) struct ConflictRow {
    pub(crate) name: String,
    pub(crate) relative_path: String,
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) help: String,
}

/// The K011 conflicts of enforced contracts under immediate table promotion, by model name.
pub(crate) fn project_promotion_conflicts(
    py: Python<'_>,
    settings: (Option<String>, String, String),
    project: &NativeCompiledProject,
) -> PyResult<Vec<ConflictRow>> {
    let (explicit_mode, adapter_default, settings_file) = settings;
    let models: &[CompiledModelFacts] = project_models(project.facts());
    let mut rows: Vec<PromotionModel> = Vec::with_capacity(models.len());
    for model in models {
        rows.push(PromotionModel {
            name: model.name.clone(),
            contract: model.config.contract.clone(),
            materialized: model.config.materialized.clone(),
            incremental_mode: model.config.incremental_mode.clone(),
        });
    }
    let request = PromotionRequest {
        explicit_mode,
        adapter_default,
        settings_file,
        models: rows,
    };
    let conflicts: Vec<PromotionConflict> = py
        .compiler_detach(|| Ok(promotion_conflicts(&request)))
        .map_err(compiler_error)?;
    Ok(conflicts
        .into_iter()
        .map(|conflict| {
            let model: &CompiledModelFacts = &models[conflict.model_index];
            ConflictRow {
                name: model.name.clone(),
                relative_path: model.relative_path.clone(),
                code: conflict.code,
                message: conflict.message,
                help: conflict.help,
            }
        })
        .collect())
}

fn diagnostic_row(diagnostic: ContractDiagnostic) -> DiagnosticRow {
    let (declared_index, output_column) = diagnostic.location.into_parts();
    DiagnosticRow {
        code: diagnostic.code,
        is_error: diagnostic.severity == ContractSeverity::Error,
        message: diagnostic.message,
        column_name: diagnostic.column_name,
        declared_index,
        output_column,
        related: diagnostic
            .related_output
            .map(|related| (related.column_name, related.message)),
        help: diagnostic.help,
    }
}
