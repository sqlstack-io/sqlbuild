//! Layouts of the rule engine value records hosted natively.

use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::records::models::{FieldDefault, FieldSpec, RecordSpec};

const R_RULE_GUIDANCE: RecordSpec = RecordSpec {
    name: "RuleGuidance",
    doc: "Structured authoring guidance shared by diagnostics and generated documentation.",
    fields: &[
        FieldSpec::required("good_example"),
        FieldSpec::required("anti_tautology"),
        FieldSpec::required("mutation_check"),
    ],
};
const R_RULE_EXEMPTION: RecordSpec = RecordSpec {
    name: "RuleExemption",
    doc: "",
    fields: &[
        FieldSpec::required("rule"),
        FieldSpec::required("path"),
        FieldSpec::required("reason"),
        FieldSpec::new("relation", FieldDefault::None),
        FieldSpec::new("cte", FieldDefault::None),
    ],
};
const R_GRAPH_EDGE_EXCLUSION: RecordSpec = RecordSpec {
    name: "GraphEdgeExclusion",
    doc: "One exact, temporary internal-layer dependency exception.",
    fields: &[
        FieldSpec::required("consumer"),
        FieldSpec::required("dependency"),
        FieldSpec::required("reason"),
    ],
};
const R_RULE_IGNORE: RecordSpec = RecordSpec {
    name: "RuleIgnore",
    doc: "",
    fields: &[
        FieldSpec::required("rules"),
        FieldSpec::required("paths"),
        FieldSpec::required("reason"),
        FieldSpec::new("selectors", FieldDefault::EmptyTuple),
    ],
};
const R_SELECT_STAR_ALLOW: RecordSpec = RecordSpec {
    name: "SelectStarAllow",
    doc: "",
    fields: &[FieldSpec::required("paths"), FieldSpec::required("reason")],
};
const R_THRESHOLD_OVERRIDE: RecordSpec = RecordSpec {
    name: "ThresholdOverride",
    doc: "",
    fields: &[
        FieldSpec::required("paths"),
        FieldSpec::required("thresholds"),
        FieldSpec::required("reason"),
    ],
};
const R_RULES_CACHE_CONFIG: RecordSpec = RecordSpec {
    name: "RulesCacheConfig",
    doc: "",
    fields: &[FieldSpec::new("enabled", FieldDefault::True)],
};
const R_SQL_TEST_RULES_CONFIG: RecordSpec = RecordSpec {
    name: "SqlTestRulesConfig",
    doc: "",
    fields: &[FieldSpec::new(
        "pipeline_directory",
        FieldDefault::Text("pipelines"),
    )],
};
const R_SQL_RULES_EVALUATION: RecordSpec = RecordSpec {
    name: "SqlRulesEvaluation",
    doc: "SQL rule findings of one run, their cache accounting and elapsed time.",
    fields: &[
        FieldSpec::required("findings"),
        FieldSpec::required("cache_hits"),
        FieldSpec::required("cache_misses"),
        FieldSpec::new("sql_ms", FieldDefault::Int(0)),
    ],
};
const R_RULE_RUN_PLAN: RecordSpec = RecordSpec {
    name: "RuleRunPlan",
    doc: "The catalogue, selected rules, project slice and settings one rules run evaluates.",
    fields: &[
        FieldSpec::required("effective_config"),
        FieldSpec::required("project_dir"),
        FieldSpec::required("catalogue"),
        FieldSpec::required("custom_payloads"),
        FieldSpec::required("selected"),
        FieldSpec::required("skipped_type_proof_rules"),
        FieldSpec::required("model_paths"),
        FieldSpec::required("project"),
        FieldSpec::required("config"),
    ],
};
const R_RULES_RUN_RESULT: RecordSpec = RecordSpec {
    name: "RulesRunResult",
    doc: "Unified findings and phase measurements for configured rules.",
    fields: &[
        FieldSpec::required("findings"),
        FieldSpec::required("evaluated_models"),
        FieldSpec::required("built_in_ms"),
        FieldSpec::required("custom_ms"),
        FieldSpec::new("custom_cpu_ms", FieldDefault::Int(0)),
        FieldSpec::new("unevaluated_resources", FieldDefault::Int(0)),
        FieldSpec::new("cache_hits", FieldDefault::Int(0)),
        FieldSpec::new("cache_misses", FieldDefault::Int(0)),
        FieldSpec::new("skipped_type_proof_rules", FieldDefault::EmptyTuple),
        FieldSpec::new("custom_rule_timings", FieldDefault::EmptyTuple),
    ],
};
const R_NATIVE_RULES_EVALUATION: RecordSpec = RecordSpec {
    name: "NativeRulesEvaluation",
    doc: "The built-in rules response, decoded natively.",
    fields: &[
        FieldSpec::required("findings"),
        FieldSpec::required("selected_codes"),
        FieldSpec::required("evaluated_models"),
        FieldSpec::required("cache_hits"),
        FieldSpec::required("cache_misses"),
        FieldSpec::required("built_in_ms"),
        FieldSpec::required("reused"),
    ],
};
const R_CUSTOM_RULES_OUTCOME: RecordSpec = RecordSpec {
    name: "CustomRulesOutcome",
    doc: "Custom-rule findings in canonical order and their cache accounting.",
    fields: &[
        FieldSpec::required("findings"),
        FieldSpec::required("cache_hits"),
        FieldSpec::required("cache_misses"),
        FieldSpec::required("custom_ms"),
        FieldSpec::new("custom_cpu_ms", FieldDefault::Int(0)),
        FieldSpec::new("custom_rule_timings", FieldDefault::EmptyTuple),
    ],
};
const R_MODEL: RecordSpec = RecordSpec {
    name: "Model",
    doc: "Stable identity and common metadata for one compiled SQL model.",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::required("path"),
        FieldSpec::required("materialization"),
    ],
};
const R_PROJECT: RecordSpec = RecordSpec {
    name: "Project",
    doc: "Stable summary for one compiled project invocation.",
    fields: &[
        FieldSpec::required("model_count"),
        FieldSpec::required("target_name"),
    ],
};
const R_PROJECT_PATH: RecordSpec = RecordSpec {
    name: "ProjectPath",
    doc: "One normalized project-relative file or directory.",
    fields: &[
        FieldSpec::required("value"),
        FieldSpec::required("is_file"),
        FieldSpec::required("is_dir"),
    ],
};
const R_SQL_NODE: RecordSpec = RecordSpec {
    name: "SqlNode",
    doc: "Stable source location for one common SQL structure fact.",
    fields: &[
        FieldSpec::required("kind"),
        FieldSpec::required("line"),
        FieldSpec::required("column"),
    ],
};
const R_MODEL_SQL: RecordSpec = RecordSpec {
    name: "ModelSql",
    doc: "Authored and compiler-expanded SQL documents for one model.",
    fields: &[
        FieldSpec::required("authored"),
        FieldSpec::required("expanded"),
    ],
};
const R_RULE_FACT_VIEWS: RecordSpec = RecordSpec {
    name: "RuleFactViews",
    doc: "Views shared by every invocation in one custom-rule host evaluation.",
    fields: &[
        FieldSpec::required("project"),
        FieldSpec::required("sql"),
        FieldSpec::required("graph"),
        FieldSpec::required("columns"),
        FieldSpec::required("contracts"),
        FieldSpec::required("tests"),
        FieldSpec::required("audits"),
        FieldSpec::required("declarations"),
    ],
};
const R_RULE_OPTION: RecordSpec = RecordSpec {
    name: "RuleOption",
    doc: "One typed configuration option declared by a rule.",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::required("value_type"),
        FieldSpec::required("default"),
        FieldSpec::required("description"),
        FieldSpec::new("choices", FieldDefault::EmptyTuple),
        FieldSpec::new("minimum", FieldDefault::None),
        FieldSpec::new("maximum", FieldDefault::None),
        FieldSpec::new("minimum_items", FieldDefault::None),
        FieldSpec::new("item_type", FieldDefault::None),
    ],
};
const R_RULE: RecordSpec = RecordSpec {
    name: "Rule",
    doc: "Immutable metadata and implementation for one rule.",
    fields: &[
        FieldSpec::required("code"),
        FieldSpec::required("family"),
        FieldSpec::required("slug"),
        FieldSpec::required("message"),
        FieldSpec::required("remediation"),
        FieldSpec::required("check"),
        FieldSpec::new("options", FieldDefault::EmptyTuple),
        FieldSpec::new("enabled_by_default", FieldDefault::False),
        FieldSpec::new("custom", FieldDefault::False),
        FieldSpec::new("source", FieldDefault::None),
        FieldSpec::new("subject", FieldDefault::None),
        FieldSpec::new("subject_parameter", FieldDefault::None),
        FieldSpec::new("context_parameter", FieldDefault::None),
        FieldSpec::new("guidance", FieldDefault::None),
    ],
};
const R_FINDING: RecordSpec = RecordSpec {
    name: "Finding",
    doc: "One deterministic compiler rules violation.",
    fields: &[
        FieldSpec::required("code"),
        FieldSpec::required("path"),
        FieldSpec::required("line"),
        FieldSpec::required("column"),
        FieldSpec::required("message"),
        FieldSpec::required("remediation"),
        FieldSpec::new("unevaluated", FieldDefault::False),
        FieldSpec::new("affected_rules", FieldDefault::EmptyTuple),
        FieldSpec::new("fixable", FieldDefault::False),
    ],
};
const R_LAYOUT_CONFIG: RecordSpec = RecordSpec {
    name: "LayoutConfig",
    doc: "",
    fields: &[
        FieldSpec::new(
            "levels",
            FieldDefault::TextTuple(&[
                "staging",
                "intermediate/clean",
                "intermediate/enriched",
                "mart",
            ]),
        ),
        FieldSpec::new("domain_roots", FieldDefault::EmptyTuple),
    ],
};
const R_RULES_CONFIG: RecordSpec = RecordSpec {
    name: "RulesConfig",
    doc: "",
    fields: &[
        FieldSpec::new("select", FieldDefault::None),
        FieldSpec::new("ignore", FieldDefault::EmptyTuple),
        FieldSpec::new("thresholds", FieldDefault::EmptyDict),
        FieldSpec::new("threshold_overrides", FieldDefault::EmptyTuple),
        FieldSpec::new("rule_options", FieldDefault::EmptyDict),
        FieldSpec::new("rule_exceptions", FieldDefault::EmptyTuple),
        FieldSpec::new("graph_edge_exceptions", FieldDefault::EmptyTuple),
        FieldSpec::new("rule_ignores", FieldDefault::EmptyTuple),
        FieldSpec::new("select_star_allow", FieldDefault::EmptyTuple),
        FieldSpec::new("domains", FieldDefault::EmptyTuple),
        FieldSpec::new("approved_source_tokens", FieldDefault::EmptyTuple),
        FieldSpec::new("retired_source_tokens", FieldDefault::EmptyDict),
        FieldSpec::new(
            "sql_tests",
            FieldDefault::NativeCall(RecordId::SqlTestRulesConfig),
        ),
        FieldSpec::new("layout", FieldDefault::NativeCall(RecordId::LayoutConfig)),
        FieldSpec::new(
            "cache",
            FieldDefault::NativeCall(RecordId::RulesCacheConfig),
        ),
    ],
};
const R_RULES_RESULT: RecordSpec = RecordSpec {
    name: "RulesResult",
    doc: "",
    fields: &[
        FieldSpec::required("findings"),
        FieldSpec::required("evaluated_models"),
        FieldSpec::new("unevaluated_resources", FieldDefault::Int(0)),
        FieldSpec::new("cache_hits", FieldDefault::Int(0)),
        FieldSpec::new("cache_misses", FieldDefault::Int(0)),
        FieldSpec::new("built_in_ms", FieldDefault::Int(0)),
        FieldSpec::new("custom_ms", FieldDefault::Int(0)),
        FieldSpec::new("custom_cpu_ms", FieldDefault::Int(0)),
        FieldSpec::new("custom_rule_timings", FieldDefault::EmptyTuple),
    ],
};
const R_RULE_FILE: RecordSpec = RecordSpec {
    name: "RuleFile",
    doc: "",
    fields: &[FieldSpec::required("path"), FieldSpec::required("source")],
};
const R_RULE_CASE: RecordSpec = RecordSpec {
    name: "RuleCase",
    doc: "",
    fields: &[
        FieldSpec::required("description"),
        FieldSpec::required("source"),
        FieldSpec::required("expected_finding_count"),
        FieldSpec::new(
            "path",
            FieldDefault::Text("models/mart/example__mart__result.sql"),
        ),
        FieldSpec::new("files", FieldDefault::EmptyTuple),
        FieldSpec::new("config", FieldDefault::EmptyDict),
    ],
};
const R_RULE_RESULT: RecordSpec = RecordSpec {
    name: "RuleResult",
    doc: "",
    fields: &[FieldSpec::required("findings")],
};
const R_CUSTOM_RULE_EVALUATION: RecordSpec = RecordSpec {
    name: "CustomRuleEvaluation",
    doc: "Findings and observed fact reads of one rule invocation for one subject.",
    fields: &[
        FieldSpec::required("code"),
        FieldSpec::required("subject"),
        FieldSpec::required("findings"),
        FieldSpec::required("reads"),
    ],
};
const R_MODULE_STATE_TRACE: RecordSpec = RecordSpec {
    name: "ModuleStateTrace",
    doc: "What exact module-state walks touched: object and type identities and their decisions.",
    fields: &[
        FieldSpec::required("touched"),
        FieldSpec::required("descended_types"),
        FieldSpec::required("checked_instances"),
    ],
};
const R_MODULE_STATE_FINGERPRINT: RecordSpec = RecordSpec {
    name: "ModuleStateFingerprint",
    doc: "One exact custom-rule module-state fingerprint and the trace of the walk behind it.",
    fields: &[FieldSpec::required("token"), FieldSpec::required("trace")],
};
const R_CUSTOM_RULE_RUN: RecordSpec = RecordSpec {
    name: "CustomRuleRun",
    doc: "Every invocation of one host run plus rules whose reads could not be attributed.",
    fields: &[
        FieldSpec::required("evaluations"),
        FieldSpec::required("untracked_codes"),
        FieldSpec::new("uncacheable_codes", FieldDefault::EmptyFrozenset),
        FieldSpec::new("observed", FieldDefault::EmptyTuple),
        FieldSpec::new("stateful_codes", FieldDefault::EmptyFrozenset),
        FieldSpec::new("rule_timings", FieldDefault::EmptyDict),
        FieldSpec::new("monitor_ns", FieldDefault::Int(0)),
    ],
};
const R_CUSTOM_HOST_PARTITION: RecordSpec = RecordSpec {
    name: "CustomHostPartition",
    doc: "How one host of a split run detects module state and learns the run was cancelled.",
    fields: &[FieldSpec::required("cancelled")],
};

