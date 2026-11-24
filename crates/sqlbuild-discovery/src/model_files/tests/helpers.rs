use sqlbuild_model_config::config_presence::models::MacroPath;

use crate::model_files::main::header_facts::header_facts;
use crate::model_files::main::parse_model_text::parse_model_text;
use crate::model_files::models::{
    DiscoveredModelFile, HeaderFacts, ModelFileOptions, NativeColumn,
};
use crate::model_files::tests::test_types::{
    ColumnSummary, EntrySummary, Located, ModelSummary, Span,
};
use crate::models::LineColumnSpan;

const FILE_PATH: &str = "/project/models/orders.sql";

fn options() -> ModelFileOptions {
    ModelFileOptions {
        path_filter: None,
        supported_keys: [
            "audits",
            "columns",
            "constants",
            "description",
            "enums",
            "materialized",
            "tags",
            "unique_key",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        removed_keys: vec!["run_despite_unchanged".to_owned()],
        extract_implicit_alias_columns: true,
        extract_output_column_locations: true,
    }
}

fn located(locations: &[(String, LineColumnSpan)]) -> Vec<Located> {
    locations
        .iter()
        .map(|(name, span)| {
            (
                name.clone(),
                (span.line, span.column, span.end_line, span.end_column),
            )
        })
        .collect()
}

fn summary(model: &DiscoveredModelFile) -> (String, Vec<Located>, Vec<Located>) {
    (
        model.query_sql.clone(),
        located(&model.header_column_locations),
        located(&model.output_column_locations),
    )
}

/// Parse one model file as `/project/models/orders.sql` and summarise the outcome.
pub(super) fn parsed_summary(contents: &str) -> ModelSummary {
    parse_model_text(FILE_PATH, contents.to_owned(), &options())
        .map(|model| summary(&model))
        .map_err(|failure| (failure.message, failure.help))
}

/// The expected successful summary.
pub(super) fn parsed(
    query: &str,
    header: &[(&str, Span)],
    output: &[(&str, Span)],
) -> ModelSummary {
    Ok((query.to_owned(), owned(header), owned(output)))
}

fn owned(locations: &[(&str, Span)]) -> Vec<Located> {
    locations
        .iter()
        .map(|(name, span)| ((*name).to_owned(), *span))
        .collect()
}

/// The expected failure summary.
pub(super) fn failed(message: &str, help: Option<&str>) -> ModelSummary {
    Err((message.to_owned(), help.map(str::to_owned)))
}

/// The header facts of one model file parsed as `/project/models/orders.sql`.
pub(super) fn parsed_header_facts(contents: &str) -> HeaderFacts {
    let model: DiscoveredModelFile = parse_model_text(FILE_PATH, contents.to_owned(), &options())
        .unwrap_or_else(|failure| panic!("{}", failure.message));
    header_facts(&model.header_values, "sql/models/orders.sql")
}

fn macro_path(path: &MacroPath) -> Option<Vec<&str>> {
    let MacroPath::Found(path) = path else {
        return None;
    };
    Some(path.iter().map(String::as_str).collect())
}

/// `(key, template, macro path, blocks reuse)` of each header entry, in dict order.
pub(super) fn entry_summaries(facts: &HeaderFacts) -> Vec<EntrySummary<'_>> {
    facts
        .entries
        .iter()
        .map(|(key, entry)| {
            (
                key.as_str(),
                entry.template,
                macro_path(&entry.macro_path),
                entry.blocks_reuse,
            )
        })
        .collect()
}

/// `(name, type, nullable, description, audit definitions)` of each parsed column.
pub(super) fn column_summaries(facts: &HeaderFacts) -> Option<Vec<ColumnSummary<'_>>> {
    let columns: &Vec<NativeColumn> = facts.columns.as_ref()?;
    Some(columns.iter().map(column_summary).collect())
}

fn column_summary(column: &NativeColumn) -> ColumnSummary<'_> {
    (
        column.name.as_str(),
        column.column_type.as_deref(),
        column.nullable,
        column.description.as_deref(),
        column
            .audits
            .iter()
            .map(|audit| audit.definition_name.as_str())
            .collect(),
    )
}
