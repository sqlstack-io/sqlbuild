//! Python's `collect_model_column_contract_diagnostics`, in its exact emission order.

use std::collections::{HashMap, HashSet};

use crate::contracts::_helpers::dynamic::dynamic_column_diagnostics;
use crate::contracts::_helpers::type_comparison::{TypeComparer, TypeComparison};
use crate::contracts::constants::{
    CONTRACT_ENFORCED, CONTRACT_NONE, DYNAMIC_OUTPUT_NOT_PROVEN_CODE, EXTRA_COLUMN_CODE,
    EXTRA_COLUMN_HELP, EXTRA_COLUMN_NAMED_SCHEMA_HELP, MISSING_COLUMN_CODE,
    MISSING_DECLARATIONS_CODE, MISSING_DECLARATIONS_HELP, MODEL_COLUMNS_DECLARATION,
    NAMED_SCHEMA_DECLARATION, NULLABILITY_HELP, NULLABILITY_MISMATCH_CODE, NULLABLE_OUTPUT_MESSAGE,
    TYPE_MISMATCH_CODE, TYPE_MISMATCH_HELP, UNKNOWN_TYPE_CODE, UNKNOWN_TYPE_HELP,
    UNPROVEN_TYPE_OUTPUT_MESSAGE,
};
use crate::contracts::models::{
    ContractDiagnostic, ContractLocation, ContractModel, ContractSchema, ContractSeverity,
    DeclaredColumn, InferredOutputColumn, RelatedOutput,
};
use crate::type_system::errors::TypeNormalizationError;
use sqlbuild_sqltext::resolution::main::unquoted_column_name::unquoted_column_name as output_column_name;

/// Python's `_requires_contract_evaluation`.
pub(crate) fn requires_contract_evaluation(model: &ContractModel, implicit: bool) -> bool {
    declared_shape_validation_is_active(model, implicit)
        || model
            .schema
            .as_ref()
            .is_some_and(|schema| schema.type_enforcement)
}

/// Python's `_declared_shape_validation_is_active`.
pub(crate) fn declared_shape_validation_is_active(model: &ContractModel, implicit: bool) -> bool {
    match model.contract.as_deref() {
        Some(CONTRACT_ENFORCED) => true,
        Some(CONTRACT_NONE) => false,
        _ => {
            implicit
                && model
                    .schema
                    .as_ref()
                    .is_some_and(|schema| !schema.columns.is_empty())
        }
    }
}

/// One model's contract diagnostics, or the reason Python must evaluate it.
pub(crate) fn model_contract_diagnostics(
    model: &ContractModel,
    validate_declared_shape: bool,
    types: &TypeComparer<'_>,
) -> Result<Vec<ContractDiagnostic>, TypeNormalizationError> {
    let contract_enforced: bool = model.contract.as_deref() == Some(CONTRACT_ENFORCED);
    let Some(schema) = model
        .schema
        .as_ref()
        .filter(|schema| !schema.columns.is_empty() || !schema.dynamic_columns.is_empty())
    else {
        return Ok(if contract_enforced {
            vec![missing_declarations(model)]
        } else {
            Vec::new()
        });
    };
    let mut diagnostics: Vec<ContractDiagnostic> = dynamic_column_diagnostics(model, types)?;
    let dynamic_output_unproven: bool = diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DYNAMIC_OUTPUT_NOT_PROVEN_CODE);
    let Some(inferred_columns) = model.inferred_columns.as_ref() else {
        return Ok(diagnostics);
    };
    let inferred_by_name: HashMap<&str, &InferredOutputColumn> = inferred_columns
        .iter()
        .map(|column| (column.name.as_str(), column))
        .collect();
    if contract_enforced {
        diagnostics.extend(extra_columns(
            model,
            schema,
            inferred_columns,
            types.folds_upper(),
        ));
    }
    let unchecked: HashSet<&str> = model
        .unchecked_output_columns
        .iter()
        .map(String::as_str)
        .collect();
    for (index, declared) in schema.columns.iter().enumerate() {
        let output: &str = output_column_name(&declared.name);
        let Some(inferred) = inferred_by_name
            .get(declared.name.as_str())
            .or_else(|| inferred_by_name.get(output))
            .or_else(|| {
                types
                    .folds_upper()
                    .then(|| inferred_by_name.get(declared.name.to_uppercase().as_str()))
                    .flatten()
            })
        else {
            if validate_declared_shape && !model.fast_lineage_has_star {
                diagnostics.push(missing_column(model, index, declared, contract_enforced));
            }
            continue;
        };
        if validate_declared_shape {
            diagnostics.extend(nullability(index, declared, inferred));
        }
        let Some(declared_type) = declared.declared_type.as_deref() else {
            continue;
        };
        if !validate_declared_shape && !schema.type_enforcement {
            continue;
        }
        if dynamic_output_unproven && inferred.inferred_type.is_none() {
            continue;
        }
        let checked = TypedColumn {
            index,
            declared,
            declared_type,
            inferred,
        };
        let check = TypeCheck {
            type_enforcement: schema.type_enforcement,
            contract_enforced,
            unchecked: &unchecked,
            types,
            untyped_calls: &model.untyped_call_outputs,
        };
        diagnostics.extend(column_type(&checked, &check)?);
    }
    Ok(diagnostics)
}

