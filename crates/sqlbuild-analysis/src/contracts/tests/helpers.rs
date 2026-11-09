use crate::contracts::models::{
    ContractDiagnostic, ContractModel, ContractSchema, DeclaredColumn, DeclaredColumnFamily,
    DynamicContractProof, EnumColumn, EnumLiteralViolation, InferredOutputColumn, PromotionModel,
    PromotionRequest,
};

pub(crate) fn model(contract: Option<&str>, schema: Option<ContractSchema>) -> ContractModel {
    ContractModel {
        name: "orders".to_owned(),
        contract: contract.map(str::to_owned),
        materialized: Some("table".to_owned()),
        incremental_mode: None,
        schema,
        inferred_columns: None,
        fast_lineage_has_star: false,
        dynamic_proof: None,
        unchecked_output_columns: Vec::new(),
        untyped_call_outputs: Vec::new(),
    }
}

pub(crate) fn schema(columns: Vec<DeclaredColumn>, type_enforcement: bool) -> ContractSchema {
    ContractSchema {
        columns,
        dynamic_columns: Vec::new(),
        type_enforcement,
        named_schema: false,
    }
}

pub(crate) fn declared(name: &str, declared_type: Option<&str>, not_null: bool) -> DeclaredColumn {
    DeclaredColumn {
        name: name.to_owned(),
        declared_type: declared_type.map(str::to_owned),
        not_null,
        declared_in_named_schema: false,
    }
}

pub(crate) fn inferred(
    name: &str,
    inferred_type: Option<&str>,
    nullable: bool,
) -> InferredOutputColumn {
    InferredOutputColumn {
        name: name.to_owned(),
        inferred_type: inferred_type.map(str::to_owned),
        nullable,
    }
}

pub(crate) fn with_inferred(
    mut model: ContractModel,
    columns: Vec<InferredOutputColumn>,
) -> ContractModel {
    model.inferred_columns = Some(columns);
    model
}

pub(crate) fn dynamic_model(
    families: &[(&str, &str)],
    proof: Option<DynamicContractProof>,
) -> ContractModel {
    let mut declared_schema: ContractSchema = schema(Vec::new(), false);
    declared_schema.dynamic_columns = families
        .iter()
        .map(|(name, declared_type)| DeclaredColumnFamily {
            name: (*name).to_owned(),
            declared_type: (*declared_type).to_owned(),
        })
        .collect();
    let mut dynamic: ContractModel = model(Some("enforced"), Some(declared_schema));
    dynamic.dynamic_proof = proof;
    dynamic
}

pub(crate) fn proof(
    output_proven: bool,
    failure_reason: Option<&str>,
    families: &[(&str, Option<&str>)],
) -> DynamicContractProof {
    DynamicContractProof {
        output_proven,
        failure_reason: failure_reason.map(str::to_owned),
        families: families
            .iter()
            .map(|(name, inferred_type)| ((*name).to_owned(), inferred_type.map(str::to_owned)))
            .collect(),
    }
}

pub(crate) fn promotion_model(
    contract: Option<&str>,
    materialized: Option<&str>,
    incremental_mode: Option<&str>,
) -> PromotionModel {
    PromotionModel {
        name: "orders".to_owned(),
        contract: contract.map(str::to_owned),
        materialized: materialized.map(str::to_owned),
        incremental_mode: incremental_mode.map(str::to_owned),
    }
}

pub(crate) fn outcome_lines(diagnostics: &[ContractDiagnostic]) -> Vec<String> {
    diagnostics.iter().map(diagnostic_line).collect()
}

fn diagnostic_line(diagnostic: &ContractDiagnostic) -> String {
    let (declared, output) = diagnostic.location.clone().into_parts();
    let location: String = declared
        .map(|index| format!("declared:{index}"))
        .or_else(|| output.map(|column| format!("output:{column}")))
        .unwrap_or_else(|| "-".to_owned());
    let related: String = diagnostic
        .related_output
        .as_ref()
        .map_or(String::new(), |related| {
            format!(" [{}: {}]", related.column_name, related.message)
        });
    format!(
        "{} {} {location} {}{related}",
        diagnostic.code,
        diagnostic.severity.as_str(),
        diagnostic.message
    )
}

pub(crate) fn promotion_request(
    explicit_mode: Option<&str>,
    adapter_default: &str,
    settings_file: &str,
) -> PromotionRequest {
    PromotionRequest {
        explicit_mode: explicit_mode.map(str::to_owned),
        adapter_default: adapter_default.to_owned(),
        settings_file: settings_file.to_owned(),
        models: vec![
            promotion_model(Some("enforced"), Some("table"), None),
            promotion_model(Some("enforced"), Some("view"), None),
            promotion_model(Some("enforced"), Some("incremental"), Some("microbatch")),
            promotion_model(None, Some("table"), None),
            promotion_model(Some("enforced"), Some("incremental"), Some("merge")),
            promotion_model(Some("enforced"), None, None),
        ],
    }
}

/// The header of every authored model the enum literal tests check.
pub(crate) const ENUM_HEADER: &str = "MODEL (description 'Orders');\n";

/// An order status column of a two-member string enum.
pub(crate) fn status_column() -> EnumColumn {
    EnumColumn {
        name: "status".to_owned(),
        enum_name: "order_status".to_owned(),
        members: vec!["open".to_owned(), "closed".to_owned()],
    }
}

/// One violation as `column value line:column`, `-` when it has no location.
pub(crate) fn enum_violation_line(violation: &EnumLiteralViolation) -> String {
    let location: String = violation.location.map_or_else(
        || "-".to_owned(),
        |location| format!("{}:{}", location.line, location.column),
    );
    format!("{} {} {location}", violation.column_name, violation.value)
}
