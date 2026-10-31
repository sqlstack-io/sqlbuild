use crate::models::{
    AttachedAuditTargetKind, AuditAttachmentKind, AuditEvaluationMode, AuditOutcome, AuditRunScope,
    AuditSeverity, BuildStatus, CancellationOutcome, ExecutionPhase, ExecutionResourceKind,
    ExecutionStatus, HookPhase, ItemKind, LifeCycleEventKind, MicrobatchLimitAction, SkipMode,
    SqlTestOutcome,
};
use crate::tests::helpers::{parsed, values};
use crate::tests::test_types::EnumValueTestCase;

#[test]
fn given_result_enums_when_read_by_value_then_they_match_the_python_members() {
    let test_cases = [
        EnumValueTestCase {
            description: "BuildStatus round-trips its Python values",
            values: values::<BuildStatus>(),
            parsed: parsed::<BuildStatus>(),
            expected_values: vec!["success", "failed"],
        },
        EnumValueTestCase {
            description: "ExecutionStatus round-trips its Python values",
            values: values::<ExecutionStatus>(),
            parsed: parsed::<ExecutionStatus>(),
            expected_values: vec!["success", "failed", "skipped"],
        },
        EnumValueTestCase {
            description: "ExecutionPhase round-trips its Python values",
            values: values::<ExecutionPhase>(),
            parsed: parsed::<ExecutionPhase>(),
            expected_values: vec![
                "pre_hook",
                "staging",
                "schema_change",
                "type_enforcement",
                "contract",
                "audit",
                "promotion",
                "dml",
                "post_hook",
                "fingerprint",
                "microbatch_state",
                "custom_materialization",
            ],
        },
        EnumValueTestCase {
            description: "HookPhase round-trips its Python values",
            values: values::<HookPhase>(),
            parsed: parsed::<HookPhase>(),
            expected_values: vec!["pre_hooks", "post_hooks"],
        },
        EnumValueTestCase {
            description: "SkipMode round-trips its Python values",
            values: values::<SkipMode>(),
            parsed: parsed::<SkipMode>(),
            expected_values: vec!["soft", "hard"],
        },
        EnumValueTestCase {
            description: "MicrobatchLimitAction round-trips its Python values",
            values: values::<MicrobatchLimitAction>(),
            parsed: parsed::<MicrobatchLimitAction>(),
            expected_values: vec!["cap_from_end", "cap_from_start", "error", "warn"],
        },
        EnumValueTestCase {
            description: "ExecutionResourceKind round-trips its Python values",
            values: values::<ExecutionResourceKind>(),
            parsed: parsed::<ExecutionResourceKind>(),
            expected_values: vec![
                "source", "loader", "seed", "udf", "table_fn", "view", "table", "custom",
                "snapshot",
            ],
        },
        EnumValueTestCase {
            description: "SqlTestOutcome round-trips its Python values",
            values: values::<SqlTestOutcome>(),
            parsed: parsed::<SqlTestOutcome>(),
            expected_values: vec!["pass", "fail", "error", "timeout", "refused"],
        },
        EnumValueTestCase {
            description: "ItemKind round-trips its Python values",
            values: values::<ItemKind>(),
            parsed: parsed::<ItemKind>(),
            expected_values: vec!["test", "scenario", "model"],
        },
        EnumValueTestCase {
            description: "CancellationOutcome round-trips its Python values",
            values: values::<CancellationOutcome>(),
            parsed: parsed::<CancellationOutcome>(),
            expected_values: vec![
                "confirmed",
                "unknown",
                "failed",
                "unsupported",
                "not_needed",
            ],
        },
        EnumValueTestCase {
            description: "AuditAttachmentKind round-trips its Python values",
            values: values::<AuditAttachmentKind>(),
            parsed: parsed::<AuditAttachmentKind>(),
            expected_values: vec!["source", "seed", "model", "end"],
        },
        EnumValueTestCase {
            description: "AuditSeverity round-trips its Python values",
            values: values::<AuditSeverity>(),
            parsed: parsed::<AuditSeverity>(),
            expected_values: vec!["warn", "error"],
        },
        EnumValueTestCase {
            description: "AuditOutcome round-trips its Python values",
            values: values::<AuditOutcome>(),
            parsed: parsed::<AuditOutcome>(),
            expected_values: vec!["pass", "warn", "error", "insufficient", "skipped"],
        },
        EnumValueTestCase {
            description: "AuditRunScope round-trips its Python values",
            values: values::<AuditRunScope>(),
            parsed: parsed::<AuditRunScope>(),
            expected_values: vec!["final", "delta_and_final"],
        },
        EnumValueTestCase {
            description: "AttachedAuditTargetKind round-trips its Python values",
            values: values::<AttachedAuditTargetKind>(),
            parsed: parsed::<AttachedAuditTargetKind>(),
            expected_values: vec!["model", "source", "seed"],
        },
        EnumValueTestCase {
            description: "AuditEvaluationMode round-trips its Python values",
            values: values::<AuditEvaluationMode>(),
            parsed: parsed::<AuditEvaluationMode>(),
            expected_values: vec!["violations", "measurement"],
        },
        EnumValueTestCase {
            description: "LifeCycleEventKind round-trips its Python values",
            values: values::<LifeCycleEventKind>(),
            parsed: parsed::<LifeCycleEventKind>(),
            expected_values: vec!["sql", "log"],
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            test_case.values, test_case.expected_values,
            "{}",
            test_case.description
        );
        assert_eq!(
            test_case.parsed,
            test_case
                .expected_values
                .iter()
                .map(|value| Some(*value))
                .collect::<Vec<_>>(),
            "{}",
            test_case.description
        );
    }
}
