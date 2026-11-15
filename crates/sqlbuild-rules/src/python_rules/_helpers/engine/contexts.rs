//! The context Fensu's rules read for one file, built the way Fensu builds it for its own runs.

use std::collections::HashMap;

use fensu_native::rules::constants::DEAD_DEFINITION_CODE;
use fensu_native::rules::constants::{
    TOO_MANY_DISTINCT_CALLS_CODE, TOO_MANY_LOCALS_CODE, TOO_MANY_STATEMENTS_CODE,
};
use fensu_native::{NativeDeadCodeContext, NativeRuleContext};
use std::path::Path;

use crate::models::{EvaluateRequest, RulesConfig};
use crate::python_rules::constants::{
    NAMING_CONTRACTS, NAMING_FAMILY, PACKAGE_ROOT, PROJECT_CONFIG_FILE,
};
use crate::python_rules::models::{PythonRuleSpec, PythonTarget};

/// Fensu passes these thresholds only to main modules.
const MAIN_ONLY_THRESHOLD_CODES: [&str; 3] = [
    TOO_MANY_STATEMENTS_CODE,
    TOO_MANY_DISTINCT_CALLS_CODE,
    TOO_MANY_LOCALS_CODE,
];

/// What every file's context shares.
#[derive(Debug)]
pub(crate) struct ContextSettings {
    pub repo_root: String,
    /// The runtime package folder, where domains begin.
    pub ownership_root: String,
    pub contracts: Vec<(String, String)>,
    pub dead_code: NativeDeadCodeContext,
}

/// The shared context of a run; `entrypoints` are the definitions SQLBuild loads, for reachability.
pub(crate) fn context_settings(
    request: &EvaluateRequest,
    rules: &[&PythonRuleSpec],
    entrypoints: Vec<(String, String)>,
) -> ContextSettings {
    let contracts: Vec<(String, String)> = if rules.iter().any(|rule| rule.family == NAMING_FAMILY)
    {
        NAMING_CONTRACTS
            .iter()
            .map(|(pattern, contract)| ((*pattern).to_owned(), (*contract).to_owned()))
            .collect()
    } else {
        Vec::new()
    };
    ContextSettings {
        repo_root: request.project_dir.clone(),
        ownership_root: Path::new(&request.project_dir)
            .join(PACKAGE_ROOT)
            .to_string_lossy()
            .into_owned(),
        contracts,
        dead_code: NativeDeadCodeContext {
            enabled: rules
                .iter()
                .any(|rule| rule.fensu_code == Some(DEAD_DEFINITION_CODE)),
            roots: Vec::new(),
            entrypoints,
            config_path: PROJECT_CONFIG_FILE.to_owned(),
        },
    }
}

/// The context of `target`, without thresholds, observations or the dead-code context.
pub(crate) fn rule_context(target: &PythonTarget, settings: &ContextSettings) -> NativeRuleContext {
    let shared = NativeRuleContext {
        scope: target.scope.to_owned(),
        repository_path: target.repository_path.clone(),
        relative_parts: target.relative_parts.clone(),
        contracts: settings.contracts.clone(),
        repo_root: settings.repo_root.clone(),
        scope_roots: vec![(target.scope.to_owned(), target.root.clone())],
        ..NativeRuleContext::default()
    };
    if !target.package {
        return NativeRuleContext {
            package_name: target
                .root
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_owned(),
            ownership_offset: None,
            ..shared
        };
    }
    NativeRuleContext {
        role: target.role.clone(),
        is_main_module: target.is_main_module,
        is_entry_module: target.is_entry_module,
        package_name: PACKAGE_ROOT.to_owned(),
        ownership_root: Some(settings.ownership_root.clone()),
        ownership_roots: vec![settings.ownership_root.clone()],
        ownership_offset: Some(0),
        ..shared
    }
}

/// The file's rules split into calls whose thresholds agree, each as Fensu codes and thresholds.
pub(crate) fn threshold_calls(
    rules: &[&PythonRuleSpec],
    config: &RulesConfig,
    is_main_module: bool,
) -> Vec<(Vec<String>, HashMap<String, u32>)> {
    let mut calls: Vec<(Vec<String>, HashMap<String, u32>)> = Vec::new();
    for rule in rules {
        let Some(fensu_code) = rule.fensu_code else {
            continue;
        };
        let thresholds: Vec<(String, u32)> = if !is_main_module
            && MAIN_ONLY_THRESHOLD_CODES.contains(&fensu_code)
        {
            Vec::new()
        } else {
            rule.options
                .iter()
                .map(|(name, default, _)| {
                    let value: u32 = config.rule_option_u32(rule.code, name).unwrap_or(*default);
                    ((*name).to_owned(), value)
                })
                .collect()
        };
        let agrees = |current: &HashMap<String, u32>| {
            thresholds
                .iter()
                .all(|(name, value)| current.get(name).is_none_or(|existing| existing == value))
        };
        let index: usize = match calls.iter().position(|(_, current)| agrees(current)) {
            Some(index) => index,
            None => {
                calls.push((Vec::new(), HashMap::new()));
                calls.len() - 1
            }
        };
        calls[index].0.push(fensu_code.to_owned());
        calls[index].1.extend(thresholds);
    }
    calls
}
