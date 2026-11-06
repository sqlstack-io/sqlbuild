use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

use std::sync::atomic::AtomicBool;

use crate::engine::main::answer_speculative_rows::answer_speculative_rows;
use crate::engine::main::discard_speculative_rows::discard_speculative_rows;
use crate::engine::main::retained_request::retained_request;
use crate::models::{
    Column, Declaration, DeclarationMember, DynamicColumnFamily, EvaluateRequest, Fault, Model,
    Reference, RetainedRows, RowsEvaluation, RowsRequest, RulesConfig, SpeculativeAnswer,
    SpeculativeRows,
};

use crate::engine::_helpers::evaluation::evaluate_json;
use crate::engine::_helpers::evaluation::evaluate_request as evaluate_parsed;
use crate::engine::_helpers::evaluation::parse_parts;
use crate::engine::main::finalize_rows::finalize_rows;
use crate::engine::tests::test_types;

pub(crate) fn request(project_dir: &TempDir, config: &Value) -> String {
    json!({
        "version": 1,
        "project_dir": project_dir.path(),
        "config": config,
        "models": [{
            "name": "commerce__mart__orders",
            "relative_path": "models/mart/commerce__mart__orders.sql",
            "query_sql": "SELECT * FROM orders",
            "authored_sql": "SELECT * FROM orders"
        }]
    })
    .to_string()
}

pub(crate) fn model(path: &str) -> Value {
    json!({
        "name": "commerce__mart__example",
        "relative_path": path,
        "query_sql": "SELECT 1",
        "authored_sql": "SELECT 1"
    })
}

pub(crate) fn domain_layout_evaluation(
    project_dir: &TempDir,
    code: &str,
    models: Value,
    thresholds: Value,
    layout: Value,
    scope_index: Value,
) -> Result<Value, String> {
    let request = json!({
        "version": 1,
        "project_dir": project_dir.path(),
        "config": {
            "select": [code],
            "thresholds": thresholds,
            "layout": layout,
            "cache": {"enabled": false}
        },
        "models": models,
        "scope_index": scope_index
    });
    serde_json::from_str(&evaluate_json(&request.to_string())?).map_err(|error| error.to_string())
}

pub(crate) fn scope_with_macros(paths: &[String]) -> Value {
    let mut scope = scope_index();
    scope["declarations"] = Value::Array(
        paths
            .iter()
            .enumerate()
            .map(|(index, path)| {
                let relative = path.trim_start_matches("models/commerce/_macros/");
                let relative = relative
                    .strip_prefix("python/")
                    .or_else(|| relative.strip_prefix("sql/"))
                    .unwrap_or(relative);
                let bucket_path = std::path::Path::new(relative)
                    .parent()
                    .and_then(std::path::Path::to_str)
                    .filter(|value| !value.is_empty());
                json!({
                    "identity": format!("macro:item_{index}"),
                    "kind": "macro",
                    "name": format!("item_{index}"),
                    "owner": null,
                    "path": path,
                    "line": 1,
                    "column": 1,
                    "scope": "local",
                    "role": "macros",
                    "visibility": "exact_owner_private",
                    "role_root": "models/commerce/_macros",
                    "bucket_path": bucket_path,
                    "ownership_root": "models",
                    "owning_path": "models/race",
                    "metadata": {"macro": {
                        "parameters": [],
                        "dependencies": [],
                        "source_digest": format!("digest_{index}")
                    }}
                })
            })
            .collect(),
    );
    scope["usages"] = json!([]);
    scope["visibility"] = json!([]);
    scope
}

pub(crate) fn threshold_request(
    project_dir: &TempDir,
    config: &Value,
    query_sql: &str,
    references: &Value,
) -> String {
    json!({
        "version": 1,
        "project_dir": project_dir.path(),
        "config": config,
        "models": [{
            "name": "commerce__mart__orders",
            "relative_path": "models/mart/commerce__mart__orders.sql",
            "query_sql": query_sql,
            "authored_sql": query_sql,
            "references": references,
            "declared_audit_count": 1,
            "targeting_test_count": 1
        }]
    })
    .to_string()
}

