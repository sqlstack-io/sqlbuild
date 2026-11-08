//! The provided native analysis: sqlquack-semantics lineage and findings as SQLBuild's facts.

use sqlquack_semantics::lineage::models::{
    LineageContext, NameFinding, Nullability, OutputLineage, QueryLineage, TransformKind,
};
use sqlquack_semantics::{statement_lineage, statement_name_findings};
use sqlquack_syntax::Expression;

use crate::analysis_route::constants::{ERROR_SEVERITY, UNKNOWN_FUNCTION_CODE};
use crate::analysis_route::models::{AnalysisInput, NativeAnalysis};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::analysis_session::constants::{
    CONFIDENCE_HIGH, CONFIDENCE_MEDIUM, CONFIDENCE_UNKNOWN, NON_NULL_NULLABILITY,
    NULLABLE_NULLABILITY, TRANSFORM_AGGREGATION, TRANSFORM_CAST, TRANSFORM_CONSTANT,
    TRANSFORM_DIRECT, TRANSFORM_EXPRESSION, UNKNOWN_NULLABILITY,
};
use crate::assembly::analysis_session::models::{ColumnFact, LineageRow};
use crate::semantic_validation::main::diagnostics::sqlbuild_code;
use crate::semantic_validation::models::DiagnosticRow;

/// The analysis of `statement`, the one statement of `input.sql`, under `dialect`'s profile.
pub(crate) fn native_analysis<D: NativeDialect + ?Sized>(
    dialect: &D,
    statement: &Expression,
    input: &AnalysisInput<'_>,
) -> NativeAnalysis {
    let context: LineageContext<'_> = LineageContext {
        profile: dialect.profile(),
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
            .map(|output| lineage_row(output, input.references))
            .collect(),
        row_sources: lineage
            .row_sources
            .into_iter()
            .map(|source| (source.relation, source.column))
            .collect(),
        has_star: lineage.projects_star,
        star_resolved: !lineage.open_star,
        diagnostics: statement_name_findings(statement, context)
            .into_iter()
            .filter_map(|finding| diagnostic_row(input, finding))
            .collect(),
    }
}

/// An output column typed through the dialect, so both routes spell its type alike.
fn column_fact<D: NativeDialect + ?Sized>(dialect: &D, output: &OutputLineage) -> ColumnFact {
    ColumnFact {
        name: output.name.clone(),
        data_type: output
            .data_type
            .as_deref()
            .and_then(|spelling| dialect.normalize_type(spelling))
            .map(|normalized| normalized.normalized_name),
        nullability: match output.nullability {
            Nullability::Unknown => UNKNOWN_NULLABILITY,
            Nullability::Nullable => NULLABLE_NULLABILITY,
            Nullability::NonNull => NON_NULL_NULLABILITY,
        }
        .to_owned(),
    }
}

/// An output column's lineage over the model's referenced resources, as the legacy rows give it:
/// columns of relations the model does not reference are left out, and a column reading none of
/// its references is of unknown confidence unless it is a constant.
fn lineage_row(output: &OutputLineage, references: &[(String, String, String)]) -> LineageRow {
    let transform_code: u8 = match output.kind {
        TransformKind::Constant => TRANSFORM_CONSTANT,
        TransformKind::Direct => TRANSFORM_DIRECT,
        TransformKind::Cast => TRANSFORM_CAST,
        TransformKind::Expression => TRANSFORM_EXPRESSION,
        TransformKind::Aggregation => TRANSFORM_AGGREGATION,
    };
    let sources: Vec<(String, String, String)> = output
        .sources
        .iter()
        .filter_map(|source| {
            references
                .iter()
                .find(|(analysis_name, _, _)| *analysis_name == source.relation)
                .map(|(_, resource_type, resource_name)| {
                    (
                        resource_type.clone(),
                        resource_name.clone(),
                        source.column.clone(),
                    )
                })
        })
        .collect();
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

/// A finding in SQLBuild's code at its position in `input.sql`; `None` for a code SQLBuild does
/// not report, and for an unknown function the dialect declares as an extension.
fn diagnostic_row(input: &AnalysisInput<'_>, finding: NameFinding) -> Option<DiagnosticRow> {
    if finding.code == UNKNOWN_FUNCTION_CODE
        && input
            .extension_functions
            .iter()
            .any(|name| name.eq_ignore_ascii_case(function_name(&finding)))
    {
        return None;
    }
    let code: String = sqlbuild_code(finding.code)?;
    let (line, column, start, end) = match finding.span {
        Some(span) => {
            let (line, column) = line_column(input.sql, span.start);
            (Some(line), Some(column), Some(span.start), Some(span.end))
        }
        None => (None, None, None, None),
    };
    Some(DiagnosticRow {
        code,
        message: finding.message,
        line,
        column,
        start,
        end,
        severity: ERROR_SEVERITY.to_owned(),
    })
}

/// The function an unknown-function finding names: the name as written when the finding keeps
/// it, else the one its message quotes, as in `Unknown function 'NAME' for dialect DuckDB`.
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
