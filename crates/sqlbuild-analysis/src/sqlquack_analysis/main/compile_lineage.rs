//! `compile_lineage`: each compiled model's column lineage from SQLQuack, for Snowflake and DuckDB.

use std::collections::{BTreeSet, HashMap};

use rayon::ThreadPool;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use sqlbuild_core::panics::main::catch_compiler_panic::catch_compiler_panic;
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_semantics::lineage::models::{LineageContext, OutputLineage, TransformKind};
use sqlquack_semantics::query_lineage;
use sqlquack_syntax::{ComplexityGuardOptions, Dialect, ParseOptions};

use crate::lineage::constants::MAX_FUNCTION_CALL_DEPTH;
use crate::lineage::models::{LineageConfidence, LineageTransformKind};
use crate::sqlquack_analysis::_helpers::schema::{
    RelationColumns, relation_key, schema_columns, sqlquack_dialect,
};
use crate::sqlquack_analysis::models::{CompileLineageColumn, CompiledModelSql};

/// Each model's lineage, `None` for one SQLQuack cannot read; `None` overall for another dialect.
pub fn compile_lineage(
    dialect: &str,
    models: &[CompiledModelSql<'_>],
    pool: &ThreadPool,
) -> Result<Option<Vec<Option<Vec<CompileLineageColumn>>>>, String> {
    let Some((dialect, profile)) = sqlquack_dialect(dialect) else {
        return Ok(None);
    };
    pool.install(|| {
        models
            .par_iter()
            .map(|model| catch_compiler_panic(|| Ok(model_lineage(model, dialect, profile))))
            .collect::<Result<Vec<_>, String>>()
    })
    .map(Some)
}

fn model_lineage(
    model: &CompiledModelSql<'_>,
    dialect: Dialect,
    profile: &'static dyn DialectProfile,
) -> Option<Vec<CompileLineageColumn>> {
    let schema = RelationColumns(
        model
            .schema
            .iter()
            .map(|(relation, columns)| {
                (
                    relation_key(relation, profile),
                    schema_columns(columns.iter().map(|(name, _)| name.as_str())),
                )
            })
            .collect(),
    );
    let references: HashMap<String, (&str, &str)> = model
        .references
        .iter()
        .map(|(name, kind, resource)| {
            (relation_key(name, profile), (kind.as_str(), resource.as_str()))
        })
        .collect();
    let options = ParseOptions {
        complexity_guard: Some(ComplexityGuardOptions {
            max_function_call_depth: Some(MAX_FUNCTION_CALL_DEPTH),
            ..ComplexityGuardOptions::default()
        }),
    };
    let context = LineageContext {
        profile,
        schema: &schema,
    };
    let lineage = query_lineage(model.sql, dialect, &options, context).ok()?;
    Some(
        lineage
            .outputs
            .iter()
            .filter(|output| !output.name.is_empty())
            .map(|output| lineage_column(output, &references, &schema, profile))
            .collect(),
    )
}

fn lineage_column(
    output: &OutputLineage,
    references: &HashMap<String, (&str, &str)>,
    schema: &RelationColumns,
    profile: &dyn DialectProfile,
) -> CompileLineageColumn {
    let mut sources: BTreeSet<(String, String, String)> = BTreeSet::new();
    for source in &output.sources {
        let Some((kind, resource)) = references.get(&source.relation) else {
            continue;
        };
        let quoted: bool = schema.0.get(&source.relation).is_some_and(|columns| {
            columns
                .iter()
                .any(|column| column.quoted && column.name == source.column)
        });
        sources.insert((
            (*kind).to_owned(),
            (*resource).to_owned(),
            profile.resolved_identifier(&source.column, quoted),
        ));
    }
    let transform: LineageTransformKind = match output.kind {
        TransformKind::Constant => LineageTransformKind::Constant,
        TransformKind::Direct => LineageTransformKind::Direct,
        TransformKind::Cast => LineageTransformKind::Cast,
        TransformKind::Expression => LineageTransformKind::Expression,
        TransformKind::Aggregation => LineageTransformKind::Aggregation,
    };
    let confidence: LineageConfidence = match (output.resolved, sources.is_empty()) {
        (true, _) => LineageConfidence::High,
        (false, false) => LineageConfidence::Medium,
        (false, true) => LineageConfidence::Unknown,
    };
    CompileLineageColumn {
        output_column: output.name.clone(),
        transform,
        confidence,
        sources: sources.into_iter().collect(),
    }
}
