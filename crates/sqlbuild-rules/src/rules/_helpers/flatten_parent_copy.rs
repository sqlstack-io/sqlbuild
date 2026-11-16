//! SQBRPERFORMANCE101: a flattened row must not copy the parent element whose child array it flattens.

use std::collections::BTreeMap;

use crate::models::{Fault, RuleMetadata};
use crate::query_facts::models::{
    ExpressionItem, FlattenInput, ItemFacts, QueryFacts, RelationFacts, SelectFacts, SetFacts,
};
use crate::rules::_helpers::evaluation::ParsedModel;
use crate::rules::models::FaultCollector;

const VALUE_COLUMN: &str = "value";

pub(crate) fn evaluate(parsed: &ParsedModel<'_>, rule: &RuleMetadata, faults: &FaultCollector) {
    let mut selects: Vec<&SelectFacts> = Vec::new();
    collect_query(parsed.facts, &mut selects);
    for select in selects {
        let children: BTreeMap<&str, Vec<Option<&str>>> = flattened_parents(select);
        for item in &select.items {
            let ItemFacts::Expression(expression) = item else {
                continue;
            };
            if let Some((parent, key)) = copied_parent(expression, &children) {
                push(parsed, rule, faults, expression, parent, key);
            }
        }
    }
}

fn collect_query<'a>(query: &'a QueryFacts, selects: &mut Vec<&'a SelectFacts>) {
    for cte in &query.ctes {
        collect_query(&cte.query, selects);
    }
    collect_set(&query.body, selects);
}

fn collect_set<'a>(set: &'a SetFacts, selects: &mut Vec<&'a SelectFacts>) {
    match set {
        SetFacts::Select(select) => selects.push(select),
        SetFacts::Query(query) => collect_query(query, selects),
        SetFacts::SetOperation { left, right, .. } => {
            collect_set(left, selects);
            collect_set(right, selects);
        }
        SetFacts::Other => {}
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
        let kept = keys.iter().find(|key| {
            key.is_none_or(|key| !whole.removed.iter().any(|removed| removed == key))
        })?;
        Some((whole.qualifier.as_str(), *kept))
    })
}

fn push(
    parsed: &ParsedModel<'_>,
    rule: &RuleMetadata,
    faults: &FaultCollector,
    expression: &ExpressionItem,
    parent: &str,
    key: Option<&str>,
) {
    let (line, column) = expression
        .start
        .map_or((1, 1), |offset| parsed.source.line_column(offset));
    let output: &str = expression.output_name.as_deref().unwrap_or("this column");
    let child: String = key.map_or_else(
        || "its child array".to_owned(),
        |key| format!("{parent}.value:{key}"),
    );
    faults.push(Fault {
        unevaluated: false,
        code: rule.code.clone(),
        path: parsed.model.relative_path.clone(),
        line,
        column,
        message: format!(
            "{output} copies {parent}.value whole onto every row flattened from {child}"
        ),
        remediation: rule.remediation.clone(),
    });
}
