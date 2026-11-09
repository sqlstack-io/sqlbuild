//! The provided native analysis: sqlquack-semantics lineage and findings as SQLBuild's facts.

use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_semantics::lineage::models::{
    LineageContext, NameFinding, Nullability, OutputLineage, QueryLineage, SourceColumn,
    TransformKind,
};
use sqlquack_semantics::{statement_lineage, statement_name_findings};
use sqlquack_syntax::Expression;

use crate::analysis_route::constants::{
    ERROR_SEVERITY, UNKNOWN_COLUMN_CODE, UNKNOWN_FUNCTION_CODE,
};
use crate::analysis_route::models::{AnalysisInput, NativeAnalysis};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::analysis_session::constants::{
    CONFIDENCE_HIGH, CONFIDENCE_MEDIUM, CONFIDENCE_UNKNOWN, NON_NULL_NULLABILITY,
    NULLABLE_NULLABILITY, TRANSFORM_AGGREGATION, TRANSFORM_CAST, TRANSFORM_CONSTANT,
    TRANSFORM_DIRECT, TRANSFORM_EXPRESSION, UNKNOWN_NULLABILITY,
};
use crate::assembly::analysis_session::models::{ColumnFact, LineageRow};
use crate::semantic_validation::main::located_type_spans::located_type_spans;
use crate::semantic_validation::main::sqlbuild_code::sqlbuild_code;
use crate::semantic_validation::main::text_findings::text_findings;
use crate::semantic_validation::models::DiagnosticRow;

/// The analysis of `statement`, the one statement of `input.sql`, under `dialect`'s profile.
pub(crate) fn native_analysis<D: NativeDialect + ?Sized>(
    dialect: &D,
    statement: &Expression,
    input: &AnalysisInput<'_>,
) -> NativeAnalysis {
    let profile: &'static dyn DialectProfile = dialect.profile_for(input);
    let context: LineageContext<'_> = LineageContext {
        profile,
        schema: input.schema,
        sql: Some(input.sql),
    };
    let lineage: QueryLineage = statement_lineage(statement, context);
    NativeAnalysis {
        columns: lineage
            .outputs
            .iter()
            .map(|output| column_fact(dialect, output))
            .collect(),
        lineage: lineage
            .outputs
            .iter()
            .map(|output| lineage_row(profile, output, input.references))
            .collect(),
        row_sources: lineage
            .row_sources
            .into_iter()
            .map(|source| (source.relation, source.column))
            .collect(),
        has_star: lineage.projects_star,
        star_resolved: !lineage.open_star,
        diagnostics: diagnostics(
            dialect.name(),
            input,
            statement_name_findings(statement, context),
        ),
    }
}

/// The findings SQLBuild reports, span-less type findings placed, then SQLBuild's own text checks.
fn diagnostics(
    dialect: &str,
    input: &AnalysisInput<'_>,
    findings: Vec<NameFinding>,
) -> Vec<DiagnosticRow> {
    let spans: Vec<Option<(usize, usize)>> = located_type_spans(
        input.sql,
        dialect,
        &findings
            .iter()
            .map(|finding| {
                (
                    finding.code,
                    finding.message.as_str(),
                    finding.span.map(|span| (span.start, span.end)),
                )
            })
            .collect::<Vec<_>>(),
    );
    let text: Vec<DiagnosticRow> = text_findings(input.sql, dialect);
    let mut rows: Vec<DiagnosticRow> = findings
        .into_iter()
        .zip(spans)
        .filter_map(|(finding, span)| diagnostic_row(input, finding, span))
        .filter(|row| {
            !text
                .iter()
                .any(|own| own.code == row.code && own.start == row.start)
        })
        .collect();
    rows.extend(text);
    rows
}

/// An output column in the analysis's own type spelling, kept when the dialect can compare it.
fn column_fact<D: NativeDialect + ?Sized>(dialect: &D, output: &OutputLineage) -> ColumnFact {
    ColumnFact {
        name: output.name.clone(),
        data_type: output
            .data_type
            .as_deref()
            .filter(|spelling| dialect.normalize_type(spelling).is_some())
            .map(str::to_owned),
        nullability: match output.nullability {
            Nullability::Unknown => UNKNOWN_NULLABILITY,
            Nullability::Nullable => NULLABLE_NULLABILITY,
            Nullability::NonNull => NON_NULL_NULLABILITY,
        }
        .to_owned(),
    }
}

