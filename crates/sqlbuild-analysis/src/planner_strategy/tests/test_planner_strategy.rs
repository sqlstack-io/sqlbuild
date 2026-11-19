use std::collections::BTreeSet;

use crate::planner_changes::models::{SchemaChangeKind, SchemaColumnSource};
use crate::planner_strategy::constants::QUERY_CHANGE_TRACKING_HELP;
use crate::planner_strategy::main::cap_integer_bounds::cap_integer_bounds;
use crate::planner_strategy::main::cap_timeline_bounds::cap_timeline_bounds;
use crate::planner_strategy::main::capped_watermark_input::capped_watermark_input;
use crate::planner_strategy::main::classify_cursor_sql_type::classify_cursor_sql_type;
use crate::planner_strategy::main::count_integer_batches::count_integer_batches;
use crate::planner_strategy::main::count_timeline_batches::count_timeline_batches;
use crate::planner_strategy::main::lineage_reference::lineage_reference;
use crate::planner_strategy::main::microbatch_limit_warning::microbatch_limit_warning;
use crate::planner_strategy::main::model_plan_action::model_plan_action;
use crate::planner_strategy::main::model_warnings::model_warnings;
use crate::planner_strategy::main::schema_actions::schema_actions;
use crate::planner_strategy::main::singular_attachment::singular_attachment;
use crate::planner_strategy::models::{
    AuditAttachmentKind, CapDirection, CappedBound, ChangeFacts, CursorType, MaterializationType,
    ModelStrategy, PlanAction, PlanChangeKind, PlanReason, SchemaActionKind, SchemaActionRow,
    WarningInputs, WarningSeverity,
};
use crate::planner_strategy::tests::helpers::{change, failure_text, finding, names, warning};
use crate::planner_strategy::tests::test_types::{
    ActionTestCase, AttachmentTestCase, ClassifyTestCase, IntegerBatchTestCase,
    LimitWarningTestCase, Lineage, LineageTestCase, SchemaActionTestCase, Steps,
    TimelineBatchTestCase, WarningTestCase,
};