pub(crate) fn scope_index() -> Value {
    json!({
        "schema_version": 3,
        "ownership_roots": [{"path": "models", "resource_kind": "model"}],
        "resources": [{
            "identity": "model:orders",
            "kind": "model",
            "name": "orders",
            "path": "models/orders.sql",
            "ownership_root": "models",
            "ownership_root_kind": "model"
        }],
        "declarations": [{
            "identity": "enum:status",
            "kind": "enum",
            "name": "status",
            "owner": null,
            "path": "enums/status.sql",
            "line": 1,
            "column": 1,
            "scope": "global",
            "role": "enums",
            "visibility": "project",
            "role_root": "enums",
            "bucket_path": null,
            "ownership_root": "enums",
            "owning_path": null,
            "metadata": {"enum": {
                "members": [{"name": "OPEN"}],
                "scalar_type": "VARCHAR"
            }}
        }, {
            "identity": "macro:normalize_status",
            "kind": "macro",
            "name": "normalize_status",
            "owner": null,
            "path": "macros/normalize_status.py",
            "line": 1,
            "column": 1,
            "scope": "global",
            "role": "macros",
            "visibility": "project",
            "role_root": "macros",
            "bucket_path": null,
            "ownership_root": "macros",
            "owning_path": null,
            "metadata": {"macro": {
                "parameters": ["status"],
                "dependencies": ["enum:status"],
                "source_digest": "digest"
            }}
        }],
        "usages": [{
            "consumer": "model:orders",
            "declaration": "enum:status",
            "kind": "runtime",
            "through": null,
            "enum_member": "OPEN"
        }],
        "grants": [{
            "resource": "model:orders",
            "declaration": "enum:status",
            "through": "model:expected_orders",
            "kind": "expected_model"
        }],
        "visibility": [{
            "resource": "model:orders",
            "declaration": "enum:status",
            "reason": "global",
            "through": null
        }],
        "inaccessible": [{
            "resource": "model:orders",
            "declaration": "constant:private_limit",
            "reason": "private_owner"
        }],
        "diagnostics": [],
        "complete": true,
        "completeness": {
            "discovery": true,
            "static_visibility": true,
            "runtime_usage": true,
            "relationships": true,
            "placement": true,
            "promotion_impact": true
        }
    })
}

pub(crate) fn request_with_scope(scope: Value) -> Value {
    json!({
        "version": 1,
        "project_dir": ".",
        "config": {},
        "models": [],
        "public_enums": [],
        "public_constants": [],
        "custom_rules": [],
        "scope_index": scope
    })
}

pub(crate) fn sql_test_rules_evaluation(
    project_dir: &TempDir,
    code: &str,
    tests: Value,
    scenarios: Value,
    scope_index: Value,
    extra_config: Value,
) -> Result<Value, String> {
    let mut config = json!({
        "select": [code],
        "cache": {"enabled": false}
    });
    config
        .as_object_mut()
        .expect("base rules config is an object")
        .extend(
            extra_config
                .as_object()
                .expect("extra rules config is an object")
                .clone(),
        );
    let request = json!({
        "version": 1,
        "project_dir": project_dir.path(),
        "config": config,
        "models": [],
        "sql_tests": tests,
        "sql_scenarios": scenarios,
        "scope_index": scope_index
    });
    serde_json::from_str(&evaluate_json(&request.to_string())?).map_err(|error| error.to_string())
}

pub(crate) fn sql_test_fact(path: &str, name: Option<&str>, targets: Value) -> Value {
    json!({
        "source_path": path,
        "ownership_root": "tests/unit",
        "block_index": 1,
        "name": name.unwrap_or("test_orders"),
        "explicit_name": name,
        "mode": "model",
        "expected_model_names": targets,
        "assertion_names": [],
        "assertion_target_model_names": [],
        "target_model_names": targets,
        "tested_resources": []
    })
}

