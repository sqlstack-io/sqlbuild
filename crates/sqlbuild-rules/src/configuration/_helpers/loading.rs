use crate::configuration::_helpers::removed_settings::removed_settings;
use crate::configuration::errors::RulesConfigLoadError;
use crate::constants::{
    ALLOWED_PATHS_KEY, ALLOWED_QUALIFIERS_OPTION, MIN_CUSTOM_RULE_TEST_CASES_KEY,
    MODEL_NAME_LAYERS, PUBLISHED_LAYERS_OPTION, PUBLISHED_READ_CODE, PUBLISHED_SHAPE_CODE,
    RELATION_EXCEPTION_RULES,
};
use crate::models::RulesConfig;
use fensu_policy::policy::types::RuleCodeGrammar;
use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;
use sqlbuild_sqltext::header_strings::main::fold_prose::fold_prose;
use std::path::Path;

pub(crate) fn load(project_dir: &Path) -> Result<RulesConfig, RulesConfigLoadError> {
    let invalid = RulesConfigLoadError::Invalid;
    let path = project_dir.join(PROJECT_CONFIG_FILENAME);
    let source = std::fs::read_to_string(&path).map_err(|error| {
        invalid(format!(
            "could not load rules config from {}: {error}",
            path.display()
        ))
    })?;
    let root: toml::Table = toml::from_str(&source).map_err(|error| {
        invalid(format!(
            "could not load rules config from {}: {error}",
            path.display()
        ))
    })?;
    if root.contains_key("policy") || root.contains_key("lint") || root.contains_key("kata") {
        return Err(invalid(
            "unsupported legacy rules configuration; use [rules]".to_owned(),
        ));
    }
    let mut config = match root.get("rules") {
        None => RulesConfig::default(),
        Some(value) => {
            if let Some(error) = value.as_table().and_then(removed_settings) {
                return Err(RulesConfigLoadError::Policy(error));
            }
            validate_raw(value).map_err(invalid)?;
            value
                .clone()
                .try_into::<RulesConfig>()
                .map_err(|error| invalid(format!("invalid rules config: {error}")))?
        }
    };
    validate(&config).map_err(invalid)?;
    fold_reasons(&mut config);
    Ok(config)
}

/// Every exception and ignore reason as folded prose, as header descriptions are stored.
fn fold_reasons(config: &mut RulesConfig) {
    let reasons = config
        .rule_exceptions
        .iter_mut()
        .map(|entry| &mut entry.reason)
        .chain(
            config
                .graph_edge_exceptions
                .iter_mut()
                .map(|entry| &mut entry.reason),
        )
        .chain(
            config
                .rule_ignores
                .iter_mut()
                .map(|entry| &mut entry.reason),
        )
        .chain(
            config
                .select_star_allow
                .iter_mut()
                .map(|entry| &mut entry.reason),
        )
        .chain(
            config
                .threshold_overrides
                .iter_mut()
                .map(|entry| &mut entry.reason),
        );
    for reason in reasons {
        *reason = fold_prose(reason);
    }
}

