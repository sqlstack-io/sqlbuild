use crate::dev_events::main::dev_control_line::dev_control_line;
use crate::dev_events::main::dev_event_line::dev_event_line;
use crate::dev_events::main::generation_failed::generation_failed;
use crate::dev_events::main::parse_dev_control::parse_dev_control;
use crate::dev_events::main::parse_dev_event::parse_dev_event;
use crate::dev_events::models::{DevControl, DevEvent, DevPhase, DevStepState, SourcePosition};
use crate::dev_events::tests::test_types::{
    ControlTestCase, ParseTestCase, PositionTestCase, RoundTripTestCase,
};

#[test]
fn given_each_event_when_encoding_then_one_prefixed_line_reads_back_unchanged() {
    let test_cases = [
        RoundTripTestCase {
            description: "a finished step keeps its detail",
            event: DevEvent::Step {
                phase: DevPhase::Discover,
                state: DevStepState::Finished,
                detail: Some("412 python nodes, 2280 models".to_owned()),
            },
            expected_newlines: 1,
        },
        RoundTripTestCase {
            description: "changed paths keep the count of further files",
            event: DevEvent::Changed {
                paths: vec!["sql/models/orders.sql".to_owned()],
                more: 2,
            },
            expected_newlines: 1,
        },
        RoundTripTestCase {
            description: "a restart keeps its reason",
            event: DevEvent::Restarted {
                reason: "Python changed".to_owned(),
            },
            expected_newlines: 1,
        },
        RoundTripTestCase {
            description: "a publication keeps its whole record on the same line",
            event: DevEvent::Published {
                generation: 2,
                snapshot_id: "b429729b".to_owned(),
                record: serde_json::json!({"snapshot_id": "b429729b", "generation": 2}),
            },
            expected_newlines: 1,
        },
        RoundTripTestCase {
            description: "a failure keeps its source position",
            event: DevEvent::Failed {
                generation: 4,
                message: "unknown column net_amount_usd\nat line 42".to_owned(),
                position: Some(SourcePosition {
                    path: "sql/models/finance/fct_revenue.sql".to_owned(),
                    line: 42,
                    column: 7,
                }),
                code: Some("D001".to_owned()),
                help: Some("rename the column".to_owned()),
            },
            expected_newlines: 1,
        },
    ];

    for test_case in test_cases {
        let line: String = dev_event_line(&test_case.event);

        assert!(
            line.starts_with("{\"event\":"),
            "{}: {line}",
            test_case.description
        );
        assert_eq!(
            line.matches('\n').count(),
            test_case.expected_newlines,
            "{}",
            test_case.description
        );
        assert!(line.ends_with('\n'), "{}: {line}", test_case.description);
        assert_eq!(
            parse_dev_event(&line),
            Some(test_case.event),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_other_lines_when_parsing_then_only_event_lines_are_events() {
    let test_cases = [
        ParseTestCase {
            description: "a failed publication reply",
            line: "null\n".to_owned(),
            expected_event: None,
        },
        ParseTestCase {
            description: "a publication record whose fields include an event key later on",
            line: "{\"snapshot_id\":\"b4\",\"event\":\"published\"}\n".to_owned(),
            expected_event: None,
        },
        ParseTestCase {
            description: "an unknown event kind",
            line: "{\"event\":\"teleported\"}\n".to_owned(),
            expected_event: None,
        },
        ParseTestCase {
            description: "a restart event",
            line: "{\"event\":\"restarted\",\"reason\":\"Python changed\"}\n".to_owned(),
            expected_event: Some(DevEvent::Restarted {
                reason: "Python changed".to_owned(),
            }),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            parse_dev_event(&test_case.line),
            test_case.expected_event,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_controls_when_encoding_then_each_reads_back_and_other_lines_are_ignored() {
    let test_cases = [
        ControlTestCase {
            description: "a connected warehouse verdict reads back",
            line: dev_control_line(DevControl::Warehouse { connected: true }),
            expected_control: Some(DevControl::Warehouse { connected: true }),
        },
        ControlTestCase {
            description: "a failed warehouse verdict reads back",
            line: dev_control_line(DevControl::Warehouse { connected: false }),
            expected_control: Some(DevControl::Warehouse { connected: false }),
        },
        ControlTestCase {
            description: "a recompile request reads back",
            line: dev_control_line(DevControl::Recompile),
            expected_control: Some(DevControl::Recompile),
        },
        ControlTestCase {
            description: "a stray key press is not a control",
            line: "q\n".to_owned(),
            expected_control: None,
        },
        ControlTestCase {
            description: "a warehouse control without its verdict is ignored",
            line: "{\"control\":\"warehouse\"}\n".to_owned(),
            expected_control: None,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            parse_dev_control(&test_case.line),
            test_case.expected_control,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_failed_generation_when_reported_then_the_first_source_location_is_its_position() {
    let test_cases = [
        PositionTestCase {
            description: "a compile error with line and column",
            message: "SQB0101 sql/models/finance/fct_revenue.sql:42:7: unknown column net_amount_usd",
            expected_position: Some(SourcePosition {
                path: "sql/models/finance/fct_revenue.sql".to_owned(),
                line: 42,
                column: 7,
            }),
        },
        PositionTestCase {
            description: "a Python traceback line without a column, in parentheses",
            message: "failed to import (python/nodes/orders.py:12) because of a syntax error",
            expected_position: Some(SourcePosition {
                path: "python/nodes/orders.py".to_owned(),
                line: 12,
                column: 1,
            }),
        },
        PositionTestCase {
            description: "a URL with a port is not a source position",
            message: "Publication outcome is unknown for http://127.0.0.1:8000/snapshots",
            expected_position: None,
        },
        PositionTestCase {
            description: "a path without a line is not a position",
            message: "sql/models/orders.sql is not valid",
            expected_position: None,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            generation_failed(4, test_case.message.to_owned(), (None, None)),
            DevEvent::Failed {
                generation: 4,
                message: test_case.message.to_owned(),
                position: test_case.expected_position,
                code: None,
                help: None,
            },
            "{}",
            test_case.description
        );
    }
}
