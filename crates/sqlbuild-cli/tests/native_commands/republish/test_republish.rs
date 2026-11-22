//! An API key republishes over plain HTTP with one warning; a saved session still needs HTTPS.

use crate::republish_helpers::{NETWORK_ORIGIN, republish_through_proxy};
use crate::republish_test_types::RepublishCliTestCase;

const KEY: &str = "sqbk_synthetic_orders_deploy_key_0000000000";
const WARNING: &str = "travels unencrypted over HTTP";

#[test]
fn given_a_plain_http_network_server_when_republishing_then_a_key_warns_and_a_session_requires_https()
 {
    let test_cases = [
        RepublishCliTestCase {
            description: "a key is sent over plain HTTP",
            key: Some(KEY),
            states: &["pending"],
            arguments: &[],
            expected_exit: 0,
            expected_methods: &["POST"],
            expected_warnings: 1,
            expected_stderr: "Republish accepted",
        },
        RepublishCliTestCase {
            description: "waiting polls with the same key and warns once",
            key: Some(KEY),
            states: &["pending", "pending", "published"],
            arguments: &["--wait"],
            expected_exit: 0,
            expected_methods: &["POST", "GET", "GET"],
            expected_warnings: 1,
            expected_stderr: "Republish published a snapshot",
        },
        RepublishCliTestCase {
            description: "a failed publish exits one",
            key: Some(KEY),
            states: &["pending", "failed"],
            arguments: &["--wait"],
            expected_exit: 1,
            expected_methods: &["POST", "GET"],
            expected_warnings: 1,
            expected_stderr: "Republish failed",
        },
        RepublishCliTestCase {
            description: "waiting no seconds reports a pending republish",
            key: Some(KEY),
            states: &["pending"],
            arguments: &["--wait", "--timeout", "0"],
            expected_exit: 2,
            expected_methods: &["POST"],
            expected_warnings: 1,
            expected_stderr: "Republish is still pending",
        },
        RepublishCliTestCase {
            description: "a session refuses plain HTTP before sending anything",
            key: None,
            states: &["pending"],
            arguments: &[],
            expected_exit: 2,
            expected_methods: &[],
            expected_warnings: 0,
            expected_stderr: "Republish request failed",
        },
        RepublishCliTestCase {
            description: "a blank key is not a session",
            key: Some(" "),
            states: &["pending"],
            arguments: &[],
            expected_exit: 2,
            expected_methods: &[],
            expected_warnings: 0,
            expected_stderr: "SQLBUILD_API_KEY must not be blank",
        },
    ];

    for test_case in test_cases {
        let (output, requests) =
            republish_through_proxy(test_case.key, test_case.states, test_case.arguments);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let methods: Vec<&str> = requests
            .iter()
            .filter_map(|head| head.split_whitespace().next())
            .collect();
        let bearers: usize = requests
            .iter()
            .filter(|head| {
                head.to_ascii_lowercase()
                    .contains(&format!("authorization: bearer {KEY}").to_ascii_lowercase())
            })
            .filter(|head| head.contains(&format!(" {NETWORK_ORIGIN}/api/control/locations/")))
            .count();

        assert_eq!(
            (
                output.status.code(),
                methods,
                bearers,
                stderr.matches(WARNING).count(),
                stderr.contains(test_case.expected_stderr),
                stderr.contains(KEY) || stdout.contains(KEY),
            ),
            (
                Some(test_case.expected_exit),
                test_case.expected_methods.to_vec(),
                test_case.expected_methods.len(),
                test_case.expected_warnings,
                true,
                false,
            ),
            "{}: {stderr}",
            test_case.description
        );
    }
}
