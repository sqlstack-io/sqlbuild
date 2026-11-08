//! Rules model rows read from the compile's natively owned project facts.

use sqlbuild_analysis::compiled_project::models::{
    CompiledModelFacts, DeclarationFact, ModelSchemaFact,
};
use sqlbuild_sqltext::compiler::models::CharSpan;

use crate::lint_text::models::ExpansionSpan;
use crate::models::{
    AuthoredQueryMap, Declaration, DeclarationMember, DynamicFamilyRow, DynamicProofRow, ModelRow,
    Reference, SchemaColumnRow, SchemaRow,
};

/// The row the rules builder reads for one model; the authored query wins over the whole file.
pub(crate) fn model_row(facts: &CompiledModelFacts) -> Result<ModelRow, String> {
    if let Some(error) = &facts.config.encoding_error {
        return Err(error.clone());
    }
    let analysis = facts.analysis.clone().unwrap_or_default();
    Ok(ModelRow {
        name: facts.name.clone(),
        relative_path: facts.relative_path.clone(),
        query_sql: facts.query_sql.to_string(),
        authored_sql: if facts.authored_query_sql.is_empty() {
            facts.authored_sql.to_string()
        } else {
            facts.authored_query_sql.to_string()
        },
        query_line_offset: query_line_offset(
            &facts.authored_sql,
            &facts.authored_query_sql,
            &facts.query_sql,
        ),
        query_column_offset: query_column_offset(
            &facts.authored_sql,
            &facts.authored_query_sql,
            &facts.query_sql,
        ),
        authored_query: authored_query_map(facts),
        config: facts.config.values.clone(),
        authored_config_keys: facts.config.header_keys.clone(),
        logical_schema: facts.config.layer_schema.clone(),
        references: facts
            .references
            .iter()
            .map(|reference| Reference {
                ref_kind: reference.kind.clone(),
                ref_name: reference.name.clone(),
                ref_package: reference.package.clone(),
            })
            .collect(),
        schema: facts.schema.as_ref().map(schema_row),
        inferred_columns: analysis.inferred_columns,
        dynamic_proof: analysis.dynamic_proof.map(|proof| DynamicProofRow {
            output_proven: proof.output_proven,
            bare_dynamic_pivot: proof.bare_dynamic_pivot,
            families: proof.families,
        }),
        enum_columns: facts.enum_columns.clone(),
        enum_declarations: facts.enum_declarations.iter().map(declaration).collect(),
        constant_declarations: facts
            .constant_declarations
            .iter()
            .map(declaration)
            .collect(),
        typed_calls: facts.typed_calls.clone(),
        sql_analysis_skipped: facts.sql_analysis_skipped,
    })
}

fn schema_row(schema: &ModelSchemaFact) -> SchemaRow {
    SchemaRow {
        audit_count: schema.audit_count,
        columns: schema
            .columns
            .iter()
            .map(|column| SchemaColumnRow {
                name: column.name.clone(),
                data_type: column.data_type.clone(),
                nullable: column.nullable,
                audit_count: u32::try_from(column.audit_definitions.len()).unwrap_or(u32::MAX),
            })
            .collect(),
        dynamic_columns: schema
            .dynamic_columns
            .iter()
            .map(|family| DynamicFamilyRow {
                key: family.key.clone(),
                name: family.name.clone(),
                pivot_column: family.pivot_column.clone(),
                value_column: family.value_column.clone(),
                aggregate: family.aggregate.clone(),
                data_type: family.data_type.clone(),
                name_pattern: family.name_pattern.clone(),
            })
            .collect(),
    }
}

/// A declaration as the rules request carries it.
pub(crate) fn declaration(fact: &DeclarationFact) -> Declaration {
    Declaration {
        name: fact.name.clone(),
        relative_path: fact.relative_path.clone(),
        members: fact
            .members
            .iter()
            .map(|(name, value)| DeclarationMember {
                name: name.clone(),
                value: value.clone(),
            })
            .collect(),
        value: fact.value.clone(),
        value_type: fact.value_type.clone(),
        render_as: fact.render_as.clone(),
    }
}

/// The expansion passes back to the authored query and where that query starts in the file.
fn authored_query_map(facts: &CompiledModelFacts) -> Option<AuthoredQueryMap> {
    let span = |span: &CharSpan| ExpansionSpan {
        source_start: character_offset(span.source_start),
        source_end: character_offset(span.source_end),
        output_start: character_offset(span.output_start),
        output_end: character_offset(span.output_end),
    };
    if facts.authored_query_sql.is_empty() || facts.expansion_passes.iter().all(Vec::is_empty) {
        return None;
    }
    let start: usize = facts.authored_sql.rfind(&*facts.authored_query_sql)?;
    let above: &str = &facts.authored_sql[..start];
    Some(AuthoredQueryMap {
        passes: facts
            .expansion_passes
            .iter()
            .map(|pass| pass.iter().map(span).collect())
            .collect(),
        line_offset: above.matches('\n').count() as u64,
        column_offset: above
            .rsplit('\n')
            .next()
            .unwrap_or_default()
            .chars()
            .count() as u64,
    })
}

fn character_offset(offset: usize) -> i64 {
    i64::try_from(offset).unwrap_or(i64::MAX)
}

/// The file-line characters before the query's first content that the query itself lacks.
fn query_column_offset(file: &str, authored_query: &str, query: &str) -> u64 {
    let line_prefix = |text: &str, at: usize| {
        text[..at]
            .rsplit('\n')
            .next()
            .unwrap_or_default()
            .chars()
            .count()
    };
    let content_start = |text: &str| text.len() - text.trim_start().len();
    if authored_query.is_empty() {
        return 0;
    }
    file.rfind(authored_query).map_or(0, |start| {
        let in_file: usize = line_prefix(file, start + content_start(authored_query));
        let in_query: usize = line_prefix(query, content_start(query));
        in_file.saturating_sub(in_query) as u64
    })
}

/// The lines of `file` above `query`, the rendered form of its `authored_query`; 0 if unknown.
fn query_line_offset(file: &str, authored_query: &str, query: &str) -> u64 {
    let leading_lines = |text: &str| {
        let body: &str = text.trim_start();
        text[..text.len() - body.len()].matches('\n').count()
    };
    if authored_query.is_empty() {
        return 0;
    }
    file.rfind(authored_query).map_or(0, |start| {
        (file[..start].matches('\n').count() + leading_lines(authored_query))
            .saturating_sub(leading_lines(query)) as u64
    })
}