pub(crate) fn cached_sql_test_rules_request(project_dir: &TempDir, name: Option<&str>) -> String {
    json!({
        "version": 1,
        "project_dir": project_dir.path(),
        "config": {"select": ["SQBRTEST104"], "cache": {"enabled": true}},
        "models": [{
            "name": "orders", "relative_path": "models/orders.sql",
            "query_sql": "SELECT 1", "authored_sql": "SELECT 1"
        }],
        "sql_tests": [sql_test_fact(
            "tests/unit/test_orders__paid.sql", name, json!(["orders"])
        )],
        "sql_scenarios": [],
        "scope_index": scope_index()
    })
    .to_string()
}

pub(crate) fn empty_input_test_fact(
    name: &str,
    mocks: &[(&str, &str)],
    expected: &[(&str, &str)],
    assertions: &[(&str, &str)],
) -> Value {
    let ctes = |items: &[(&str, &str)]| -> Value {
        Value::Array(
            items
                .iter()
                .map(|(cte_name, sql)| json!({"name": cte_name, "sql": sql}))
                .collect(),
        )
    };
    let mut fact = sql_test_fact(
        &format!("tests/unit/test_{name}.sql"),
        Some(name),
        json!(["orders"]),
    );
    fact["authored_ctes"] = ctes(mocks);
    fact["expected_ctes"] = ctes(expected);
    fact["assertion_ctes"] = ctes(assertions);
    fact
}

pub(crate) fn empty_input_rule_evaluation(
    project_dir: &TempDir,
    select: Value,
    tests: Value,
    allowed_tests: Value,
    cache_enabled: bool,
) -> Result<Vec<String>, String> {
    dialect_empty_input_rule_evaluation(
        "duckdb",
        project_dir,
        select,
        tests,
        allowed_tests,
        cache_enabled,
    )
}

pub(crate) fn dialect_empty_input_rule_evaluation(
    dialect: &str,
    project_dir: &TempDir,
    select: Value,
    tests: Value,
    allowed_tests: Value,
    cache_enabled: bool,
) -> Result<Vec<String>, String> {
    let request = json!({
        "version": 1,
        "project_dir": project_dir.path(),
        "dialect": dialect,
        "config": {
            "select": select,
            "thresholds": {"min_tests_per_model": 1},
            "rule_options": {"SQBRTEST203": {"allowed_tests": allowed_tests}},
            "cache": {"enabled": cache_enabled}
        },
        "models": [{
            "name": "orders",
            "relative_path": "models/orders.sql",
            "query_sql": "SELECT order_id, amount * 2 AS doubled_amount FROM raw_orders",
            "authored_sql": "SELECT order_id, amount * 2 AS doubled_amount FROM raw_orders",
            "targeting_test_count": tests.as_array().map_or(0, Vec::len)
        }],
        "sql_tests": tests,
        "sql_scenarios": [],
        "scope_index": scope_index()
    });
    let result: Value = serde_json::from_str(&evaluate_json(&request.to_string())?)
        .map_err(|error| error.to_string())?;
    Ok(result["faults"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|fault| {
            format!(
                "{} {}: {}",
                fault["code"].as_str().unwrap_or_default(),
                fault["path"].as_str().unwrap_or_default(),
                fault["message"].as_str().unwrap_or_default()
            )
        })
        .collect())
}

pub(crate) fn filler_test() -> Value {
    empty_input_test_fact(
        "orders__empty_inputs_produce_no_rows",
        &[
            (
                "__source__raw_orders",
                "SELECT NULL AS order_id, NULL AS amount WHERE FALSE",
            ),
            ("__ref__customers", "SELECT NULL AS customer_id WHERE 1 = 0"),
        ],
        &[],
        &[(
            "__assert__empty_inputs_produce_no_rows",
            "SELECT 1 AS unexpected_row FROM __ref(\"orders\")",
        )],
    )
}