#[test]
fn given_model_change_when_resolving_action_then_matches_python_strategy() {
    let test_cases = [
        ActionTestCase {
            description: "view rename",
            materialization: MaterializationType::View,
            strategy: None,
            change: change(PlanChangeKind::Renamed),
            full_refresh: false,
            expected_action: Ok((PlanAction::CreateView, PlanReason::Renamed)),
        },
        ActionTestCase {
            description: "snapshot without change",
            materialization: MaterializationType::Snapshot,
            strategy: None,
            change: change(PlanChangeKind::NoChange),
            full_refresh: false,
            expected_action: Ok((PlanAction::Snapshot, PlanReason::NormalIncremental)),
        },
        ActionTestCase {
            description: "custom full refresh",
            materialization: MaterializationType::Custom,
            strategy: None,
            change: change(PlanChangeKind::Renamed),
            full_refresh: true,
            expected_action: Ok((PlanAction::Custom, PlanReason::FullRefresh)),
        },
        ActionTestCase {
            description: "table query change",
            materialization: MaterializationType::Table,
            strategy: None,
            change: change(PlanChangeKind::QueryChanged),
            full_refresh: false,
            expected_action: Ok((PlanAction::CreateTable, PlanReason::QueryChanged)),
        },
        ActionTestCase {
            description: "incremental full backfill of a config change",
            materialization: MaterializationType::Incremental,
            strategy: Some("merge"),
            change: ChangeFacts {
                backfill_full: true,
                config_changed: true,
                ..change(PlanChangeKind::ConfigChanged)
            },
            full_refresh: false,
            expected_action: Ok((PlanAction::CreateTable, PlanReason::ConfigChanged)),
        },
        ActionTestCase {
            description: "incremental append",
            materialization: MaterializationType::Incremental,
            strategy: Some("append"),
            change: change(PlanChangeKind::NoChange),
            full_refresh: false,
            expected_action: Ok((PlanAction::IncrementalAppend, PlanReason::NormalIncremental)),
        },
        ActionTestCase {
            description: "incremental without strategy",
            materialization: MaterializationType::Incremental,
            strategy: None,
            change: change(PlanChangeKind::NoChange),
            full_refresh: false,
            expected_action: Err(
                "S201 incremental model 'orders' is missing required incremental_strategy"
                    .to_owned(),
            ),
        },
        ActionTestCase {
            description: "incremental with unknown strategy",
            materialization: MaterializationType::Incremental,
            strategy: Some("upsert"),
            change: change(PlanChangeKind::QueryChanged),
            full_refresh: false,
            expected_action: Err(
                "S202 incremental model 'orders' has unknown strategy 'upsert'".to_owned(),
            ),
        },
        ActionTestCase {
            description: "maintained first run creates the dynamic table",
            materialization: MaterializationType::Maintained,
            strategy: None,
            change: change(PlanChangeKind::FirstRun),
            full_refresh: false,
            expected_action: Ok((PlanAction::CreateMaintained, PlanReason::FirstRun)),
        },
        ActionTestCase {
            description: "maintained query change recreates it",
            materialization: MaterializationType::Maintained,
            strategy: None,
            change: change(PlanChangeKind::QueryChanged),
            full_refresh: false,
            expected_action: Ok((PlanAction::CreateMaintained, PlanReason::QueryChanged)),
        },
        ActionTestCase {
            description: "maintained config change such as refresh_mode recreates it",
            materialization: MaterializationType::Maintained,
            strategy: None,
            change: change(PlanChangeKind::ConfigChanged),
            full_refresh: false,
            expected_action: Ok((PlanAction::CreateMaintained, PlanReason::ConfigChanged)),
        },
        ActionTestCase {
            description: "maintained rename creates it under the new name",
            materialization: MaterializationType::Maintained,
            strategy: None,
            change: change(PlanChangeKind::Renamed),
            full_refresh: false,
            expected_action: Ok((PlanAction::CreateMaintained, PlanReason::Renamed)),
        },
        ActionTestCase {
            description: "maintained full refresh recreates an unchanged model",
            materialization: MaterializationType::Maintained,
            strategy: None,
            change: change(PlanChangeKind::NoChange),
            full_refresh: true,
            expected_action: Ok((PlanAction::CreateMaintained, PlanReason::FullRefresh)),
        },
        ActionTestCase {
            description: "unchanged maintained model is kept",
            materialization: MaterializationType::Maintained,
            strategy: None,
            change: change(PlanChangeKind::NoChange),
            full_refresh: false,
            expected_action: Ok((PlanAction::KeepMaintained, PlanReason::NoChange)),
        },
    ];
    for test_case in test_cases {
        let model = ModelStrategy {
            name: "orders".to_owned(),
            disabled: false,
            materialization: test_case.materialization,
            incremental_strategy: test_case.strategy.map(str::to_owned),
        };
        assert_eq!(
            model_plan_action(&model, &test_case.change, test_case.full_refresh)
                .map_err(|error| format!("{} {}", error.code(), error.message())),
            test_case.expected_action,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_schema_findings_when_resolving_actions_then_follows_on_schema_change() {
    let added = SchemaActionRow {
        kind: SchemaActionKind::AddColumn,
        column_name: "amount".to_owned(),
        column_type: Some("INTEGER".to_owned()),
    };
    let test_cases = [
        SchemaActionTestCase {
            description: "default appends new columns",
            on_schema_change: None,
            expected_actions: vec![added.clone()],
        },
        SchemaActionTestCase {
            description: "sync drops and retypes",
            on_schema_change: Some("sync_all_columns"),
            expected_actions: vec![
                added,
                SchemaActionRow {
                    kind: SchemaActionKind::DropColumn,
                    column_name: "amount".to_owned(),
                    column_type: None,
                },
                SchemaActionRow {
                    kind: SchemaActionKind::AlterColumnType,
                    column_name: "amount".to_owned(),
                    column_type: Some("BIGINT".to_owned()),
                },
            ],
        },
        SchemaActionTestCase {
            description: "fail resolves nothing",
            on_schema_change: Some("fail"),
            expected_actions: Vec::new(),
        },
    ];
    let findings = [
        finding(
            SchemaChangeKind::ColumnAdded,
            SchemaColumnSource::Yml,
            (Some("INTEGER"), None),
        ),
        finding(
            SchemaChangeKind::ColumnRemoved,
            SchemaColumnSource::Yml,
            (None, Some("INTEGER")),
        ),
        finding(
            SchemaChangeKind::ColumnTypeChanged,
            SchemaColumnSource::Yml,
            (Some("BIGINT"), Some("INTEGER")),
        ),
    ];
    for test_case in test_cases {
        assert_eq!(
            schema_actions(&findings, test_case.on_schema_change),
            test_case.expected_actions,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_change_facts_when_building_warnings_then_matches_python_messages() {
    let test_cases = [
        WarningTestCase {
            description: "ignored inferred addition",
            on_schema_change: Some("ignore"),
            type_enforcement: false,
            change: ChangeFacts {
                schema_findings: vec![finding(
                    SchemaChangeKind::ColumnAdded,
                    SchemaColumnSource::SqlAnalysis,
                    (Some(""), None),
                )],
                ..change(PlanChangeKind::SchemaChanged)
            },
            expected_warnings: vec![
                warning(
                    WarningSeverity::Info,
                    "inferred column amount: new column not in warehouse",
                ),
                warning(
                    WarningSeverity::Info,
                    "schema change detected but on_schema_change is set to ignore",
                ),
            ],
        },
        WarningTestCase {
            description: "failing enforced type change",
            on_schema_change: Some("fail"),
            type_enforcement: true,
            change: ChangeFacts {
                schema_findings: vec![finding(
                    SchemaChangeKind::ColumnTypeChanged,
                    SchemaColumnSource::Yml,
                    (Some("BIGINT"), None),
                )],
                ..change(PlanChangeKind::SchemaChanged)
            },
            expected_warnings: vec![
                warning(
                    WarningSeverity::Error,
                    "schema change detected and on_schema_change is set to fail",
                ),
                warning(
                    WarningSeverity::Warning,
                    "enforced column amount type mismatch: expected BIGINT, warehouse has None",
                ),
            ],
        },
        WarningTestCase {
            description: "forward-only function change",
            on_schema_change: None,
            type_enforcement: false,
            change: ChangeFacts {
                changed_functions: names(&["discount", "tax"]),
                ..change(PlanChangeKind::FunctionChanged)
            },
            expected_warnings: vec![warning(
                WarningSeverity::Warning,
                &format!(
                    "orders: function discount, tax changed, so already-built dates keep their \
                     old results - only new dates will use the new query. To apply the change \
                     to existing data, choose how much to reprocess: replay_on_change full \
                     (rebuild every date with the new SQL), replay_on_change bounded-<duration> \
                     e.g. bounded-14d (reprocess just that window), or leave it forward-only \
                     (existing dates as-is, new dates use the new query - the default). If this \
                     model's output does not depend on the SQL that changed, this is safe to \
                     ignore. {QUERY_CHANGE_TRACKING_HELP}"
                ),
            )],
        },
    ];
    for test_case in test_cases {
        let inputs = WarningInputs {
            model_name: "orders".to_owned(),
            materialization: MaterializationType::Incremental,
            has_schema_actions: false,
            on_schema_change: test_case.on_schema_change.map(str::to_owned),
            type_enforcement: test_case.type_enforcement,
        };
        assert_eq!(
            model_warnings(&inputs, &test_case.change),
            test_case.expected_warnings,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_warehouse_type_when_classifying_cursor_type_then_matches_python() {
    let test_cases = [
        ClassifyTestCase {
            description: "timestamp with time zone",
            sql_type: "timestamp with time zone",
            expected_cursor_type: Some(CursorType::Timestamp),
        },
        ClassifyTestCase {
            description: "number(38, 0)",
            sql_type: "number(38, 0)",
            expected_cursor_type: Some(CursorType::Integer),
        },
        ClassifyTestCase {
            description: "NUMERIC(10,2)",
            sql_type: "NUMERIC(10,2)",
            expected_cursor_type: None,
        },
        ClassifyTestCase {
            description: "decimal",
            sql_type: "decimal",
            expected_cursor_type: Some(CursorType::Integer),
        },
        ClassifyTestCase {
            description: "  big   int ",
            sql_type: "  big   int ",
            expected_cursor_type: Some(CursorType::Integer),
        },
        ClassifyTestCase {
            description: "VARCHAR(20)",
            sql_type: "VARCHAR(20)",
            expected_cursor_type: None,
        },
        ClassifyTestCase {
            description: "INTEGER",
            sql_type: "INTEGER",
            expected_cursor_type: Some(CursorType::Integer),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            classify_cursor_sql_type(test_case.sql_type).ok().flatten(),
            test_case.expected_cursor_type,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_integer_range_when_counting_and_capping_then_matches_python() {
    let test_cases = [
        IntegerBatchTestCase {
            description: "partial last batch from the end",
            bounds: (0, 10),
            batch_size: " 3 ",
            direction: CapDirection::FromEnd,
            expected_count: 4,
            expected_cap: Some(CappedBound::Start(6)),
        },
        IntegerBatchTestCase {
            description: "from the start",
            bounds: (0, 10),
            batch_size: "1_0",
            direction: CapDirection::FromStart,
            expected_count: 1,
            expected_cap: Some(CappedBound::End(10)),
        },
        IntegerBatchTestCase {
            description: "unparsable size",
            bounds: (0, 10),
            batch_size: "1d",
            direction: CapDirection::FromStart,
            expected_count: 0,
            expected_cap: None,
        },
        IntegerBatchTestCase {
            description: "zero size",
            bounds: (0, 10),
            batch_size: "0",
            direction: CapDirection::FromEnd,
            expected_count: 0,
            expected_cap: None,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            count_integer_batches(test_case.bounds, test_case.batch_size, false),
            test_case.expected_count,
            "{}",
            test_case.description
        );
        assert_eq!(
            cap_integer_bounds(
                test_case.bounds,
                test_case.batch_size,
                2,
                test_case.direction
            ),
            test_case.expected_cap,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_timeline_range_when_counting_and_capping_then_clips_the_last_batch() {
    let test_cases = [
        TimelineBatchTestCase {
            description: "last two of four batches",
            bounds: (0, 10),
            max_batches: 2,
            direction: CapDirection::FromEnd,
            expected_count: 4,
            expected_cap: CappedBound::Start(6),
        },
        TimelineBatchTestCase {
            description: "first two batches",
            bounds: (0, 10),
            max_batches: 2,
            direction: CapDirection::FromStart,
            expected_count: 4,
            expected_cap: CappedBound::End(6),
        },
        TimelineBatchTestCase {
            description: "more allowed than planned",
            bounds: (0, 4),
            max_batches: 5,
            direction: CapDirection::FromStart,
            expected_count: 2,
            expected_cap: CappedBound::End(4),
        },
        TimelineBatchTestCase {
            description: "equal bounds",
            bounds: (5, 5),
            max_batches: 1,
            direction: CapDirection::FromEnd,
            expected_count: 1,
            expected_cap: CappedBound::Start(5),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            count_timeline_batches(&Steps, test_case.bounds, true),
            Ok(test_case.expected_count),
            "{}",
            test_case.description
        );
        assert_eq!(
            cap_timeline_bounds(
                &Steps,
                test_case.bounds,
                test_case.max_batches,
                test_case.direction
            ),
            Ok(test_case.expected_cap),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_microbatch_limit_when_exceeded_then_formats_python_warning() {
    let test_cases = [
        LimitWarningTestCase {
            description: "within the limit",
            limit: (Some(2), 2),
            action: ("warn", false),
            expected_warning: None,
        },
        LimitWarningTestCase {
            description: "no limit",
            limit: (None, 9),
            action: ("error", false),
            expected_warning: None,
        },
        LimitWarningTestCase {
            description: "capped",
            limit: (Some(2), 5),
            action: ("cap_from_end", true),
            expected_warning: Some(
                "MICROBATCH LIMIT EXCEEDED: model 'orders' planned 5 batches, above the \
                 per-model limit of 2 (action=cap_from_end). Selected 2 batches and deferred 3.",
            ),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            microbatch_limit_warning("orders", test_case.limit, test_case.action).as_deref(),
            test_case.expected_warning,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_audit_references_when_inferring_attachment_then_picks_latest_owner() {
    let test_cases = [
        AttachmentTestCase {
            description: "sources only",
            models: Vec::new(),
            sources: names(&["payments", "events"]),
            expected_attachment: (AuditAttachmentKind::Source, Some("events".to_owned())),
        },
        AttachmentTestCase {
            description: "nothing",
            models: Vec::new(),
            sources: Vec::new(),
            expected_attachment: (AuditAttachmentKind::End, None),
        },
        AttachmentTestCase {
            description: "downstream model owns",
            models: names(&["orders", "customers"]),
            sources: Vec::new(),
            expected_attachment: (AuditAttachmentKind::Model, Some("orders".to_owned())),
        },
        AttachmentTestCase {
            description: "duplicate reference",
            models: names(&["products", "products"]),
            sources: Vec::new(),
            expected_attachment: (AuditAttachmentKind::Model, Some("products".to_owned())),
        },
        AttachmentTestCase {
            description: "unrelated models",
            models: names(&["products", "customers"]),
            sources: Vec::new(),
            expected_attachment: (AuditAttachmentKind::End, None),
        },
    ];
    for test_case in test_cases {
        let reaches = |from: &str, to: &str| -> Result<bool, ()> {
            Ok(from == "customers" && to == "orders")
        };
        assert_eq!(
            singular_attachment(&test_case.models, &test_case.sources, &reaches),
            Ok(test_case.expected_attachment),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_watermark_input_when_walking_lineage_then_finds_reference_and_capped_producer() {
    let test_cases = [
        LineageTestCase {
            description: "through a function",
            input: "products",
            capped: &[],
            expected_reference: Ok("products".to_owned()),
            expected_capped: Ok(None),
        },
        LineageTestCase {
            description: "capped direct producer",
            input: "customers",
            capped: &["customers"],
            expected_reference: Ok("customers".to_owned()),
            expected_capped: Ok(Some("customers".to_owned())),
        },
        LineageTestCase {
            description: "capped producer behind a microbatch watermark",
            input: "customers",
            capped: &["events"],
            expected_reference: Ok("customers".to_owned()),
            expected_capped: Ok(Some("events".to_owned())),
        },
        LineageTestCase {
            description: "missing input",
            input: "returns",
            capped: &[],
            expected_reference: Err(
                "NotInLineage { model: \"orders\", input: \"returns\" }".to_owned()
            ),
            expected_capped: Err(
                "NotInLineage { model: \"orders\", input: \"returns\" }".to_owned()
            ),
        },
    ];
    for test_case in test_cases {
        let graph = Lineage {
            capped: test_case
                .capped
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<BTreeSet<_>>(),
        };
        assert_eq!(
            lineage_reference(&graph, ("orders", Lineage::roots()), test_case.input)
                .map(|reference| reference.name)
                .map_err(failure_text),
            test_case.expected_reference,
            "{}",
            test_case.description
        );
        assert_eq!(
            capped_watermark_input(
                &graph,
                ("orders", Lineage::roots()),
                names(&[test_case.input])
            )
            .map(|found| found.map(|capped| capped.producer))
            .map_err(failure_text),
            test_case.expected_capped,
            "{}",
            test_case.description
        );
    }
}
