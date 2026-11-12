use sqlbuild_analysis::assembly::analysis_session::models::{LineageFacts, LineageRow};

use sqlbuild_analysis::compiled_project::models::{
    CompiledModelFacts, ModelSchemaFact, SchemaColumnFact,
};

use crate::models::{AnalysisRecord, ExpansionRecord, ModelInputRecord};
use crate::tests::test_types::TestHost;

pub(super) fn lineage_row(output: &str, sources: &[&str]) -> LineageRow {
    LineageRow {
        output_column: output.to_owned(),
        transform_code: 0,
        confidence_code: 0,
        sources: sources
            .iter()
            .map(|source| {
                (
                    "orders".to_owned(),
                    (*source).to_owned(),
                    "model".to_owned(),
                )
            })
            .collect(),
        expression_range: None,
    }
}

pub(super) fn analysis(lineage: Option<LineageFacts>) -> AnalysisRecord<TestHost> {
    AnalysisRecord {
        inferred: None,
        lineage,
        has_star: false,
        star_resolved: false,
        binding: Vec::new(),
        binding_validated: false,
        proof: None,
        host: (),
    }
}

pub(super) fn model_input(name: &str) -> ModelInputRecord<TestHost> {
    ModelInputRecord {
        name: name.to_owned(),
        query_sql: "SELECT 1".into(),
        macro_source_sql: "SELECT 1".into(),
        references: Vec::new(),
        schema: None,
        sql_validation_enabled: true,
        macro_deps: Vec::new(),
        sql_macro_checks: Vec::new(),
        typed_calls: Vec::new(),
        expansion: ExpansionRecord {
            authored_sql: "SELECT 1".into(),
            expanded_sql: "SELECT 1".into(),
            interpolation_spans: Vec::new(),
            declaration_spans: Vec::new(),
            macro_spans: Vec::new(),
            authored: true,
        },
        compiled: None,
        host: (),
    }
}

pub(super) fn schema_facts(name: &str, column: &str) -> CompiledModelFacts {
    CompiledModelFacts {
        name: name.to_owned(),
        schema: Some(ModelSchemaFact {
            columns: vec![SchemaColumnFact {
                name: column.to_owned(),
                ..SchemaColumnFact::default()
            }],
            ..ModelSchemaFact::default()
        }),
        ..CompiledModelFacts::default()
    }
}