pub(crate) fn with_field(mut fact: Value, field: &str, value: Value) -> Value {
    fact[field] = value;
    fact
}

pub(crate) fn orders_model(path: &str, query: &str) -> Value {
    let name = path
        .rsplit('/')
        .next()
        .and_then(|file| file.strip_suffix(".sql"))
        .unwrap_or("commerce__mart__orders");
    json!({
        "name": name,
        "relative_path": path,
        "query_sql": query,
        "authored_sql": query,
        "config": {"materialized": "table"},
        "references": [{"ref_kind": "ref", "ref_name": "commerce__int_clean__orders"}],
        "targeting_test_count": 0
    })
}

pub(crate) fn orders_models() -> Vec<Value> {
    vec![
        orders_model(
            "models/commerce/mart/commerce__mart__orders.sql",
            "SELECT * FROM warehouse.raw.orders",
        ),
        orders_model(
            "models/commerce/mart/commerce__mart__customers.sql",
            "WITH customers AS (SELECT 1 AS customer_id) SELECT customer_id FROM customers",
        ),
        orders_model(
            "models/commerce/staging/commerce__stg__products.sql",
            "SELECT product_id, 1 AS quantity FROM products WHERE status = 3",
        ),
    ]
}

pub(crate) fn split_request(project_dir: &TempDir, config: &Value) -> Value {
    json!({
        "version": 1,
        "project_dir": project_dir.path(),
        "dialect": "duckdb",
        "config": config,
        "models": [],
        "defer_suppressions": true
    })
}

pub(crate) fn evaluate_split(rest: &Value, models: &[Value]) -> Result<Value, String> {
    let encoded: Vec<Vec<u8>> = models
        .iter()
        .map(|model| serde_json::to_vec(model).map_err(|error| error.to_string()))
        .collect::<Result<_, _>>()?;
    let payloads: Vec<&[u8]> = encoded.iter().map(Vec::as_slice).collect();
    let digests: Vec<String> = payloads
        .iter()
        .map(|payload| format!("{:x}", Sha256::digest(payload)))
        .collect();
    let rest_json = serde_json::to_vec(rest).map_err(|error| error.to_string())?;
    serde_json::from_str(&evaluate_parsed(parse_parts(
        &rest_json, &payloads, &digests,
    )?)?)
    .map_err(|error| error.to_string())
}

pub(crate) fn evaluate_whole(rest: &Value, models: &[Value]) -> Result<Value, String> {
    let mut request = rest.clone();
    request["models"] = Value::Array(models.to_vec());
    serde_json::from_str(&evaluate_json(&request.to_string())?).map_err(|error| error.to_string())
}

pub(crate) fn uncached(rest: &Value) -> Value {
    let mut request = rest.clone();
    request["config"]["cache"] = json!({"enabled": false});
    request
}

