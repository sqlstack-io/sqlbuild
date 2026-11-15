use crate::dev_events::main::dev_control_line::dev_control_line;
use crate::dev_events::main::dev_event_line::dev_event_line;
use crate::dev_events::main::parse_dev_control::parse_dev_control;
use crate::dev_events::main::parse_dev_event::parse_dev_event;
use crate::dev_events::models::{DevControl, DevEvent, DevPhase, DevStepState, SourcePosition};

struct ParseTestCase {
    description: &'static str,
    line: &'static str,
    expected_event: Option<DevEvent>,
}

#[test]
fn given_each_event_when_encoding_then_one_prefixed_line_reads_back_unchanged() {
    let events = [
        DevEvent::Step {
            phase: DevPhase::Discover,
            state: DevStepState::Finished,
            detail: Some("412 python nodes, 2280 models".to_owned()),
        },
        DevEvent::Changed {
            paths: vec!["models/orders.sql".to_owned()],
            more: 2,
        },
        DevEvent::Restarted {
            reason: "Python changed".to_owned(),
        },
        DevEvent::Published {
            generation: 2,
            snapshot_id: "b429729b".to_owned(),
            record: serde_json::json!({"snapshot_id": "b429729b", "generation": 2}),
        },
        DevEvent::Failed {
            generation: 4,
            message: "unknown column net_amount_usd".to_owned(),
            position: Some(SourcePosition {
                path: "models/finance/fct_revenue.sql".to_owned(),
                line: 42,
                column: 7,
            }),
        },
    ];

    for event in events {
        let line: String = dev_event_line(&event);

        assert!(line.starts_with("{\"event\":"), "{line}");
        assert_eq!(line.matches('\n').count(), 1, "{line}");
        assert!(line.ends_with('\n'), "{line}");
        assert_eq!(parse_dev_event(&line), Some(event));
    }
}

#[test]
fn given_other_lines_when_parsing_then_only_event_lines_are_events() {
    let test_cases = [
        ParseTestCase {
            description: "a failed publication reply",
            line: "null\n",
            expected_event: None,
        },
        ParseTestCase {
            description: "a publication record whose fields include an event key later on",
            line: "{\"snapshot_id\":\"b4\",\"event\":\"published\"}\n",
            expected_event: None,
        },
        ParseTestCase {
            description: "an unknown event kind",
            line: "{\"event\":\"teleported\"}\n",
            expected_event: None,
        },
        ParseTestCase {
            description: "a restart event",
            line: "{\"event\":\"restarted\",\"reason\":\"Python changed\"}\n",
            expected_event: Some(DevEvent::Restarted {
                reason: "Python changed".to_owned(),
            }),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            parse_dev_event(test_case.line),
            test_case.expected_event,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_controls_when_encoding_then_each_reads_back_and_other_lines_are_ignored() {
    for control in [
        DevControl::Warehouse { connected: true },
        DevControl::Warehouse { connected: false },
        DevControl::Recompile,
    ] {
        assert_eq!(parse_dev_control(&dev_control_line(control)), Some(control));
    }
    assert_eq!(parse_dev_control("q\n"), None);
    assert_eq!(parse_dev_control("{\"control\":\"warehouse\"}\n"), None);
}
