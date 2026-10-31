//! `compile_lineage`: each compiled model's column lineage from SQLQuack, for Snowflake and DuckDB.

use std::collections::{BTreeSet, HashMap};

use rayon::ThreadPool;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use sqlbuild_core::panics::main::catch_compiler_panic::catch_compiler_panic;
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_semantics::lineage::models::{LineageContext, OutputLineage, SchemaColumn};
use sqlquack_semantics::query_lineage;
use sqlquack_syntax::Dialect;

use crate::sqlquack_analysis::_helpers::outputs::{
    confidence, lineage_parse_options, transform_kind,
};
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
            .map(|(relation, columns)| (relation_key(relation, profile), column_names(columns)))
            .collect(),
    );
    let references: HashMap<String, (&str, &str)> = model
        .references
        .iter()
        .map(|(name, kind, resource)| {
            (
                relation_key(name, profile),
                (kind.as_str(), resource.as_str()),
            )
        })
        .collect();
    let context = LineageContext {
        profile,
        schema: &schema,
        sql: None,
    };
    match query_lineage(model.sql, dialect, &lineage_parse_options(), context) {
        Ok(lineage) => Some(
            lineage
                .outputs
                .iter()
                .filter(|output| !output.name.is_empty())
                .map(|output| lineage_column(output, &references, profile))
                .collect(),
        ),
        Err(_unreadable) => None,
    }
}

/// A relation's schema columns from its `(column, type)` pairs.
fn column_names(columns: &[(String, String)]) -> Vec<SchemaColumn> {
    schema_columns(columns.iter().map(|(name, _)| name.as_str()))
}

fn lineage_column(
    output: &OutputLineage,
    references: &HashMap<String, (&str, &str)>,
    profile: &dyn DialectProfile,
) -> CompileLineageColumn {
    let mut sources: BTreeSet<(String, String, String)> = BTreeSet::new();
    for source in &output.sources {
        let Some((kind, resource)) = references.get(&source.relation) else {
            continue;
        };
        sources.insert((
            (*kind).to_owned(),
            (*resource).to_owned(),
            profile.resolved_identifier(&source.column, source.quoted),
        ));
    }
    CompileLineageColumn {
        output_column: output.name.clone(),
        transform: transform_kind(output.kind),
        confidence: confidence(output.resolved, !sources.is_empty()),
        sources: sources.into_iter().collect(),
    }
}
