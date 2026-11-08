//! One model's outcome from its dialect's native analysis module, in the session's facts.

use std::collections::HashMap;

use sqlbuild_sqltext::compiler::main::placeholder_defaults::placeholder_defaults;
use sqlquack_semantics::lineage::models::{Nullability, SchemaColumn};

use crate::analysis_route::models::{AnalysisInput, NativeAnalysis, ParserGap};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::analysis_session::_helpers::mappings::ShapeTable;
use crate::assembly::analysis_session::constants::UNKNOWN_TYPE;
use crate::assembly::analysis_session::models::{
    LineageFacts, ModelAnalysis, ModelOutcome, ModelRequest, SessionSchema,
};
use crate::assembly::analysis_session::types::{Pairs, Shapes};
use crate::semantic_validation::main::normalize_native::normalize_native_sql;

/// The model's outcome read against `schema`, its complete input shapes, and why it failed if it did.
pub(crate) fn native_outcome(
    dialect: &dyn NativeDialect,
    model: &ModelRequest,
    (schema, nullability): (Shapes, &ShapeTable),
) -> (ModelOutcome, Option<String>) {
    let normalised: Result<String, String> =
        normalize_native_sql(&model.query_sql, dialect.name(), dialect.stub_prefix());
    let (cleaned_sql, analysed): (String, Result<NativeAnalysis, String>) = match normalised {
        Ok(normalised) => {
            let sql: String = placeholder_defaults(&normalised, &model.placeholders);
            let relations: SessionSchema = session_schema(dialect, &schema, nullability);
            let analysed = dialect
                .analyse(&AnalysisInput {
                    sql: &sql,
                    schema: &relations,
                    references: &model.lineage_references,
                    extension_functions: &[],
                })
                .map_err(|gap: ParserGap| gap.message);
            (sql, analysed)
        }
        Err(reason) => (String::new(), Err(reason)),
    };
    let failure: Option<String> = analysed
        .as_ref()
        .err()
        .map(|reason| format!("{}: {reason}", model.name));
    let analysis: ModelAnalysis = match analysed {
        Ok(native) => ModelAnalysis {
            analysis_succeeded: true,
            columns: Some(native.columns),
            lineage: LineageFacts::Native(native.lineage),
            has_star: native.has_star,
            star_resolved: native.star_resolved,
            binding_diagnostics: native.diagnostics,
            binding_validated: true,
        },
        Err(_) => ModelAnalysis {
            analysis_succeeded: false,
            columns: None,
            lineage: LineageFacts::Native(Vec::new()),
            has_star: false,
            star_resolved: false,
            binding_diagnostics: Vec::new(),
            binding_validated: true,
        },
    };
    let outcome: ModelOutcome = ModelOutcome {
        analysis,
        cleaned_sql,
        deferred_cleaning: None,
        validated_schema: schema,
        fused_binding_validated: true,
        sql_macro_findings: Vec::new(),
    };
    (outcome, failure)
}

/// Each relation with a known shape, its columns typed and with the nullability the session knows.
fn session_schema(
    dialect: &dyn NativeDialect,
    schema: &Shapes,
    nullability: &ShapeTable,
) -> SessionSchema {
    let relations: HashMap<String, Vec<SchemaColumn>> = schema
        .iter()
        .filter(|(_, columns)| !columns.is_empty())
        .map(|(relation, columns)| {
            let known: Option<&Pairs> = nullability.get(relation);
            let typed: Vec<SchemaColumn> = columns
                .iter()
                .map(|(name, data_type)| SchemaColumn {
                    name: name.clone(),
                    quoted: false,
                    data_type: (!data_type.is_empty() && data_type != UNKNOWN_TYPE)
                        .then(|| dialect.session_type(data_type)),
                    nullability: column_nullability(known, name),
                })
                .collect();
            (relation.clone(), typed)
        })
        .collect();
    SessionSchema { relations }
}

fn column_nullability(known: Option<&Pairs>, column: &str) -> Nullability {
    let spelled: Option<&str> = known.and_then(|pairs| {
        pairs
            .iter()
            .find(|(name, _)| name == column)
            .map(|(_, value)| value.as_str())
    });
    match spelled {
        Some("non_null") => Nullability::NonNull,
        Some("nullable") => Nullability::Nullable,
        _ => Nullability::Unknown,
    }
}
