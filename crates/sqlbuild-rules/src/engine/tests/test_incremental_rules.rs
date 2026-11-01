use std::collections::BTreeMap;

use serde_json::{Value, json};
use tempfile::TempDir;

use crate::engine::_helpers::evaluation::parse_parts;
use crate::engine::tests::helpers::{
    cached_paths, evaluate_split, evaluate_whole, orders_models, split_request, uncached,
    write_model_files,
};
use crate::engine::tests::test_types;
use crate::models::RuleScope;
use crate::rules::main::catalogue::catalogue;

const REVIEWED_SCOPES: &[(&str, RuleScope)] = &[
    ("SQBRCONTRACT101", RuleScope::Model),
    ("SQBRCONTRACT102", RuleScope::Model),
    ("SQBRCONTRACT103", RuleScope::Model),
    ("SQBRCONTRACT104", RuleScope::Model),
    ("SQBRCONTRACT105", RuleScope::Model),
    ("SQBRCONTRACT106", RuleScope::Model),
    ("SQBRDECLARATION101", RuleScope::Model),
    ("SQBRDECLARATION102", RuleScope::Model),
    ("SQBRDECLARATION201", RuleScope::Project),
    ("SQBRDECLARATION301", RuleScope::Project),
    ("SQBRDECLARATION302", RuleScope::Project),
    ("SQBRDECLARATION303", RuleScope::Project),
    ("SQBRDECLARATION304", RuleScope::Project),
    ("SQBRDECLARATION305", RuleScope::Project),
    ("SQBRDECLARATION306", RuleScope::Project),
    ("SQBRGRAPH101", RuleScope::Project),
    ("SQBRGRAPH102", RuleScope::Model),
    ("SQBRGRAPH103", RuleScope::Project),
    ("SQBRMODEL101", RuleScope::Model),
    ("SQBRMODEL102", RuleScope::Model),
    ("SQBRMODEL103", RuleScope::Model),
    ("SQBRMODEL104", RuleScope::Model),
    ("SQBRPROJECT101", RuleScope::Model),
    ("SQBRPROJECT102", RuleScope::Model),
    ("SQBRPROJECT103", RuleScope::Model),
    ("SQBRPROJECT104", RuleScope::Model),
    ("SQBRPROJECT105", RuleScope::Model),
    ("SQBRPROJECT106", RuleScope::Model),
    ("SQBRPROJECT107", RuleScope::Project),
    ("SQBRPROJECT201", RuleScope::Project),
    ("SQBRPROJECT202", RuleScope::Project),
    ("SQBRPROJECT203", RuleScope::Project),
    ("SQBRPROJECT204", RuleScope::Project),
    ("SQBRTEST101", RuleScope::Project),
    ("SQBRTEST102", RuleScope::Project),
    ("SQBRTEST103", RuleScope::Project),
    ("SQBRTEST104", RuleScope::Project),
    ("SQBRTEST201", RuleScope::ModelAndProject),
    ("SQBRTEST202", RuleScope::ModelAndProject),
    ("SQBRTEST203", RuleScope::Project),
    ("SQBRTEST301", RuleScope::Project),
];

#[test]
fn given_built_in_catalogue_when_reading_rule_scopes_then_every_rule_matches_reviewed_scope() {
    let test_cases = [test_types::RuleScopeTestCase {
        description: "every built-in rule declares its reviewed scope",
        expected_scopes: REVIEWED_SCOPES,
        expected_sql_scope: RuleScope::SqlLint,
    }];
    for test_case in &test_cases {
        let declared: BTreeMap<String, RuleScope> = catalogue()
            .into_iter()
            .filter(|rule| !rule.code.starts_with("SQLCLEAN"))
            .map(|rule| (rule.code, rule.scope))
            .collect();
        let declared: Vec<(&str, RuleScope)> = declared
            .iter()
            .map(|(code, scope)| (code.as_str(), *scope))
            .collect();
        let sql_scopes_match = catalogue()
            .iter()
            .filter(|rule| rule.code.starts_with("SQLCLEAN"))
            .all(|rule| rule.scope == test_case.expected_sql_scope);

        assert_eq!(
            declared, test_case.expected_scopes,
            "{}",
            test_case.description
        );
        assert!(sql_scopes_match, "{}", test_case.description);
    }
}

#[test]
fn given_every_built_in_rule_when_evaluating_then_findings_stay_inside_declared_scopes()
-> Result<(), String> {
    let test_cases = [test_types::ScopedEvaluationTestCase {
        description: "model and project findings come from scoped phases",
        expected_codes: &["SQBRCONTRACT101", "SQBRGRAPH102", "SQBRTEST202"],
    }];
    for test_case in &test_cases {
        let project_dir = TempDir::new().map_err(|error| error.to_string())?;
        let rest = split_request(
            &project_dir,
            &json!({"select": ["SQBR"], "domains": ["commerce"], "cache": {"enabled": false}}),
        );

        let result = evaluate_whole(&rest, &orders_models())?;
        let codes: Vec<&str> = result["faults"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|fault| fault["code"].as_str())
            .collect();

        let found: Vec<&str> = test_case
            .expected_codes
            .iter()
            .copied()
            .filter(|code| codes.contains(code))
            .collect();

        assert_eq!(found, test_case.expected_codes, "{}", test_case.description);
    }
    Ok(())
}

