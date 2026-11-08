//! One model's outcome from its dialect's native analysis module, in the session's facts.

use sqlbuild_sqltext::compiler::main::placeholder_defaults::placeholder_defaults;

use crate::analysis_route::models::{AnalysisInput, NativeAnalysis, ParserGap};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::analysis_session::models::{
    LineageFacts, ModelAnalysis, ModelOutcome, ModelRequest, SessionSchema,
};
use crate::assembly::analysis_session::types::Shapes;
use crate::semantic_validation::main::normalize_native::normalize_native_sql;

/// The model's outcome over `schema` with the adapter's declared functions, and why it failed.
pub(crate) fn model_outcome(
    dialect: &dyn NativeDialect,
    model: &ModelRequest,
    (schema, relations): (Shapes, &SessionSchema),
    extension_functions: &[String],
) -> (ModelOutcome, Option<String>) {
    let (cleaned_sql, analysed): (String, Result<NativeAnalysis, String>) =
        match normalize_native_sql(&model.query_sql, dialect.name(), dialect.stub_prefix()) {
            Ok(normalised) => {
                let sql: String = placeholder_defaults(&normalised, &model.placeholders);
                let analysed = dialect
                    .analyse(&AnalysisInput {
                        sql: &sql,
                        schema: relations,
                        references: &model.lineage_references,
                        extension_functions,
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
