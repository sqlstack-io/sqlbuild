use crate::constants::ENFORCED_CONTRACT;
use crate::models::{Fault, Model, RuleMetadata};
use crate::rules::models::FaultCollector;

pub(crate) fn evaluate(model: &Model, rule: &RuleMetadata, faults: &FaultCollector) {
    if model
        .config
        .get("contract")
        .and_then(serde_json::Value::as_str)
        != Some(ENFORCED_CONTRACT)
    {
        return;
    }
    for column in &model.columns {
        if column.data_type.trim().is_empty() {
            faults.push(Fault {
                unevaluated: false,
                code: rule.code.clone(),
                path: model.relative_path.clone(),
                line: 1,
                column: 1,
                message: format!("contract column {:?} has no declared type", column.name),
                remediation: rule.remediation.clone(),
            });
        }
    }
    for family in &model.dynamic_columns {
        if family.data_type.trim().is_empty() {
            faults.push(Fault {
                unevaluated: false,
                code: rule.code.clone(),
                path: model.relative_path.clone(),
                line: 1,
                column: 1,
                message: format!(
                    "dynamic contract family {:?} has no declared type",
                    family.name
                ),
                remediation: rule.remediation.clone(),
            });
        }
    }
}
