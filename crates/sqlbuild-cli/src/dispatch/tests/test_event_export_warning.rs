use crate::dispatch::main::event_export_warning::event_export_warning;
use crate::dispatch::models::ExportSummary;
use crate::dispatch::tests::helpers::export_summary;
use crate::dispatch::tests::test_types::EventExportWarningTestCase;

#[test]
fn given_final_export_summary_when_formatting_warning_then_reports_loss_only() {
    let test_cases = [
        EventExportWarningTestCase {
            description: "all delivered",
            summary: export_summary(&[("orders_events", 248, 248, 0, 0)]),
            command_succeeded: true,
            expected_warning: None,
        },
        EventExportWarningTestCase {
            description: "successful command with drops",
            summary: export_summary(&[("orders_events", 248, 108, 0, 140)]),
            command_succeeded: true,
            expected_warning: Some(
                "Warning: command completed successfully, but lifecycle event export was \
                 incomplete: 140 of 248 events dropped, 0 failed (sink 'orders_events'). \
                 Check sink health: a sink call slower than 1s drops that sink's events until it \
                 returns; events still queued at exit need a longer \
                 event_sinks.lifecycle.shutdown_timeout.",
            ),
        },
        EventExportWarningTestCase {
            description: "failed command with failures",
            summary: export_summary(&[("orders_events", 10, 9, 1, 0)]),
            command_succeeded: false,
            expected_warning: Some(
                "Warning: lifecycle event export was incomplete: 0 of 10 events dropped, \
                 1 failed (sink 'orders_events'). \
                 Check sink health: a sink call slower than 1s drops that sink's events until it \
                 returns; events still queued at exit need a longer \
                 event_sinks.lifecycle.shutdown_timeout.",
            ),
        },
        EventExportWarningTestCase {
            description: "multiple sinks names only affected sinks",
            summary: export_summary(&[
                ("customer_events", 20, 20, 0, 0),
                ("orders_events", 20, 15, 1, 4),
            ]),
            command_succeeded: true,
            expected_warning: Some(
                "Warning: command completed successfully, but lifecycle event export was \
                 incomplete: 4 of 40 event deliveries dropped, 1 failed (sink 'orders_events'). \
                 Check sink health: a sink call slower than 1s drops that sink's events until it \
                 returns; events still queued at exit need a longer \
                 event_sinks.lifecycle.shutdown_timeout.",
            ),
        },
        EventExportWarningTestCase {
            description: "a sink provider teardown that raised",
            summary: ExportSummary {
                teardown_failures: vec!["'kafka' (KafkaProducer) raised KafkaException".into()],
                ..export_summary(&[("orders_events", 10, 10, 0, 0)])
            },
            command_succeeded: true,
            expected_warning: Some(
                "Warning: command completed successfully, but lifecycle event export was \
                 incomplete: sink provider teardown failed ('kafka' (KafkaProducer) raised \
                 KafkaException); events the provider still held may not have been delivered.",
            ),
        },
        EventExportWarningTestCase {
            description: "drops and a failed teardown together",
            summary: ExportSummary {
                teardown_failures: vec!["'kafka' (KafkaProducer) raised KafkaException".into()],
                ..export_summary(&[("orders_events", 10, 9, 1, 0)])
            },
            command_succeeded: false,
            expected_warning: Some(
                "Warning: lifecycle event export was incomplete: 0 of 10 events dropped, \
                 1 failed (sink 'orders_events'). \
                 Check sink health: a sink call slower than 1s drops that sink's events until it \
                 returns; events still queued at exit need a longer \
                 event_sinks.lifecycle.shutdown_timeout. sink provider teardown failed \
                 ('kafka' (KafkaProducer) raised KafkaException); events the provider still held \
                 may not have been delivered.",
            ),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            event_export_warning(&test_case.summary, test_case.command_succeeded).as_deref(),
            test_case.expected_warning,
            "{}",
            test_case.description
        );
    }
}