struct TypeCheck<'a> {
    type_enforcement: bool,
    contract_enforced: bool,
    unchecked: &'a HashSet<&'a str>,
    types: &'a TypeComparer<'a>,
    /// Output columns that are one call to a SQL macro without `returns`, and the macro.
    untyped_calls: &'a [(String, String)],
}

struct TypedColumn<'a> {
    index: usize,
    declared: &'a DeclaredColumn,
    declared_type: &'a str,
    inferred: &'a InferredOutputColumn,
}

fn missing_declarations(model: &ContractModel) -> ContractDiagnostic {
    ContractDiagnostic {
        code: MISSING_DECLARATIONS_CODE,
        severity: ContractSeverity::Error,
        message: format!(
            "model '{}' has contract enforced but declares no columns",
            model.name
        ),
        column_name: None,
        location: ContractLocation::None,
        related_output: None,
        help: MISSING_DECLARATIONS_HELP.to_owned(),
    }
}

/// Inferred columns no declared column names; a folded `order_id` also names a star's `ORDER_ID`.
fn extra_columns(
    model: &ContractModel,
    schema: &ContractSchema,
    inferred_columns: &[InferredOutputColumn],
    folds_upper: bool,
) -> Vec<ContractDiagnostic> {
    let declared_names: HashSet<String> = schema
        .columns
        .iter()
        .flat_map(|column| {
            let output: &str = output_column_name(&column.name);
            let folded: Option<String> = folds_upper.then(|| column.name.to_uppercase());
            std::iter::once(column.name.clone())
                .chain(std::iter::once(output.to_owned()))
                .chain(folded)
        })
        .collect();
    let help: &str = if schema.named_schema {
        EXTRA_COLUMN_NAMED_SCHEMA_HELP
    } else {
        EXTRA_COLUMN_HELP
    };
    inferred_columns
        .iter()
        .filter(|column| !declared_names.contains(column.name.as_str()))
        .map(|column| ContractDiagnostic {
            code: EXTRA_COLUMN_CODE,
            severity: ContractSeverity::Error,
            message: format!(
                "column '{}' is not declared in enforced contract for model '{}'",
                column.name, model.name
            ),
            column_name: Some(column.name.clone()),
            location: ContractLocation::Output(column.name.clone()),
            related_output: None,
            help: help.to_owned(),
        })
        .collect()
}

