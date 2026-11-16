//! SQBRPERFORMANCE101: a flattened row must not copy the parent element whose child array it flattens.

use std::collections::BTreeMap;

use crate::models::{Fault, RuleMetadata};
use crate::query_facts::models::{
    ExpressionItem, FlattenInput, ItemFacts, QueryFacts, RelationFacts, SelectFacts, SetFacts,
};
use crate::rules::_helpers::dispatch::evaluation::ParsedModel;
use crate::rules::models::FaultCollector;

const VALUE_COLUMN: &str = "value";

pub(crate) fn evaluate(parsed: &ParsedModel<'_>, rule: &RuleMetadata, faults: &FaultCollector) {
    for select in query_selects(parsed.facts) {
        let children: BTreeMap<&str, Vec<Option<&str>>> = flattened_parents(select);
        for item in &select.items {
            let ItemFacts::Expression(expression) = item else {
                continue;
            };
            if let Some(copied) = copied_parent(expression, &children) {
                faults.push(fault(parsed, rule, expression, copied));
            }
        }
    }
}

/// Every SELECT in `query`, its CTEs first.
fn query_selects(query: &QueryFacts) -> Vec<&SelectFacts> {
    let mut selects: Vec<&SelectFacts> = Vec::new();
    for cte in &query.ctes {
        selects.extend(query_selects(&cte.query));
    }
    selects.extend(set_selects(&query.body));
    selects
}

fn set_selects(set: &SetFacts) -> Vec<&SelectFacts> {
    match set {
        SetFacts::Select(select) => vec![select],
        SetFacts::Query(query) => query_selects(query),
        SetFacts::SetOperation { left, right, .. } => {
            let mut selects: Vec<&SelectFacts> = set_selects(left);
            selects.extend(set_selects(right));
            selects
        }
        SetFacts::Other => Vec::new(),
    }
}

/// Each FLATTEN alias in `select` that a deeper FLATTEN reads, with the child keys read from it.
fn flattened_parents(select: &SelectFacts) -> BTreeMap<&str, Vec<Option<&str>>> {
    let relations = || select.from.iter().chain(&select.joins);
    let aliases: Vec<&str> = relations()
        .filter_map(|relation| match relation {
            RelationFacts::Flatten {
                alias: Some(alias), ..
            } => Some(alias.as_str()),
            _ => None,
        })
        .collect();
    let mut parents: BTreeMap<&str, Vec<Option<&str>>> = BTreeMap::new();
    for relation in relations() {
        if let RelationFacts::Flatten {
            input:
                Some(FlattenInput {
                    qualifier,
                    column,
                    key,
                }),
            ..
        } = relation
            && column == VALUE_COLUMN
            && aliases.contains(&qualifier.as_str())
        {
            parents
                .entry(qualifier.as_str())
                .or_default()
                .push(key.as_deref());
        }
    }
    parents
}

/// The parent `expression` copies whole and a child key it keeps, unless `OBJECT_DELETE` removes them all.
fn copied_parent<'a>(
    expression: &'a ExpressionItem,
    parents: &BTreeMap<&str, Vec<Option<&'a str>>>,
) -> Option<(&'a str, Option<&'a str>)> {
    expression.whole_values.iter().find_map(|whole| {
        let keys = parents.get(whole.qualifier.as_str())?;
        let kept: Option<&str> = kept_key(keys, &whole.removed)?;
        Some((whole.qualifier.as_str(), kept))
    })
}

/// The first child key that `removed` does not take out of the parent; `None` reads it all.
fn kept_key<'a>(keys: &[Option<&'a str>], removed: &[String]) -> Option<Option<&'a str>> {
    for key in keys {
        match key {
            None => return Some(None),
            Some(key) if !removed.iter().any(|name| name == key) => return Some(Some(key)),
            Some(_) => {}
        }
    }
    None
}

fn fault(
    parsed: &ParsedModel<'_>,
    rule: &RuleMetadata,
    expression: &ExpressionItem,
    (parent, key): (&str, Option<&str>),
) -> Fault {
    let (line, column) = expression
        .start
        .map_or((1, 1), |offset| parsed.source.line_column(offset));
    let output: &str = expression.output_name.as_deref().unwrap_or("this column");
    let child: String = key.map_or_else(
        || "its child array".to_owned(),
        |key| format!("{parent}.value:{key}"),
    );
    Fault {
        unevaluated: false,
        code: rule.code.clone(),
        path: parsed.model.relative_path.clone(),
        line,
        column,
        message: format!(
            "{output} copies {parent}.value whole onto every row flattened from {child}"
        ),
        remediation: rule.remediation.clone(),
    }
}
