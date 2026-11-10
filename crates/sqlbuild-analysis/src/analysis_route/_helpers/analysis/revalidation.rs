//! One model's native findings read again over its bound schema with poisoned outputs untyped.

use std::borrow::Cow;
use std::collections::HashMap;

use sqlquack_semantics::lineage::models::{Nullability, SchemaColumn};

use crate::analysis_route::main::schema_column::schema_column;
use crate::analysis_route::models::{AnalysisInput, NativeAnalysis, ParserGap};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::analysis_session::constants::UNKNOWN_TYPE;
use crate::assembly::analysis_session::main::resolved_cleaned_sql::resolved_cleaned_sql;
use crate::assembly::analysis_session::models::{
    FinishedSession, ModelOutcome, PivotOutcome, SessionSchema,
};
use crate::assembly::analysis_session::types::Pairs;
use crate::semantic_checks::models::RecoveryModel;
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

/// The findings of `outcome`'s SQL with the poisoned columns untyped; `None` when it is unreadable.
pub(crate) fn revised(
    dialect: &dyn NativeDialect,
    outcome: &ModelOutcome,
    poisoned: &[(String, String)],
    finished: &FinishedSession,
) -> Result<Option<Vec<RevisedBinding>>, String> {
    let sql: Cow<'_, str> = resolved_cleaned_sql(outcome)?;
    let mut relations: HashMap<String, Vec<SchemaColumn>> = HashMap::new();
    let mut open: HashMap<String, Vec<SchemaColumn>> = HashMap::new();
    for (relation, columns) in &outcome.validated_schema {
        let (table, columns) = if columns.is_empty() {
            match declared_columns(finished, relation) {
                Some(declared) => (&mut open, declared),
                None => continue,
            }
        } else {
            (&mut relations, columns)
        };
        let _ = table.insert(
            relation.clone(),
            schema_columns(dialect, relation, columns, poisoned),
        );
    }
    let relations: SessionSchema = SessionSchema {
        relations,
        open,
        functions: finished.functions.clone(),
    };
    let analysed: Result<NativeAnalysis, ParserGap> = dialect.analyse(&AnalysisInput {
        sql: &sql,
        schema: &relations,
        references: &[],
        extension_functions: &finished.declared_functions,
        quoted_ignore_case: finished.quoted_ignore_case,
    });
    let Ok(analysis) = analysed else {
        return Ok(None);
    };
    analysis
        .diagnostics
        .into_iter()
        .map(revised_binding)
        .collect::<Result<Vec<RevisedBinding>, String>>()
        .map(Some)
}

/// The declared columns of `relation`, whose full shape the session did not know, when it has any.
fn declared_columns<'a>(finished: &'a FinishedSession, relation: &str) -> Option<&'a Pairs> {
    finished
        .tables
        .column_types
        .iter()
        .find(|(name, columns)| name == relation && !columns.is_empty())
        .map(|(_, columns)| columns)
}

/// The model's own findings, as type recovery matches them, kept as they are.
pub(crate) fn unrevised(model: &RecoveryModel) -> Vec<RevisedBinding> {
    model
        .raw_bindings
        .iter()
        .map(|raw| (raw.code.clone(), raw.start, raw.end))
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
        schema.push(schema_column(
            name,
            (!untyped).then(|| dialect.session_type(data_type)),
            Nullability::Unknown,
        ));
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