fn nullability(
    index: usize,
    declared: &DeclaredColumn,
    inferred: &InferredOutputColumn,
) -> Option<ContractDiagnostic> {
    if !declared.not_null || !inferred.nullable {
        return None;
    }
    Some(ContractDiagnostic {
        code: NULLABILITY_MISMATCH_CODE,
        severity: ContractSeverity::Error,
        message: format!(
            "column '{}' is declared non-null but may be nullable",
            declared.name
        ),
        column_name: Some(declared.name.clone()),
        location: ContractLocation::Declared(index),
        related_output: Some(RelatedOutput {
            column_name: declared.name.clone(),
            message: NULLABLE_OUTPUT_MESSAGE.to_owned(),
        }),
        help: NULLABILITY_HELP.to_owned(),
    })
}

fn missing_column(
    model: &ContractModel,
    index: usize,
    column: &DeclaredColumn,
    contract_enforced: bool,
) -> ContractDiagnostic {
    let contract_label: &str = if contract_enforced {
        "enforced contract column"
    } else {
        "declared column"
    };
    let declaration_label: &str = if column.declared_in_named_schema {
        NAMED_SCHEMA_DECLARATION
    } else {
        MODEL_COLUMNS_DECLARATION
    };
    let name: &str = &column.name;
    let help: String = if contract_enforced {
        format!(
            "add {name} to the SELECT list or correct {declaration_label}; validation is active \
             because this model declares contract enforced"
        )
    } else {
        format!(
            "add {name} to the SELECT list or correct/remove {declaration_label}; \
             {declaration_label} is validated using static SQL analysis because \
             settings.column_contract_mode is \"implicit\" (the default). If this project \
             intentionally uses columns only for metadata and audits, set [settings] \
             column_contract_mode = \"explicit\"; models with contract enforced remain validated"
        )
    };
    ContractDiagnostic {
        code: MISSING_COLUMN_CODE,
        severity: ContractSeverity::Error,
        message: format!(
            "{contract_label} '{name}' was not found in statically inferred output for model '{}'",
            model.name
        ),
        column_name: Some(name.to_owned()),
        location: ContractLocation::Declared(index),
        related_output: None,
        help,
    }
}

fn column_type(
    column: &TypedColumn<'_>,
    check: &TypeCheck<'_>,
) -> Result<Option<ContractDiagnostic>, TypeNormalizationError> {
    let name: &str = &column.declared.name;
    let declared_type: &str = column.declared_type;
    let Some(inferred_type) = column.inferred.inferred_type.as_deref() else {
        if !check.type_enforcement || check.unchecked.contains(column.inferred.name.as_str()) {
            return Ok(None);
        }
        return Ok(Some(ContractDiagnostic {
            code: UNKNOWN_TYPE_CODE,
            severity: ContractSeverity::Warning,
            message: format!(
                "column '{name}' type could not be proven against declared {declared_type}"
            ),
            column_name: Some(name.to_owned()),
            location: ContractLocation::Declared(column.index),
            related_output: Some(RelatedOutput {
                column_name: name.to_owned(),
                message: UNPROVEN_TYPE_OUTPUT_MESSAGE.to_owned(),
            }),
            help: match check
                .untyped_calls
                .iter()
                .find(|(output, _)| output.eq_ignore_ascii_case(name))
            {
                Some((_, macro_name)) => format!(
                    "declare returns {declared_type} in SQL macro {macro_name}(), or add an \
                     explicit CAST if this declared type should be checked statically"
                ),
                None => UNKNOWN_TYPE_HELP.to_owned(),
            },
        }));
    };
    if check.types.types_equal(declared_type, inferred_type)? == TypeComparison::Equal {
        return Ok(None);
    }
    Ok(Some(ContractDiagnostic {
        code: TYPE_MISMATCH_CODE,
        severity: if check.type_enforcement || check.contract_enforced {
            ContractSeverity::Error
        } else {
            ContractSeverity::Warning
        },
        message: format!(
            "column '{name}' inferred as {inferred_type} but declared type is {declared_type}"
        ),
        column_name: Some(name.to_owned()),
        location: ContractLocation::Declared(column.index),
        related_output: Some(RelatedOutput {
            column_name: name.to_owned(),
            message: format!("inferred {inferred_type}"),
        }),
        help: TYPE_MISMATCH_HELP.to_owned(),
    }))
}
