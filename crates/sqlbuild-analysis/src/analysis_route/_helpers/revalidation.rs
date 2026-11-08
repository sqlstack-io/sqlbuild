//! One model's native findings read again over its bound schema with poisoned outputs untyped.

use std::borrow::Cow;
use std::collections::HashMap;

use sqlquack_semantics::lineage::models::{Nullability, SchemaColumn};

use crate::analysis_route::models::{AnalysisInput, NativeAnalysis};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::analysis_session::constants::UNKNOWN_TYPE;
use crate::assembly::analysis_session::main::resolved_cleaned_sql::resolved_cleaned_sql;
use crate::assembly::analysis_session::models::{
    FinishedSession, ModelOutcome, PivotOutcome, SessionSchema,
};
use crate::assembly::analysis_session::types::Pairs;
use crate::semantic_checks::types::RevisedBinding;
use crate::semantic_validation::models::DiagnosticRow;

/// The finished outcome of the model the session analysed as `name`.
pub(crate) fn session_outcome<'a>(
    finished: &FinishedSession,
    outcomes: &'a [(ModelOutcome, PivotOutcome)],
    name: &str,
) -> Result<&'a ModelOutcome, String> {
    let position: Option<usize> = finished
        .model_names
        .iter()
        .position(|analysed| analysed == name);
    position
        .and_then(|position| outcomes.get(position))
        .map(|(outcome, _)| outcome)
        .ok_or_else(|| format!("the session analysed no model {name}"))
}

/// The findings of `outcome`'s analysed SQL over its bound schema, the poisoned columns untyped.
pub(crate) fn revised(
    dialect: &dyn NativeDialect,
    outcome: &ModelOutcome,
    poisoned: &[(String, String)],
    declared: &[String],
) -> Result<Vec<RevisedBinding>, String> {
    let sql: Cow<'_, str> = resolved_cleaned_sql(outcome)?;
    let relations: SessionSchema = SessionSchema {
        relations: outcome
            .validated_schema
            .iter()
            .filter(|(_, columns)| !columns.is_empty())
            .map(|(relation, columns)| {
                (
                    relation.clone(),
                    schema_columns(dialect, relation, columns, poisoned),
                )
            })
            .collect::<HashMap<String, Vec<SchemaColumn>>>(),
    };
    let analysis: NativeAnalysis = dialect
        .analyse(&AnalysisInput {
            sql: &sql,
            schema: &relations,
            references: &[],
            extension_functions: declared,
        })
        .map_err(|gap| gap.message)?;
    analysis
        .diagnostics
        .into_iter()
        .map(revised_binding)
        .collect()
}

/// `relation`'s columns as the session bound them and `dialect` reads them, poisoned ones untyped.
fn schema_columns(
    dialect: &dyn NativeDialect,
    relation: &str,
    columns: &Pairs,
    poisoned: &[(String, String)],
) -> Vec<SchemaColumn> {
    let mut schema: Vec<SchemaColumn> = Vec::with_capacity(columns.len());
    for (name, data_type) in columns {
        let untyped: bool = data_type.is_empty()
            || data_type == UNKNOWN_TYPE
            || poisoned
                .iter()
                .any(|(model, output)| model == relation && output == name);
        schema.push(SchemaColumn {
            name: name.clone(),
            quoted: false,
            data_type: (!untyped).then(|| dialect.session_type(data_type)),
            nullability: Nullability::Unknown,
        });
    }
    schema
}

/// A finding as type recovery matches it: its code and character span.
fn revised_binding(row: DiagnosticRow) -> Result<RevisedBinding, String> {
    let start: Option<i64> = row
        .start
        .map(i64::try_from)
        .transpose()
        .map_err(|error| error.to_string())?;
    let end: Option<i64> = row
        .end
        .map(i64::try_from)
        .transpose()
        .map_err(|error| error.to_string())?;
    Ok((row.code, start, end))
}
