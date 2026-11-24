use crate::custom_rule_cache::main::rule_subjects::rule_subjects;
use crate::custom_rule_cache::models::{
    CustomRule, ModelSubjectInput, RuleIdentityInput, SubjectsInput,
};
use crate::custom_rule_cache::types::ReadSet;

pub(crate) fn identity_input() -> RuleIdentityInput {
    RuleIdentityInput {
        code_identity: "code".to_owned(),
        runtime_version: "runtime".to_owned(),
        code: "CUSTOM001".to_owned(),
        family: "CUSTOM".to_owned(),
        slug: "orders-rule".to_owned(),
        message: "message".to_owned(),
        remediation: "remediation".to_owned(),
        subject: Some("model".to_owned()),
        implementation_fingerprint: "source-a".to_owned(),
        config_json: "{}".to_owned(),
        dialect: "duckdb".to_owned(),
        project_dir: "/project".to_owned(),
    }
}

pub(crate) fn project(model_name: &str) -> SubjectsInput {
    SubjectsInput {
        target_name: Some("dev".to_owned()),
        models: vec![
            ModelSubjectInput {
                path: "sql/models/orders.sql".to_owned(),
                name: model_name.to_owned(),
                materialized: None,
            },
            ModelSubjectInput {
                path: "sql/models/customers.sql".to_owned(),
                name: "customers".to_owned(),
                materialized: Some("table".to_owned()),
            },
        ],
    }
}

pub(crate) fn rule(model_name: &str) -> CustomRule {
    CustomRule {
        code: "CUSTOM001".to_owned(),
        identity: Some("rule-identity".to_owned()),
        subject: Some("model".to_owned()),
        subjects: rule_subjects(Some("model"), &project(model_name)),
    }
}

pub(crate) fn readset() -> ReadSet {
    vec![vec!["project.models".to_owned()]]
}
