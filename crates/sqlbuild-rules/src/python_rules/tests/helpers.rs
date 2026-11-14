use std::collections::BTreeSet;

use fensu_facts::extension::models::ProgramHandle;
use fensu_native::{NativeProjectPlane, NativeRuleContext, evaluate_core_rules};

use crate::engine::_helpers::memo::{evaluate_rows, prepare};
use crate::models::{EvaluateRequest, RowsRequest, RuleMetadata, RuleScope, RulesConfig};
use crate::rules::_helpers::catalogue::{catalogue, select};

/// How many distinct `SQBRPYTHON` codes the catalogue holds.
pub(super) fn distinct_python_codes(rules: &[RuleMetadata]) -> usize {
    rules
        .iter()
        .filter(|rule| rule.code.starts_with("SQBRPYTHON"))
        .map(|rule| rule.code.as_str())
        .collect::<BTreeSet<&str>>()
        .len()
}

/// How many catalogue rules are project-wide Python rules.
pub(super) fn project_wide_python_rules(rules: &[RuleMetadata]) -> usize {
    rules
        .iter()
        .filter(|rule| {
            rule.scope == RuleScope::Python && rule.project_wide && rule.family == "python"
        })
        .count()
}

/// Whether selecting `select` runs the rule `code`.
pub(super) fn is_selected(selectors: &[String], code: &str) -> bool {
    let rules: Vec<RuleMetadata> = catalogue();
    select(&rules, Some(selectors), &[])
        .expect("valid selection")
        .iter()
        .any(|rule| rule.code == code)
}

/// The faults one Fensu core rule reports on a root-scope module, as code and line.
pub(super) fn core_rule_faults(source: &str, fensu_code: &str) -> Vec<(String, u32)> {
    let program: ProgramHandle = ProgramHandle::parse_source(source, 3, 12).expect("source parses");
    let context = NativeRuleContext {
        scope: "root".to_owned(),
        ..NativeRuleContext::default()
    };
    evaluate_core_rules(
        &program,
        &[fensu_code.to_owned()],
        &context,
        &NativeProjectPlane::new(Vec::new(), Vec::new()),
    )
    .expect("rule evaluates")
    .into_iter()
    .map(|fault| (fault.code, fault.line))
    .collect()
}

/// The speculation keys of a request selecting `select`, before and after its one Python module changes from `original` to `edited`.
pub(super) fn source_keys(select: &[&str], original: &str, edited: &str) -> (String, String) {
    let directory = tempfile::tempdir().expect("temporary project");
    let module = directory.path().join("python/orders/main.py");
    std::fs::create_dir_all(module.parent().expect("parent")).expect("folders");
    std::fs::write(&module, original).expect("module");
    let before: String = request_key(directory.path(), select);
    std::fs::write(&module, edited).expect("module");
    (before, request_key(directory.path(), select))
}

fn request_key(project_dir: &std::path::Path, select: &[&str]) -> String {
    let config: RulesConfig =
        serde_json::from_value(serde_json::json!({ "select": select })).expect("rules config");
    let request = EvaluateRequest {
        project_dir: project_dir.to_string_lossy().into_owned(),
        config,
        ..EvaluateRequest::default()
    };
    prepare(
        RowsRequest {
            request,
            memo: None,
            retain: false,
        },
        true,
    )
    .expect("request prepares")
    .key
    .expect("keyed")
}

/// The Python faults of a project holding `files`, under rules config `config`, as code, path, line and column.
pub(super) fn python_rule_faults(
    files: &[(&str, &str)],
    config: serde_json::Value,
) -> Result<Vec<(String, String, u64, u64)>, String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    for (path, contents) in files {
        let file = directory.path().join(path);
        std::fs::create_dir_all(file.parent().expect("parent"))
            .map_err(|error| error.to_string())?;
        std::fs::write(&file, contents).map_err(|error| error.to_string())?;
    }
    let mut config = config;
    config["cache"] = serde_json::json!({ "enabled": false });
    let request = EvaluateRequest {
        project_dir: directory.path().to_string_lossy().into_owned(),
        config: serde_json::from_value(config).map_err(|error| error.to_string())?,
        ..EvaluateRequest::default()
    };
    let evaluation = evaluate_rows(RowsRequest {
        request,
        memo: None,
        retain: false,
    })
    .map_err(|error| format!("{error:?}"))?;
    Ok(evaluation
        .faults
        .into_iter()
        .filter(|fault| fault.code.starts_with("SQBRPYTHON"))
        .map(|fault| (fault.code, fault.path, fault.line, fault.column))
        .collect())
}

/// Expected faults as the owned tuples `python_rule_faults` returns.
pub(super) fn owned_faults(faults: &[(&str, &str, u64, u64)]) -> Vec<(String, String, u64, u64)> {
    faults
        .iter()
        .map(|(code, path, line, column)| ((*code).to_owned(), (*path).to_owned(), *line, *column))
        .collect()
}
