//! One model's outcome from its dialect's native analysis module, in the session's facts.

use std::collections::HashMap;

use sqlbuild_sqltext::compiler::main::placeholder_defaults::placeholder_defaults;

use crate::analysis_route::models::{AnalysisInput, NativeAnalysis, ParserGap};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::analysis_session::models::{
    LineageFacts, ModelAnalysis, ModelOutcome, ModelRequest, SessionSchema,
};
use crate::assembly::analysis_session::types::Shapes;
use crate::semantic_validation::_helpers::normalization::normalize_native_sql;

/// The model's analysis SQL as its native parser reads it, each reference in `relations` renamed.
pub(crate) fn native_sql(
    dialect: &dyn NativeDialect,
    model: &ModelRequest,
    relations: &HashMap<String, String>,
) -> Result<String, String> {
    normalize_native_sql(
        &model.query_sql,
        dialect.name(),
        dialect.stub_prefix(),
        relations,
    )
    .map(|normalised| placeholder_defaults(&normalised, &model.placeholders))
}

/// The native analysis of `sql` over `relations` and `references`, or why the parser refused it.
pub(crate) fn analysed_sql(
    dialect: &dyn NativeDialect,
    sql: &str,
    (relations, references): (&SessionSchema, &[(String, String, String)]),
    (extension_functions, quoted_ignore_case): (&[String], bool),
) -> Result<NativeAnalysis, String> {
    dialect
        .analyse(&AnalysisInput {
            sql,
            schema: relations,
            references,
            extension_functions,
            quoted_ignore_case,
        })
        .map_err(|gap: ParserGap| gap.message)
}

/// The model's outcome from its cleaned SQL and analysis, and why the analysis failed.
pub(crate) fn native_outcome(
    model: &ModelRequest,
    schema: Shapes,
    cleaned_sql: String,
    analysed: Result<NativeAnalysis, String>,
) -> (ModelOutcome, Option<String>) {
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