fn validate_raw(value: &toml::Value) -> Result<(), String> {
    let table = value
        .as_table()
        .ok_or_else(|| "rules must be a table".to_owned())?;
    let known = [
        "select",
        "ignore",
        "thresholds",
        "threshold_overrides",
        "rule_options",
        "rule_exceptions",
        "graph_edge_exceptions",
        "rule_ignores",
        "select_star_allow",
        "domains",
        "approved_source_tokens",
        "retired_source_tokens",
        "sql_tests",
        "layout",
        "cache",
    ];
    let mut unknown: Vec<String> = table
        .keys()
        .filter(|key| !known.contains(&key.as_str()))
        .cloned()
        .collect();
    unknown.sort();
    if !unknown.is_empty() {
        return Err(format!("unknown rules config keys: {}", unknown.join(", ")));
    }
    for (key, fields) in [
        ("rule_exceptions", ["rule", "path", "reason"].as_slice()),
        (
            "graph_edge_exceptions",
            ["consumer", "dependency", "reason"].as_slice(),
        ),
        ("rule_ignores", ["rules", "reason"].as_slice()),
        ("select_star_allow", ["paths", "reason"].as_slice()),
        (
            "threshold_overrides",
            ["paths", "thresholds", "reason"].as_slice(),
        ),
    ] {
        let Some(entries) = table.get(key).and_then(toml::Value::as_array) else {
            continue;
        };
        for entry in entries {
            let Some(entry_table) = entry.as_table() else {
                continue;
            };
            for field in fields {
                if !entry_table.contains_key(*field) {
                    return Err(format!("rules.{key}.{field} must be a non-empty value"));
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn validate(config: &RulesConfig) -> Result<(), String> {
    const THRESHOLDS: [&str; 12] = [
        "max_literal_length",
        "max_ranking_order_by",
        "min_audits_per_model",
        "min_tests_per_model",
        MIN_CUSTOM_RULE_TEST_CASES_KEY,
        "max_subdomain_depth",
        "min_shared_owner_prefix_directories",
        "max_role_container_depth",
        "max_macro_container_files",
        "max_constant_container_files",
        "max_enum_container_files",
        "min_shared_container_prefix_files",
    ];
    if config.layout.levels.is_empty() {
        return Err(
            "rules.layout.levels must contain at least one normalized relative path".into(),
        );
    }
    let mut levels: std::collections::BTreeSet<&String> = std::collections::BTreeSet::new();
    for level in &config.layout.levels {
        validate_relative_path(level, "rules.layout.levels")?;
        if !levels.insert(level) {
            return Err(format!("duplicate rules layout level: {level}"));
        }
    }
    validate_non_overlapping_paths(&config.layout.levels, "rules.layout.levels")?;
    let mut domain_roots: std::collections::BTreeSet<&String> = std::collections::BTreeSet::new();
    for root in &config.layout.domain_roots {
        validate_relative_path(root, "rules.layout.domain_roots")?;
        if !domain_roots.insert(root) {
            return Err(format!("duplicate rules layout domain root: {root}"));
        }
    }
    validate_non_overlapping_paths(&config.layout.domain_roots, "rules.layout.domain_roots")?;
    let pipeline = Path::new(&config.sql_tests.pipeline_directory);
    if config.sql_tests.pipeline_directory.trim().is_empty()
        || config.sql_tests.pipeline_directory.contains('\\')
        || config.sql_tests.pipeline_directory.contains("//")
        || config.sql_tests.pipeline_directory.ends_with('/')
        || config
            .sql_tests
            .pipeline_directory
            .split('/')
            .next()
            .is_some_and(|component| component.ends_with(':'))
        || pipeline.is_absolute()
        || pipeline
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(
            "rules.sql_tests.pipeline_directory must be a normalized path relative to tests/unit"
                .into(),
        );
    }
    if let Some(name) = config
        .thresholds
        .keys()
        .find(|key| !THRESHOLDS.contains(&key.as_str()))
    {
        return Err(format!("unknown rule thresholds: {name}"));
    }
    if let Some(name) = ["max_literal_length", "max_ranking_order_by"]
        .into_iter()
        .find(|name| config.thresholds.get(*name) == Some(&0))
    {
        return Err(format!(
            "rules.thresholds.{name} must be a positive integer"
        ));
    }
    for entry in &config.threshold_overrides {
        if entry.paths.is_empty() || entry.thresholds.is_empty() || entry.reason.trim().is_empty() {
            return Err("rules.threshold_overrides requires paths, thresholds, and reason".into());
        }
        if let Some(name) = entry
            .thresholds
            .keys()
            .find(|key| !["min_audits_per_model", "min_tests_per_model"].contains(&key.as_str()))
        {
            return Err(format!("unknown rule threshold override: {name}"));
        }
        for pattern in &entry.paths {
            if pattern.trim().is_empty() {
                return Err("rule threshold override paths must be non-empty globs".into());
            }
            globset::Glob::new(pattern).map_err(|error| {
                format!("invalid rule threshold override path {pattern}: {error}")
            })?;
        }
    }
    let grammar = crate::models::RulesCodeGrammar;
    for selector in config.select.iter().flatten().chain(&config.ignore) {
        if !grammar.rule_selector_is_valid(selector) {
            return Err(format!("malformed rule selector: {selector}"));
        }
    }
    for code in config.rule_options.keys() {
        if !grammar.rule_code_is_exact(code) {
            return Err(format!("invalid rule code: {code}"));
        }
    }
    validate_published_layer_options(config)?;
    for exception in &config.rule_exceptions {
        if exception.rule.trim().is_empty()
            || exception.path.trim().is_empty()
            || exception.reason.trim().is_empty()
        {
            return Err("rules.rule_exceptions fields must be non-empty strings".into());
        }
        if !grammar.rule_code_is_exact(&exception.rule) {
            return Err(format!("invalid rule code: {}", exception.rule));
        }
        validate_relation_scope(exception)?;
    }
    let mut graph_edges: std::collections::BTreeSet<(&str, &str)> =
        std::collections::BTreeSet::new();
    for exception in &config.graph_edge_exceptions {
        if exception.consumer.trim().is_empty()
            || exception.dependency.trim().is_empty()
            || exception.reason.trim().is_empty()
        {
            return Err("rules.graph_edge_exceptions fields must be non-empty strings".into());
        }
        if !graph_edges.insert((&exception.consumer, &exception.dependency)) {
            return Err(format!(
                "duplicate graph edge exception: {} -> {}",
                exception.consumer, exception.dependency
            ));
        }
    }
    for ignore in &config.rule_ignores {
        if ignore.rules.is_empty()
            || (ignore.paths.is_empty() && ignore.selectors.is_empty())
            || ignore.reason.trim().is_empty()
        {
            return Err("rules.rule_ignores requires rules, paths or selectors, and reason".into());
        }
        if let Some(selector) = ignore
            .rules
            .iter()
            .find(|selector| !grammar.rule_selector_is_valid(selector))
        {
            return Err(format!("malformed rule selector: {selector}"));
        }
        if ignore.paths.iter().any(|path| path.trim().is_empty()) {
            return Err("rule ignore paths must be non-empty".into());
        }
        if ignore
            .selectors
            .iter()
            .any(|selector| selector.trim().is_empty())
        {
            return Err("rule ignore selectors must be non-empty".into());
        }
    }
    for allow in &config.select_star_allow {
        if allow.paths.is_empty() || allow.reason.trim().is_empty() {
            return Err("rules.select_star_allow requires paths and reason".into());
        }
    }
    Ok(())
}

/// Published layers name model layers; allowed qualifiers are literal and carry a reason.
fn validate_published_layer_options(config: &RulesConfig) -> Result<(), String> {
    for code in [PUBLISHED_READ_CODE, PUBLISHED_SHAPE_CODE] {
        let Some(layers) = config
            .rule_options
            .get(code)
            .and_then(|options| options.get(PUBLISHED_LAYERS_OPTION))
        else {
            continue;
        };
        let label: String = format!("rules.rule_options.{code}.{PUBLISHED_LAYERS_OPTION}");
        let Some(layers) = layers.as_array() else {
            return Err(format!("{label} must be a list of model name layers"));
        };
        for layer in layers {
            match layer.as_str() {
                Some(name) if MODEL_NAME_LAYERS.contains(&name) => {}
                _ => {
                    return Err(format!(
                        "{label} entries must be model name layers: {}",
                        MODEL_NAME_LAYERS.join(", ")
                    ));
                }
            }
        }
    }
    let Some(entries) = config
        .rule_options
        .get(PUBLISHED_SHAPE_CODE)
        .and_then(|options| options.get(ALLOWED_QUALIFIERS_OPTION))
    else {
        return Ok(());
    };
    let label: String =
        format!("rules.rule_options.{PUBLISHED_SHAPE_CODE}.{ALLOWED_QUALIFIERS_OPTION}");
    let shape: String = format!(
        "{label} entries must be {{ qualifier = \"...\", reason = \"...\" }} or \
         {{ paths = [\"...\"], reason = \"...\" }} tables with an optional domain"
    );
    let Some(entries) = entries.as_array() else {
        return Err(shape);
    };
    let mut seen: std::collections::BTreeSet<(Option<&str>, Option<&str>, Vec<&str>)> =
        std::collections::BTreeSet::new();
    for entry in entries {
        let Some(table) = entry.as_object() else {
            return Err(shape);
        };
        if table.keys().any(|key| {
            !["domain", "qualifier", ALLOWED_PATHS_KEY, "reason"].contains(&key.as_str())
        }) {
            return Err(shape);
        }
        let text = |key: &str| table.get(key).map(|value| value.as_str().map(str::trim));
        let Some(Some(reason)) = text("reason") else {
            return Err(shape);
        };
        let domain: Option<&str> = match text("domain") {
            None => None,
            Some(Some(domain)) if !domain.is_empty() => Some(domain),
            Some(_) => return Err(shape),
        };
        let qualifier: Option<&str> = match text("qualifier") {
            None => None,
            Some(Some(qualifier)) => Some(qualifier),
            Some(None) => return Err(shape),
        };
        let paths: Option<Vec<&str>> = match table.get(ALLOWED_PATHS_KEY) {
            None => None,
            Some(paths) => {
                let Some(paths) = paths.as_array() else {
                    return Err(shape);
                };
                let paths: Option<Vec<&str>> = paths
                    .iter()
                    .map(|path| path.as_str().map(str::trim))
                    .collect();
                let Some(mut paths) = paths else {
                    return Err(shape);
                };
                paths.sort_unstable();
                Some(paths)
            }
        };
        if qualifier.is_none() && paths.is_none() {
            return Err(shape);
        }
        if reason.is_empty() {
            return Err(format!("{label} entries need a non-empty reason"));
        }
        if qualifier.is_some_and(str::is_empty) {
            return Err(format!("{label} entries need a non-empty qualifier"));
        }
        if let Some(paths) = &paths {
            validate_allowed_paths(paths, &label)?;
        }
        let paths: Vec<&str> = paths.unwrap_or_default();
        let described: String = match qualifier {
            Some(qualifier) => qualifier.to_owned(),
            None => paths.join(", "),
        };
        if !seen.insert((domain, qualifier, paths)) {
            return Err(format!("duplicate {label} entry: {described}"));
        }
    }
    Ok(())
}

/// One entry's sorted `paths` globs: non-empty, `/`-separated and each listed once.
fn validate_allowed_paths(paths: &[&str], label: &str) -> Result<(), String> {
    if paths.is_empty() || paths.iter().any(|path| path.is_empty()) {
        return Err(format!(
            "{label} entries need a non-empty list of non-empty paths"
        ));
    }
    if let Some(path) = paths.iter().find(|path| path.contains('\\')) {
        return Err(format!(
            "{label} paths must be project-relative globs with / separators: {path}"
        ));
    }
    if let Some(pair) = paths.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(format!("{label} entry lists path {} twice", pair[0]));
    }
    Ok(())
}

/// A relation-level exception names a non-empty relation, for a Rule that reports one.
fn validate_relation_scope(exception: &crate::models::RuleException) -> Result<(), String> {
    let blank = |value: &Option<String>| value.as_ref().is_some_and(|text| text.trim().is_empty());
    if blank(&exception.relation) || blank(&exception.cte) {
        return Err("rules.rule_exceptions relation and cte must be non-empty strings".into());
    }
    if exception.cte.is_some() && exception.relation.is_none() {
        return Err(format!(
            "rules.rule_exceptions entry for {} at {} sets cte without relation; cte narrows a \
             relation-level exception",
            exception.rule, exception.path
        ));
    }
    if exception.relation.is_some() && !RELATION_EXCEPTION_RULES.contains(&exception.rule.as_str())
    {
        return Err(format!(
            "rules.rule_exceptions entry for {} at {} sets relation, but only {} report a joined \
             relation; remove relation to cover the whole file",
            exception.rule,
            exception.path,
            RELATION_EXCEPTION_RULES.join(", ")
        ));
    }
    Ok(())
}

fn validate_relative_path(value: &str, label: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.trim().is_empty()
        || value.contains('\\')
        || value.contains("//")
        || value.ends_with('/')
        || value
            .split('/')
            .next()
            .is_some_and(|component| component.ends_with(':'))
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(format!(
            "{label} entries must be normalized project-relative paths"
        ));
    }
    Ok(())
}

fn validate_non_overlapping_paths(values: &[String], label: &str) -> Result<(), String> {
    for (index, left) in values.iter().enumerate() {
        for right in values.iter().skip(index + 1) {
            let left_prefix = format!("{left}/");
            let right_prefix = format!("{right}/");
            if left.starts_with(&right_prefix) || right.starts_with(&left_prefix) {
                return Err(format!("{label} entries must not overlap: {left}, {right}"));
            }
        }
    }
    Ok(())
}
