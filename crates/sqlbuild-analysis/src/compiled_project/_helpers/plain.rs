//! The model loop's ordered decisions and the facts it retains for a finished model.

use crate::compiled_project::constants::MACRO_TOKEN;
use crate::compiled_project::models::{
    AnalysisGate, AnalysisOverride, CompiledModelFacts, ModelLoopSettings, PlainModel,
};

/// The attachment's gate: project switches first, so an override is only read when they allow.
pub(crate) fn analysis_gate(
    settings: ModelLoopSettings,
    analysis_override: AnalysisOverride,
) -> AnalysisGate {
    if !settings.sql_analysis || settings.no_sql_validation {
        return AnalysisGate::Disabled;
    }
    match analysis_override {
        AnalysisOverride::Inherit | AnalysisOverride::Enabled => AnalysisGate::Enabled,
        AnalysisOverride::Disabled => AnalysisGate::Disabled,
        AnalysisOverride::Conflict => AnalysisGate::Conflict,
    }
}

/// Whether declaration-expanded SQL still calls macros.
pub(crate) fn calls_macros(sql: &str) -> bool {
    sql.contains(MACRO_TOKEN)
}

pub(crate) fn plain_model_facts(model: PlainModel) -> CompiledModelFacts {
    CompiledModelFacts {
        schema: model.schema,
        name: model.name,
        relative_path: model.relative_path,
        query_sql: model.query_sql,
        authored_query_sql: model.authored_query_sql,
        authored_sql: model.authored_sql,
        config: model.config,
        references: model.references,
        enum_columns: model.enum_columns,
        enum_declarations: model.enum_declarations,
        constant_declarations: model.constant_declarations,
        analysis: None,
        typed_calls: model.typed_calls,
        expansion_passes: model.expansion_passes,
        sql_analysis_skipped: model.sql_analysis_skipped,
    }
}