/// An output column's lineage over the model's referenced resources, as the legacy rows give it.
fn lineage_row(
    profile: &dyn DialectProfile,
    output: &OutputLineage,
    references: &[(String, String, String)],
) -> LineageRow {
    let transform_code: u8 = match output.kind {
        TransformKind::Constant => TRANSFORM_CONSTANT,
        TransformKind::Direct => TRANSFORM_DIRECT,
        TransformKind::Cast => TRANSFORM_CAST,
        TransformKind::Expression => TRANSFORM_EXPRESSION,
        TransformKind::Aggregation => TRANSFORM_AGGREGATION,
    };
    let mut sources: Vec<(String, String, String)> = Vec::new();
    for source in output
        .sources
        .iter()
        .filter_map(|source| referenced_source(profile, source, references))
    {
        if !sources.contains(&source) {
            sources.push(source);
        }
    }
    let confidence_code: u8 = if sources.is_empty() && transform_code != TRANSFORM_CONSTANT {
        CONFIDENCE_UNKNOWN
    } else if output.resolved {
        CONFIDENCE_HIGH
    } else {
        CONFIDENCE_MEDIUM
    };
    LineageRow {
        output_column: output.name.clone(),
        transform_code,
        confidence_code,
        sources,
    }
}

/// `(resource type, resource name, column)` of a source column, each name resolved as the profile does.
fn referenced_source(
    profile: &dyn DialectProfile,
    source: &SourceColumn,
    references: &[(String, String, String)],
) -> Option<(String, String, String)> {
    let (_, resource_type, resource_name) = references.iter().find(|(analysis_name, _, _)| {
        *analysis_name == source.relation
            || profile.same_identifier(
                &profile.resolved_identifier(analysis_name, false),
                &source.relation,
            )
    })?;
    Some((
        resource_type.clone(),
        resource_name.clone(),
        source.column.clone(),
    ))
}

/// A finding in SQLBuild's code at its position in `input.sql`, unless SQLBuild does not report it.
fn diagnostic_row(
    input: &AnalysisInput<'_>,
    finding: NameFinding,
    span: Option<(usize, usize)>,
) -> Option<DiagnosticRow> {
    if finding.code == UNKNOWN_FUNCTION_CODE
        && input
            .extension_functions
            .iter()
            .any(|name| name.eq_ignore_ascii_case(function_name(&finding)))
    {
        return None;
    }
    let code: String = sqlbuild_code(finding.code)?;
    let (line, column, start, end) = match span {
        Some((start, end)) => {
            let (line, column) = line_column(input.sql, start);
            (Some(line), Some(column), Some(start), Some(end))
        }
        None => (None, None, None, None),
    };
    Some(DiagnosticRow {
        code,
        message: authored_message(&finding),
        line,
        column,
        start,
        end,
        severity: ERROR_SEVERITY.to_owned(),
    })
}

/// An unknown column's message with a quoted name as written, which SQLQuack quotes in lower case.
fn authored_message(finding: &NameFinding) -> String {
    if finding.code != UNKNOWN_COLUMN_CODE || finding.written.is_empty() {
        return finding.message.clone();
    }
    finding.message.replacen(
        &format!("'{}'", finding.written.to_lowercase()),
        &format!("'{}'", finding.written),
        1,
    )
}

/// The function an unknown-function finding names, as written or as its message quotes it.
fn function_name(finding: &NameFinding) -> &str {
    if finding.written.is_empty() {
        finding.message.split('\'').nth(1).unwrap_or_default()
    } else {
        &finding.written
    }
}

/// The line and column, from 1, of character `offset` in `sql`.
fn line_column(sql: &str, offset: usize) -> (usize, usize) {
    sql.chars()
        .take(offset)
        .fold((1, 1), |(line, column), character| {
            if character == '\n' {
                (line + 1, 1)
            } else {
                (line, column + 1)
            }
        })
}
