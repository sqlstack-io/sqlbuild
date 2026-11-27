//! A worker sends `ctx.log` lines from a background thread: a node never waits, lines past the cap are counted, and only acknowledged lines count as sent.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::run_output::constants::NODE_LOGS_PER_NODE;
use crate::run_output::main::node_log_overflow::node_log_overflow;
use crate::run_output::models::NodeLogSender;
use crate::run_output::tests::helpers::{close_sender, flush_node};
use crate::run_output::tests::test_types::{
    NodeLogSendTestCase, RecordingReport, SlowNodeLogTestCase,
};
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
            finish: flush_node,
            expected_reported: 3,
            expected_last_sent: true,
            expected_overflow: None,
        },
        NodeLogSendTestCase {
            description: "lines past the cap are counted, not sent, and a warning says how many",
            lines: NODE_LOGS_PER_NODE + 5,
            acknowledged: true,
            finish: flush_node,
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
            finish: flush_node,
            expected_reported: 2,
            expected_last_sent: false,
            expected_overflow: None,
        },
        NodeLogSendTestCase {
            description: "closing the sender at the end of the run sends what is waiting",
            lines: 4,
            acknowledged: true,
            finish: close_sender,
            expected_reported: 4,
            expected_last_sent: true,
            expected_overflow: None,
        },
    ];
    for test_case in test_cases {
        let reported: Arc<Mutex<Vec<(String, RunChange)>>> = Arc::default();
        let (arrivals, _arrived): (Sender<String>, Receiver<String>) = channel();
        let sender = NodeLogSender::new(Box::new(RecordingReport {
            reported: Arc::clone(&reported),
            acknowledged: test_case.acknowledged,
            arrivals: Mutex::new(arrivals),
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
        (test_case.finish)(&sender, NODE);
        let reported = reported.lock().expect("reported lines").clone();
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
    let test_cases = [SlowNodeLogTestCase {
        description: "one line is sent on the sender's timer, with no node finishing",
        message: "step 1",
        wait: Duration::from_secs(5),
        expected_key: "node-log/asset:fulfil[pack]/0",
    }];
    for test_case in test_cases {
        let (arrivals, arrived): (Sender<String>, Receiver<String>) = channel();
        let sender = NodeLogSender::new(Box::new(RecordingReport {
            reported: Arc::default(),
            acknowledged: true,
            arrivals: Mutex::new(arrivals),
        }));
        sender.push(NODE, (NodeLogLevel::Info, test_case.message));

        assert_eq!(
            arrived.recv_timeout(test_case.wait),
            Ok(test_case.expected_key.to_owned()),
            "{}",
            test_case.description
        );
        sender.close();
        assert!(
            sender.was_sent(test_case.expected_key),
            "{}",
            test_case.description
        );
    }
}
