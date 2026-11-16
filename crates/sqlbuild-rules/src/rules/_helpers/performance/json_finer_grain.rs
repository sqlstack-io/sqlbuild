//! SQBRPERFORMANCE102: a semi-structured column must not repeat on a finer grain than its input.

use crate::models::{Fault, GrainColumn, JsonGrain, RuleMetadata, RulesConfig};
use crate::query_facts::models::{ItemFacts, SetFacts};
use crate::rules::_helpers::dispatch::evaluation::ParsedModel;
use crate::rules::_helpers::layers::model_layers::backs_published_view;
use crate::rules::models::FaultCollector;

pub(crate) fn evaluate(
    parsed: &ParsedModel<'_>,
    config: &RulesConfig,
    rule: &RuleMetadata,
    faults: &FaultCollector,
) {
    let Some(grain) = &parsed.model.json_grain else {
        return;
    };
    if grain.skipped.is_some() || backs_published_view(&parsed.model.name, &grain.readers, config) {
        return;
    }
    for column in &grain.columns {
        if let Some((input, input_key, model_key)) = coarser_input(column, grain) {
            let input: &str = input.split_once(':').map_or(input, |(_, name)| name);
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

/// An input whose key is a strict subset of a model key, unless another of its keys covers one.
fn coarser_input<'a>(
    column: &'a GrainColumn,
    grain: &'a JsonGrain,
) -> Option<(&'a str, &'a [String], &'a [String])> {
    for input in &column.inputs {
        let Some(keys) = grain.input_keys.get(input) else {
            continue;
        };
        let one_model_row_per_input_row = keys
            .iter()
            .any(|input_key| covers_model_key(input_key, &grain.model_keys));
        if one_model_row_per_input_row {
            continue;
        }
        for input_key in keys {
            if let Some(model_key) = finer_model_key(input_key, &grain.model_keys) {
                return Some((input.as_str(), input_key.as_slice(), model_key));
            }
        }
    }
    None
}

fn covers_model_key(input_key: &[String], model_keys: &[Vec<String>]) -> bool {
    model_keys
        .iter()
        .any(|model_key| subset(model_key, input_key))
}

fn finer_model_key<'a>(
    input_key: &[String],
    model_keys: &'a [Vec<String>],
) -> Option<&'a [String]> {
    model_keys
        .iter()
        .find(|model_key| input_key.len() < model_key.len() && subset(input_key, model_key))
        .map(Vec::as_slice)
}

fn subset(smaller: &[String], larger: &[String]) -> bool {
    smaller
        .iter()
        .all(|column| contains_ignoring_case(larger, column))
}

fn contains_ignoring_case(columns: &[String], name: &str) -> bool {
    columns
        .iter()
        .any(|column| column.eq_ignore_ascii_case(name))
}

/// Where the root select projects `name`, following the left branch of set operations.
fn output_start(body: &SetFacts, name: &str) -> Option<usize> {
    match body {
        SetFacts::Select(select) => {
            for item in &select.items {
                if let ItemFacts::Expression(expression) = item
                    && expression
                        .output_name
                        .as_deref()
                        .is_some_and(|output| output.eq_ignore_ascii_case(name))
                    && expression.start.is_some()
                {
                    return expression.start;
                }
            }
            None
        }
        SetFacts::Query(query) => output_start(&query.body, name),
        SetFacts::SetOperation { left, .. } => output_start(left, name),
        SetFacts::Other => None,
    }
}