#[test]
fn given_split_model_payloads_when_evaluating_then_response_matches_single_request()
-> Result<(), String> {
    let test_cases = [
        test_types::IncrementalRulesTestCase {
            description: "uncached split request",
            cache_enabled: false,
            compared_fields: &["faults", "selected_codes", "evaluated_models"],
            expected_mismatched_fields: &[],
        },
        test_types::IncrementalRulesTestCase {
            description: "cached split request",
            cache_enabled: true,
            compared_fields: &["faults", "selected_codes", "evaluated_models"],
            expected_mismatched_fields: &[],
        },
    ];
    for test_case in &test_cases {
        let whole_dir = TempDir::new().map_err(|error| error.to_string())?;
        let split_dir = TempDir::new().map_err(|error| error.to_string())?;
        let config = json!({"select": ["SQBR"], "cache": {"enabled": test_case.cache_enabled}});

        let whole = evaluate_whole(&split_request(&whole_dir, &config), &orders_models())?;
        let split = evaluate_split(&split_request(&split_dir, &config), &orders_models())?;

        let mismatched: Vec<&str> = test_case
            .compared_fields
            .iter()
            .copied()
            .filter(|field| whole[field] != split[field])
            .collect();

        assert_eq!(
            mismatched, test_case.expected_mismatched_fields,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_malformed_split_request_when_evaluating_then_request_is_rejected() -> Result<(), String> {
    let test_cases = [
        test_types::MalformedSplitRequestTestCase {
            description: "models embedded in the request",
            inline_model_count: 3,
            payload_count: 0,
            expected_error: "separate payloads",
        },
        test_types::MalformedSplitRequestTestCase {
            description: "payload without a digest",
            inline_model_count: 0,
            payload_count: 1,
            expected_error: "one digest",
        },
    ];
    for test_case in &test_cases {
        let project_dir = TempDir::new().map_err(|error| error.to_string())?;
        let mut request = split_request(&project_dir, &json!({"select": ["SQBR"]}));
        request["models"] = Value::Array(
            orders_models()
                .into_iter()
                .take(test_case.inline_model_count)
                .collect(),
        );
        let rest = serde_json::to_vec(&request).map_err(|error| error.to_string())?;
        let model = serde_json::to_vec(&orders_models()[0]).map_err(|error| error.to_string())?;
        let payloads: Vec<&[u8]> = vec![model.as_slice(); test_case.payload_count];

        let error = parse_parts(&rest, &payloads, &[]);

        assert!(
            error.is_err_and(|message| message.contains(test_case.expected_error)),
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_cached_models_when_editing_one_model_then_only_it_reevaluates_with_uncached_findings()
-> Result<(), String> {
    let test_cases = [test_types::ModelCacheReuseTestCase {
        description: "one edited model misses while the others hit",
        expected_hits: 2,
        expected_misses: 1,
    }];
    for test_case in &test_cases {
        let project_dir = TempDir::new().map_err(|error| error.to_string())?;
        let rest = split_request(
            &project_dir,
            &json!({"select": ["SQBR"], "cache": {"enabled": true}}),
        );
        let mut models = orders_models();
        write_model_files(&project_dir, &models)?;
        let _ = evaluate_split(&rest, &models)?;
        models[1]["query_sql"] = json!("SELECT * FROM warehouse.raw.customers");
        models[1]["authored_sql"] = models[1]["query_sql"].clone();

        let edited = evaluate_split(&rest, &models)?;
        let expected = evaluate_split(&uncached(&rest), &models)?;

        assert_eq!(
            edited["cache_hits"], test_case.expected_hits,
            "{}",
            test_case.description
        );
        assert_eq!(
            edited["cache_misses"], test_case.expected_misses,
            "{}",
            test_case.description
        );
        assert_eq!(
            edited["faults"], expected["faults"],
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_suppression_change_when_evaluating_then_cached_models_are_reused_with_uncached_findings()
-> Result<(), String> {
    let test_cases = [test_types::ModelCacheReuseTestCase {
        description: "a scoped ignore does not invalidate model findings",
        expected_hits: 3,
        expected_misses: 0,
    }];
    for test_case in &test_cases {
        let project_dir = TempDir::new().map_err(|error| error.to_string())?;
        let models = orders_models();
        write_model_files(&project_dir, &models)?;
        let mut rest = split_request(
            &project_dir,
            &json!({"select": ["SQBR"], "cache": {"enabled": true}}),
        );
        rest["defer_suppressions"] = json!(false);
        let _ = evaluate_split(&rest, &models)?;
        rest["config"]["rule_ignores"] = json!([{
            "rules": ["SQBRGRAPH102"],
            "paths": ["models/commerce/mart/commerce__mart__orders.sql"],
            "reason": "legacy warehouse table"
        }]);

        let ignored = evaluate_split(&rest, &models)?;
        let expected = evaluate_split(&uncached(&rest), &models)?;

        assert_eq!(
            ignored["cache_hits"], test_case.expected_hits,
            "{}",
            test_case.description
        );
        assert_eq!(
            ignored["cache_misses"], test_case.expected_misses,
            "{}",
            test_case.description
        );
        assert_eq!(
            ignored["faults"], expected["faults"],
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_removed_model_file_when_cache_is_rewritten_then_only_its_entry_is_dropped()
-> Result<(), String> {
    let test_cases = [test_types::CachePruningTestCase {
        description: "deleted model is pruned while an unselected model is kept",
        expected_paths: &[
            "models/commerce/mart/commerce__mart__customers.sql",
            "models/commerce/staging/commerce__stg__products.sql",
        ],
    }];
    for test_case in &test_cases {
        let project_dir = TempDir::new().map_err(|error| error.to_string())?;
        let rest = split_request(
            &project_dir,
            &json!({"select": ["SQBR"], "cache": {"enabled": true}}),
        );
        let mut models = orders_models();
        write_model_files(&project_dir, &models)?;
        let _ = evaluate_split(&rest, &models)?;
        let removed = models.remove(0);
        std::fs::remove_file(
            project_dir
                .path()
                .join(removed["relative_path"].as_str().unwrap_or_default()),
        )
        .map_err(|error| error.to_string())?;
        let _unselected = models.remove(0);
        models[0]["query_sql"] = json!("SELECT 2 AS product_id");
        models[0]["authored_sql"] = models[0]["query_sql"].clone();

        let _ = evaluate_split(&rest, &models)?;

        assert_eq!(
            cached_paths(&project_dir)?,
            test_case.expected_paths,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_configured_cache_path_when_evaluating_then_only_that_file_is_used() -> Result<(), String> {
    let test_cases = [test_types::ConfiguredCachePathTestCase {
        description: "an engine-owned cache file replaces the default bulk cache",
        cache_file: "target/rules-cache-native-v1/bulk/native.json",
        expected_default_cache_exists: false,
        expected_warm_hits: 3,
    }];
    for test_case in &test_cases {
        let project_dir = TempDir::new().map_err(|error| error.to_string())?;
        let mut rest = split_request(
            &project_dir,
            &json!({"select": ["SQBR"], "cache": {"enabled": true}}),
        );
        let cache_path = project_dir.path().join(test_case.cache_file);
        rest["rules_cache_path"] = json!(cache_path);
        let models = orders_models();

        let cold = evaluate_split(&rest, &models)?;
        let warm = evaluate_split(&rest, &models)?;

        assert!(cache_path.is_file(), "{}", test_case.description);
        assert_eq!(
            project_dir
                .path()
                .join("target/rules-cache/bulk/native.json")
                .exists(),
            test_case.expected_default_cache_exists,
            "{}",
            test_case.description
        );
        assert_eq!(
            warm["cache_hits"], test_case.expected_warm_hits,
            "{}",
            test_case.description
        );
        assert_eq!(warm["faults"], cold["faults"], "{}", test_case.description);
    }
    Ok(())
}

#[test]
fn given_corrupt_model_cache_when_evaluating_then_every_model_reevaluates() -> Result<(), String> {
    let test_cases = [test_types::ModelCacheReuseTestCase {
        description: "a truncated cache file is a miss for every model",
        expected_hits: 0,
        expected_misses: 3,
    }];
    for test_case in &test_cases {
        let project_dir = TempDir::new().map_err(|error| error.to_string())?;
        let rest = split_request(
            &project_dir,
            &json!({"select": ["SQBR"], "cache": {"enabled": true}}),
        );
        let models = orders_models();
        let expected = evaluate_split(&rest, &models)?;
        std::fs::write(
            project_dir
                .path()
                .join("target/rules-cache/bulk/native.json"),
            b"{\"fingerprint\": \"truncated",
        )
        .map_err(|error| error.to_string())?;

        let recovered = evaluate_split(&rest, &models)?;

        assert_eq!(
            recovered["cache_hits"], test_case.expected_hits,
            "{}",
            test_case.description
        );
        assert_eq!(
            recovered["cache_misses"], test_case.expected_misses,
            "{}",
            test_case.description
        );
        assert_eq!(
            recovered["faults"], expected["faults"],
            "{}",
            test_case.description
        );
    }
    Ok(())
}
