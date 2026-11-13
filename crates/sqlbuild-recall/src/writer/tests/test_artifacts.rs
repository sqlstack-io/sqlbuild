use crate::records::models::RecallValue;
use crate::settings::models::AuditEvidence;
use crate::writer::tests::helpers::{
    failed_statement_sql, folded_statements, kept_evidence, left_out_evidence, recall_text,
    stored_evidence,
};
use crate::writer::tests::test_types::{
    AuditEvidenceTestCase, FailedStatementTestCase, StatementFoldTestCase,
};

#[test]
fn given_audit_evidence_settings_when_audits_complete_then_kept_evidence_is_an_artifact_and_none_stays_in_the_event()
 {
    let test_cases = [
        AuditEvidenceTestCase {
            description: "warn_and_error keeps both outcomes' rows",
            audit_evidence: AuditEvidence::WarnAndError,
            expected_rows: vec![kept_evidence("error", 1), kept_evidence("warn", 0)],
        },
        AuditEvidenceTestCase {
            description: "error keeps only the failing audit's rows",
            audit_evidence: AuditEvidence::Error,
            expected_rows: vec![kept_evidence("error", 1), left_out_evidence("warn")],
        },
        AuditEvidenceTestCase {
            description: "off keeps no rows",
            audit_evidence: AuditEvidence::Off,
            expected_rows: vec![left_out_evidence("error"), left_out_evidence("warn")],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            stored_evidence(test_case.audit_evidence),
            test_case.expected_rows,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_statements_sharing_sql_and_a_failed_one_when_recorded_then_text_is_stored_once_and_every_statement_is_referenced()
 {
    let test_cases = [FailedStatementTestCase {
        description: "two texts, three references, no conflict",
        expected_rows: vec![vec![
            RecallValue::Integer(2),
            RecallValue::Integer(3),
            RecallValue::Integer(0),
            recall_text("s1,s2,s3"),
        ]],
    }];

    for test_case in test_cases {
        assert_eq!(
            failed_statement_sql(),
            test_case.expected_rows,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_statement_started_and_ended_in_one_batch_when_recorded_then_one_row_keeps_its_start_and_a_running_one_keeps_its_start_row()
 {
    let test_cases = [StatementFoldTestCase {
        description: "the ended statement is one completed row; the running one stays a start row",
        expected_rows: vec![
            vec![
                recall_text("done"),
                recall_text("statement_completed"),
                recall_text("2026-10-19T00:00:01.000000Z"),
                recall_text("2026-10-19T00:00:01.000000Z"),
                recall_text("2026-10-19T00:00:03.000000Z"),
                recall_text("completed"),
            ],
            vec![
                recall_text("running"),
                recall_text("statement_started"),
                RecallValue::Null,
                recall_text("2026-10-19T00:00:02.000000Z"),
                RecallValue::Null,
                recall_text("no_terminal"),
            ],
        ],
    }];

    for test_case in test_cases {
        assert_eq!(
            folded_statements(),
            test_case.expected_rows,
            "{}",
            test_case.description
        );
    }
}