pub(crate) fn write_model_files(project_dir: &TempDir, models: &[Value]) -> Result<(), String> {
    for model in models {
        let path = project_dir
            .path()
            .join(model["relative_path"].as_str().unwrap_or_default());
        let parent = path
            .parent()
            .ok_or_else(|| "model path has no parent".to_owned())?;
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        std::fs::write(&path, "SELECT 1").map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(crate) fn cached_paths(project_dir: &TempDir) -> Result<Vec<String>, String> {
    let bytes = std::fs::read(
        project_dir
            .path()
            .join("target/rules-cache/bulk/native.json"),
    )
    .map_err(|error| error.to_string())?;
    let bucket: Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    Ok(bucket["entries"]
        .as_object()
        .map(|entries| entries.keys().cloned().collect())
        .unwrap_or_default())
}

const SPECULATIVE_CODE_IDENTITY: &str = "speculative-rows-test-build";
pub(crate) const SPECULATIVE_CACHE_FILE: &str = "target/rules-cache/bulk/native.json";
const SPECULATIVE_MEMO_FILE: &str = "target/rules-cache/bulk/native-response.json";

pub(crate) fn speculative_rows_request(
    project_dir: &Path,
    cache_enabled: bool,
    models: &[Value],
) -> Result<RowsRequest, String> {
    let mut request: Value = speculative_request_json(project_dir, cache_enabled);
    request["models"] = Value::Array(models.to_vec());
    let request: EvaluateRequest =
        serde_json::from_value(request).map_err(|error| error.to_string())?;
    Ok(RowsRequest {
        request,
        memo: cache_enabled.then(|| {
            (
                SPECULATIVE_CODE_IDENTITY.to_owned(),
                project_dir.join(SPECULATIVE_MEMO_FILE),
            )
        }),
        retain: false,
    })
}

fn speculative_request_json(project_dir: &Path, cache_enabled: bool) -> Value {
    json!({
        "version": 1,
        "project_dir": project_dir,
        "dialect": "duckdb",
        "config": {"select": ["SQBR"], "domains": ["commerce"], "cache": {"enabled": cache_enabled}},
        "models": [],
        "defer_suppressions": true
    })
}

pub(crate) fn changed_orders_models(query: Option<&str>) -> Vec<Value> {
    let mut models: Vec<Value> = orders_models();
    let query: Value = query.map_or_else(|| models[0]["query_sql"].clone(), |query| json!(query));
    models[0]["query_sql"] = query.clone();
    models[0]["authored_sql"] = query;
    models
}

pub(crate) fn evaluation_summary(
    evaluation: &RowsEvaluation,
) -> (Vec<Fault>, Vec<String>, u64, u64, u64, bool) {
    (
        evaluation.faults.clone(),
        evaluation.selected_codes.clone(),
        evaluation.evaluated_models,
        evaluation.cache_hits,
        evaluation.cache_misses,
        evaluation.reused,
    )
}

pub(crate) fn written_rules_files(project_dir: &Path) -> Vec<PathBuf> {
    [SPECULATIVE_CACHE_FILE, SPECULATIVE_MEMO_FILE]
        .iter()
        .map(|file| project_dir.join(file))
        .filter(|path| path.is_file())
        .collect()
}

/// Discard a speculation after it ran a while; it never reports stopping.
pub(crate) fn discard_after_running(
    speculative: SpeculativeRows,
    _project_dir: &Path,
) -> Result<bool, String> {
    std::thread::sleep(std::time::Duration::from_millis(200));
    discard_speculative_rows(speculative);
    Ok(false)
}

/// Answer beside a speculation once the rules run was cancelled; whether the answer stopped.
pub(crate) fn answer_after_stop(
    speculative: SpeculativeRows,
    project_dir: &Path,
) -> Result<bool, String> {
    Ok(matches!(
        answer_speculative_rows(
            speculative,
            speculative_rows_request(project_dir, true, &orders_models())?,
            &AtomicBool::new(true),
        ),
        SpeculativeAnswer::Stopped
    ))
}

/// Drop a speculation at once; it never reports stopping.
pub(crate) fn drop_at_once(
    speculative: SpeculativeRows,
    _project_dir: &Path,
) -> Result<bool, String> {
    drop(speculative);
    Ok(false)
}
pub(crate) fn query_facts_faults(
    test_case: &test_types::QueryFactsTestCase,
) -> Result<Vec<(String, u64, u64, String)>, String> {
    let project_dir = TempDir::new().map_err(|error| error.to_string())?;
    let config = json!({"select": test_case.select, "cache": {"enabled": false}});
    let mut request: Value =
        serde_json::from_str(&request(&project_dir, &config)).map_err(|error| error.to_string())?;
    request["dialect"] = json!(test_case.dialect);
    request["models"][0]["query_sql"] = json!(test_case.query_sql);
    request["models"][0]["authored_sql"] = json!(test_case.query_sql);
    request["models"][0]["enum_columns"] = json!(test_case.enum_columns);
    request["models"][0]["config"] = json!({"contract": "enforced"});
    request["models"][0]["columns"] = test_case.columns.clone();
    let result: Value = serde_json::from_str(&evaluate_json(&request.to_string())?)
        .map_err(|error| error.to_string())?;
    let faults = result["faults"]
        .as_array()
        .ok_or("faults must be an array")?;
    Ok(faults
        .iter()
        .map(|fault| {
            (
                fault["code"].as_str().unwrap_or_default().to_owned(),
                fault["line"].as_u64().unwrap_or_default(),
                fault["column"].as_u64().unwrap_or_default(),
                fault["message"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect())
}

/// The retained request with each row's digest restored, as a delta compile rebuilds it.
pub(crate) fn restored_rows(retained: &RetainedRows) -> Result<EvaluateRequest, String> {
    let mut request: EvaluateRequest =
        retained_request(retained).ok_or("the retained request does not decode")?;
    for (model, digest) in request.models.iter_mut().zip(&retained.digests) {
        model.payload_digest = Some(digest.clone());
        model.stripped = true;
    }
    Ok(request)
}

/// A model with a value in every field the retained rows keep, and SQL text they drop.
pub(crate) fn every_field_model() -> Model {
    let declaration: Declaration = Declaration {
        name: "status".to_owned(),
        relative_path: "enums/status.sql".to_owned(),
        members: vec![DeclarationMember {
            name: "open".to_owned(),
            value: json!({"code": 1, "ratio": -0.25, "tags": ["a", null, true]}),
        }],
        value: Some(json!(12345678901234567890_u64)),
        value_type: Some("BIGINT".to_owned()),
        render_as: None,
    };
    Model {
        sql_analysis_disabled: true,
        name: "orders".to_owned(),
        relative_path: "models/orders.sql".to_owned(),
        query_sql: "SELECT 1".to_owned(),
        authored_sql: "SELECT 1 -- authored".to_owned(),
        config: BTreeMap::from([
            ("materialized".to_owned(), json!("table")),
            ("tags".to_owned(), json!(["finance", "daily"])),
        ]),
        authored_config_keys: vec!["materialized".to_owned()],
        logical_schema: Some("analytics".to_owned()),
        references: vec![Reference {
            ref_kind: "source".to_owned(),
            ref_name: "raw_orders".to_owned(),
            ref_package: Some("shop".to_owned()),
        }],
        columns: vec![Column {
            name: "amount ✓".to_owned(),
            data_type: "DOUBLE".to_owned(),
            nullable: Some(false),
            audit_count: 2,
            type_proven: true,
        }],
        dynamic_columns: vec![DynamicColumnFamily {
            name: "amounts".to_owned(),
            pivot_column: "region".to_owned(),
            value_column: "amount".to_owned(),
            aggregate: "sum".to_owned(),
            data_type: "DOUBLE".to_owned(),
            name_pattern: Some("amount_{region}".to_owned()),
            type_proven: false,
        }],
        dynamic_columns_proven: true,
        bare_dynamic_pivot: true,
        enum_columns: vec!["status".to_owned()],
        enum_declarations: vec![declaration.clone()],
        constant_declarations: vec![declaration],
        typed_calls: Vec::new(),
        query_line_offset: 0,
        query_column_offset: 0,
        authored_query: None,
        declared_audit_count: 3,
        targeting_test_count: 4,
        empty_input_only_test_count: 0,
        payload_digest: Some("digest".to_owned()),
        stripped: false,
    }
}

/// Two models: one with every field set, one with none.
pub(crate) fn coded_models() -> Vec<Model> {
    vec![every_field_model(), Model::default()]
}

/// No models.
pub(crate) fn no_models() -> Vec<Model> {
    Vec::new()
}

/// `models` as retained rows read back would hold them: SQL text and digests dropped.
pub(crate) fn without_sql(models: Vec<Model>) -> Value {
    let stripped: Vec<Model> = models
        .into_iter()
        .map(|model| Model {
            query_sql: String::new(),
            authored_sql: String::new(),
            payload_digest: None,
            ..model
        })
        .collect();
    serde_json::to_value(stripped).unwrap_or_default()
}

/// One published model JSON row, stored under its domain's `mart` folder.
fn published_model(name: &str, references: &[&str]) -> Value {
    let domain: &str = name.split("__").next().unwrap_or_default();
    json!({
        "name": name,
        "relative_path": format!("models/{domain}/mart/{name}.sql"),
        "query_sql": "SELECT 1",
        "authored_sql": "SELECT 1",
        "references": references
            .iter()
            .map(|reference| json!({"ref_kind": "ref", "ref_name": reference}))
            .collect::<Vec<Value>>(),
    })
}

/// `(path, message and remediation)` of every fault the case's published models raise.
pub(crate) fn published_layer_faults(
    test_case: &test_types::PublishedLayerTestCase,
) -> Result<Vec<(String, String)>, String> {
    let project_dir = TempDir::new().map_err(|error| error.to_string())?;
    let config = json!({
        "select": [test_case.code],
        "rule_options": {test_case.code: test_case.options},
        "cache": {"enabled": false}
    });
    let mut request: Value =
        serde_json::from_str(&request(&project_dir, &config)).map_err(|error| error.to_string())?;
    request["models"] = test_case
        .models
        .iter()
        .map(|(name, references)| published_model(name, references))
        .collect();
    let result: Value = serde_json::from_str(&evaluate_json(&request.to_string())?)
        .map_err(|error| error.to_string())?;
    Ok(result["faults"]
        .as_array()
        .ok_or("faults must be an array")?
        .iter()
        .map(|fault| {
            (
                fault["path"].as_str().unwrap_or_default().to_owned(),
                format!(
                    "{}\n{}",
                    fault["message"].as_str().unwrap_or_default(),
                    fault["remediation"].as_str().unwrap_or_default()
                ),
            )
        })
        .collect())
}

/// The model path the relation-exception cases use.
pub(crate) const CROSS_JOIN_MODEL_PATH: &str = "models/orders.sql";
/// The Rule whose findings name a joined relation.
pub(crate) const CROSS_JOIN_CODE: &str = "SQLCLEAN038";

fn cross_join_fault(message: &str, line: u64) -> Fault {
    Fault {
        unevaluated: false,
        code: CROSS_JOIN_CODE.to_owned(),
        path: CROSS_JOIN_MODEL_PATH.to_owned(),
        line,
        column: 1,
        message: message.to_owned(),
        remediation: String::new(),
    }
}

/// Finalize a top-level `order_policy` join and a `customers` join in CTE `checks`.
pub(crate) fn finalize_cross_joins(exceptions: Value) -> Result<Vec<Fault>, String> {
    let project_dir = TempDir::new().map_err(|error| error.to_string())?;
    std::fs::create_dir_all(project_dir.path().join("models"))
        .map_err(|error| error.to_string())?;
    std::fs::write(project_dir.path().join(CROSS_JOIN_MODEL_PATH), "SELECT 1")
        .map_err(|error| error.to_string())?;
    let config: RulesConfig = serde_json::from_value(json!({"rule_exceptions": exceptions}))
        .map_err(|error| error.to_string())?;
    let findings: Vec<Fault> = vec![
        cross_join_fault(
            "CROSS JOIN with `order_policy` creates an explicit cartesian product",
            3,
        ),
        cross_join_fault(
            "CROSS JOIN with `customers` in CTE `checks` creates an explicit cartesian product",
            7,
        ),
    ];
    finalize_rows(
        project_dir.path().to_string_lossy().into_owned(),
        config,
        &[CROSS_JOIN_CODE.to_owned()],
        findings,
    )
}
