use std::collections::BTreeMap;

use crate::constants::{ENFORCED_CONTRACT, SELECT_STAR_MODEL_RULE_CODE};
use crate::models::{Fault, Model, RuleMetadata, RulesConfig};
use crate::query_facts::models::{QueryFacts, RelationFacts, SelectFacts};
use crate::rules::_helpers::evaluation::sole_table_name;
use crate::rules::_helpers::{contract_name_types, explicit_output_types, typed_contract_columns};
use crate::rules::models::FaultCollector;

pub(crate) fn evaluate_without_query(
    model: &Model,
    config: &RulesConfig,
    selected: &BTreeMap<String, &RuleMetadata>,
) -> Option<Vec<Fault>> {
    if !model.dynamic_columns_proven || !rules_need_no_query(model, selected) {
        return None;
    }
    let faults = FaultCollector::default();
    if let Some(rule) = selected.get("SQBRCONTRACT101")
        && model
            .config
            .get("contract")
            .and_then(serde_json::Value::as_str)
            != Some(ENFORCED_CONTRACT)
    {
        faults.push(fault(model, rule));
    }
    if let Some(rule) = selected.get("SQBRCONTRACT106") {
        typed_contract_columns::evaluate(model, rule, &faults);
    }
    if let Some(rule) = selected.get("SQBRCONTRACT105") {
        explicit_output_types::evaluate_proven_dynamic(model, rule, &faults);
    }
    for column in &model.columns {
        if let Some((code, message)) = contract_name_types::finding(column, config)
            && let Some(rule) = selected.get(code)
        {
            faults.push(Fault {
                unevaluated: false,
                code: rule.code.clone(),
                path: model.relative_path.clone(),
                line: 1,
                column: 1,
                message,
                remediation: rule.remediation.clone(),
            });
        }
    }
    Some(faults.into_inner())
}

/// Whether `select` is a lone star over one dynamic pivot, whose columns only the pivot knows.
pub(crate) fn is_dynamic_pivot_star(select: &SelectFacts) -> bool {
    select.items.len() == 1
        && select.from.len() == 1
        && select.joins.is_empty()
        && matches!(select.from[0], RelationFacts::DynamicPivot)
}

pub(crate) fn allows_output_star(query: &QueryFacts, root: &QueryFacts, model: &Model) -> bool {
    let Some(select) = query.root_select() else {
        return false;
    };
    is_dynamic_pivot_star(select)
        || (model.dynamic_columns_proven
            && std::ptr::eq(query, root)
            && root.ctes.last().is_some_and(|cte| {
                sole_table_name(query).is_some_and(|name| name.eq_ignore_ascii_case(&cte.name))
            }))
}

fn rules_need_no_query(model: &Model, selected: &BTreeMap<String, &RuleMetadata>) -> bool {
    selected.values().all(|rule| {
        rule.custom
            || rule.code.starts_with("SQLCLEAN")
            || rule.code.starts_with("SQBRCONTRACT10")
            || (model.bare_dynamic_pivot && rule.code == SELECT_STAR_MODEL_RULE_CODE)
            || matches!(
                rule.code.as_str(),
                "SQBRDECLARATION201" | "SQBRDECLARATION301" | "SQBRTEST301"
            )
    })
}

fn fault(model: &Model, rule: &RuleMetadata) -> Fault {
    Fault {
        unevaluated: false,
        code: rule.code.clone(),
        path: model.relative_path.clone(),
        line: 1,
        column: 1,
        message: rule.message.clone(),
        remediation: rule.remediation.clone(),
    }
}
