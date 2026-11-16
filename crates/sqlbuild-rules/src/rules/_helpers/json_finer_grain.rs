//! SQBRPERFORMANCE102: a semi-structured column must not repeat on a finer grain than its input.

use crate::models::{Fault, GrainColumn, JsonGrain, RuleMetadata};
use crate::query_facts::models::{ItemFacts, SetFacts};
use crate::rules::_helpers::evaluation::ParsedModel;
use crate::rules::models::FaultCollector;

const SEMI_STRUCTURED_TYPES: [&str; 3] = ["VARIANT", "OBJECT", "ARRAY"];

pub(crate) fn evaluate(parsed: &ParsedModel<'_>, rule: &RuleMetadata, faults: &FaultCollector) {
    let Some(grain) = &parsed.model.json_grain else {
        return;
    };
    if grain.skipped.is_some() || grain.backs_published_view {
        return;
    }
    for column in &grain.columns {
        if !column.data_type.as_deref().is_some_and(|data_type| {
            SEMI_STRUCTURED_TYPES
                .iter()
                .any(|kind| starts_with_word(data_type, kind))
        }) {
            continue;
        }
        if let Some((input, input_key, model_key)) = coarser_input(column, grain) {
            let (line, column_number) = output_start(&parsed.facts.body, &column.name)
                .map_or((1, 1), |offset| parsed.source.line_column(offset));
            faults.push(Fault {
                unevaluated: false,
                code: rule.code.clone(),
                path: parsed.model.relative_path.clone(),
                line,
                column: column_number,
                message: format!(
                    "{} copies {} from {input}, one row per ({}), onto rows keyed by ({}), so the \
                     same value repeats on every row",
                    column.name,
                    column.data_type.as_deref().unwrap_or_default(),
                    input_key.join(", "),
                    model_key.join(", "),
                ),
                remediation: rule.remediation.clone(),
            });
        }
    }
}

fn starts_with_word(data_type: &str, kind: &str) -> bool {
    let upper: String = data_type.trim().to_ascii_uppercase();
    upper == kind
        || upper
            .strip_prefix(kind)
            .is_some_and(|rest| rest.starts_with(['(', '<', ' ']))
}

/// An input of `column` whose key is a strict subset of one of the model's keys.
fn coarser_input<'a>(
    column: &'a GrainColumn,
    grain: &'a JsonGrain,
) -> Option<(&'a str, &'a [String], &'a [String])> {
    column.inputs.iter().find_map(|input| {
        let keys = grain.input_keys.get(input)?;
        keys.iter().find_map(|input_key| {
            grain
                .model_keys
                .iter()
                .find(|model_key| strict_subset(input_key, model_key))
                .map(|model_key| (input.as_str(), input_key.as_slice(), model_key.as_slice()))
        })
    })
}

fn strict_subset(smaller: &[String], larger: &[String]) -> bool {
    smaller.len() < larger.len()
        && smaller.iter().all(|column| {
            larger
                .iter()
                .any(|other| other.eq_ignore_ascii_case(column))
        })
}

/// Where the root select projects `name`, following the left branch of set operations.
fn output_start(body: &SetFacts, name: &str) -> Option<usize> {
    match body {
        SetFacts::Select(select) => select.items.iter().find_map(|item| match item {
            ItemFacts::Expression(expression)
                if expression
                    .output_name
                    .as_deref()
                    .is_some_and(|output| output.eq_ignore_ascii_case(name)) =>
            {
                expression.start
            }
            _ => None,
        }),
        SetFacts::Query(query) => output_start(&query.body, name),
        SetFacts::SetOperation { left, .. } => output_start(left, name),
        SetFacts::Other => None,
    }
}
