//! A worker sends `ctx.log` lines from a background thread: a node never waits, lines past the cap are counted, and only acknowledged lines count as sent.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::run_output::constants::NODE_LOGS_PER_NODE;
use crate::run_output::main::node_log_overflow::node_log_overflow;
use crate::run_output::models::NodeLogSender;
use crate::run_output::tests::test_types::NodeLogSendTestCase;
use crate::runs::models::{NodeLogLevel, RunChange};

const NODE: &str = "asset:fulfil[pack]";

#[test]
fn given_a_node_logging_when_its_lines_are_sent_then_the_server_gets_the_kept_ones_under_stable_keys()
 {
    let test_cases = [
        NodeLogSendTestCase {
            description: "a few lines reach the server before the node finishes",
            lines: 3,
            acknowledged: true,
            flushed: true,
            expected_reported: 3,
            expected_last_sent: true,
            expected_overflow: None,
        },
        NodeLogSendTestCase {
            description: "lines past the cap are counted, not sent, and a warning says how many",
            lines: NODE_LOGS_PER_NODE + 5,
            acknowledged: true,
            flushed: true,
            expected_reported: NODE_LOGS_PER_NODE,
            expected_last_sent: true,
            expected_overflow: Some(
                "5 more lines not kept as log lines; ctx.log keeps 200 per node, and print() output is kept in full in Output",
            ),
        },
        NodeLogSendTestCase {
            description: "a line the server refused is left for the finishing node to report",
            lines: 2,
            acknowledged: false,
            flushed: true,
            expected_reported: 2,
            expected_last_sent: false,
            expected_overflow: None,
        },
        NodeLogSendTestCase {
            description: "closing the sender at the end of the run sends what is waiting",
            lines: 4,
            acknowledged: true,
            flushed: false,
            expected_reported: 4,
            expected_last_sent: true,
            expected_overflow: None,
        },
    ];
    for test_case in test_cases {
        let reported: Arc<Mutex<Vec<(String, RunChange)>>> = Arc::default();
        let seen: Arc<Mutex<Vec<(String, RunChange)>>> = Arc::clone(&reported);
        let acknowledged: bool = test_case.acknowledged;
        let sender = NodeLogSender::new(Box::new(move |key: &str, change: &RunChange| {
            std::thread::sleep(Duration::from_millis(1));
            seen.lock().unwrap().push((key.to_owned(), change.clone()));
            acknowledged
        }));
        let started: Instant = Instant::now();
        for index in 0..test_case.lines {
            sender.push(NODE, (NodeLogLevel::Info, &format!("step {index}")));
        }
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "{}: logging waited on the server",
            test_case.description
        );
        if test_case.flushed {
            sender.flush(NODE);
        } else {
            sender.close();
        }
        let reported = reported.lock().unwrap().clone();
        let last: usize = test_case.expected_reported - 1;
        assert_eq!(
            reported.len(),
            test_case.expected_reported,
            "{}",
            test_case.description
        );
        assert_eq!(
            reported.last().map(|(key, _)| key.clone()),
            Some(format!("node-log/{NODE}/{last}")),
            "{}",
            test_case.description
        );
        assert_eq!(
            reported.last().map(|(_, change)| change.clone()),
            Some(RunChange::NodeLog {
                node_key: NODE.into(),
                level: NodeLogLevel::Info,
                message: format!("step {last}"),
            }),
            "{}",
            test_case.description
        );
        assert_eq!(
            sender.was_sent(&format!("node-log/{NODE}/{last}")),
            test_case.expected_last_sent,
            "{}",
            test_case.description
        );
        assert_eq!(
            sender.logged(NODE),
            test_case.lines,
            "{}",
            test_case.description
        );
        assert_eq!(
            node_log_overflow(NODE, test_case.lines),
            test_case.expected_overflow.map(|message| (
                format!("node-log/{NODE}/{NODE_LOGS_PER_NODE}"),
                RunChange::NodeLog {
                    node_key: NODE.into(),
                    level: NodeLogLevel::Warning,
                    message: message.into(),
                }
            )),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_lines_arriving_slowly_when_no_node_finishes_then_they_are_sent_within_about_a_second() {
    let reported: Arc<Mutex<Vec<String>>> = Arc::default();
    let seen: Arc<Mutex<Vec<String>>> = Arc::clone(&reported);
    let sender = NodeLogSender::new(Box::new(move |key: &str, _: &RunChange| {
        seen.lock().unwrap().push(key.to_owned());
        true
    }));
    sender.push(NODE, (NodeLogLevel::Info, "step 1"));
    let deadline: Instant = Instant::now() + Duration::from_secs(5);
    while reported.lock().unwrap().is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }

    assert_eq!(
        *reported.lock().unwrap(),
        vec![format!("node-log/{NODE}/0")]
    );
    assert!(sender.was_sent(&format!("node-log/{NODE}/0")));
}