/// The rule engine value records.
pub(crate) const RULE_RECORDS: &[RecordSpec] = &[
    R_RULE_GUIDANCE,
    R_RULE_EXEMPTION,
    R_GRAPH_EDGE_EXCLUSION,
    R_RULE_IGNORE,
    R_SELECT_STAR_ALLOW,
    R_THRESHOLD_OVERRIDE,
    R_RULES_CACHE_CONFIG,
    R_SQL_TEST_RULES_CONFIG,
    R_SQL_RULES_EVALUATION,
    R_RULE_RUN_PLAN,
    R_RULES_RUN_RESULT,
    R_NATIVE_RULES_EVALUATION,
    R_CUSTOM_RULES_OUTCOME,
    R_MODEL,
    R_PROJECT,
    R_PROJECT_PATH,
    R_SQL_NODE,
    R_MODEL_SQL,
    R_RULE_FACT_VIEWS,
    R_RULE_OPTION,
    R_RULE,
    R_FINDING,
    R_LAYOUT_CONFIG,
    R_RULES_CONFIG,
    R_RULES_RESULT,
    R_RULE_FILE,
    R_RULE_CASE,
    R_RULE_RESULT,
    R_CUSTOM_RULE_EVALUATION,
    R_MODULE_STATE_TRACE,
    R_MODULE_STATE_FINGERPRINT,
    R_CUSTOM_RULE_RUN,
    R_CUSTOM_HOST_PARTITION,
];
