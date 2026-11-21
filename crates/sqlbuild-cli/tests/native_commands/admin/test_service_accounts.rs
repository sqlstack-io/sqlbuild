//! The control-store owner manages service accounts with sqb admin; a key printed once calls the server.

use std::time::Duration;

use reqwest::{StatusCode, header};
use serde_json::Value;

use crate::admin_helpers::{administer, invoke_admin_with};
use crate::helpers::{api_ready, start_control};
use crate::test_types::ServiceAccountCliTestCase;

const KEY_VARIABLE: &str = "SQB_TEST_IMPORTED_KEY";

async fn bootstrap_status(url: &str, key: &str) -> StatusCode {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("HTTP client")
        .get(format!("{url}/api/control/bootstrap"))
        .header(header::HOST, "10.20.30.40:8000")
        .bearer_auth(key)
        .send()
        .await
        .expect("keyed request")
        .status()
}

#[test]
fn given_a_control_store_owner_when_service_accounts_are_managed_with_sqb_admin_then_printed_keys_call_the_server()
 {
    let test_cases = [
        ServiceAccountCliTestCase {
            description: "a generated key is printed once and works until rotated away or disabled",
            arguments: &["--role", "developer", "--description", "Release pipeline"],
            imported_key: None,
            expected_error: None,
        },
        ServiceAccountCliTestCase {
            description: "an imported key of 32 characters after sqbk_ works and isn't printed",
            arguments: &["--role", "viewer", "--key-from-env", KEY_VARIABLE],
            imported_key: Some("sqbk_0123456789abcdefghijABCDEFGHIJKL"),
            expected_error: None,
        },
        ServiceAccountCliTestCase {
            description: "an imported key shorter than 32 characters after sqbk_ is refused",
            arguments: &["--role", "viewer", "--key-from-env", KEY_VARIABLE],
            imported_key: Some("sqbk_too-short-for-a-key"),
            expected_error: Some("at least 32 characters"),
        },
        ServiceAccountCliTestCase {
            description: "--key-from-env naming an unset variable is refused",
            arguments: &["--role", "viewer", "--key-from-env", "SQB_TEST_UNSET_KEY"],
            imported_key: None,
            expected_error: Some("SQB_TEST_UNSET_KEY, which is not set"),
        },
    ];
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    for test_case in test_cases {
        runtime.block_on(async {
            let what = test_case.description;
            let mut control = start_control(&["server", "--port", "0", "--no-scheduler"]);
            let url = api_ready(&mut control).await;
            let mut arguments = vec!["create-service-account", "orders-deploy"];
            arguments.extend_from_slice(test_case.arguments);
            let output = invoke_admin_with(
                &control,
                &arguments,
                (KEY_VARIABLE, test_case.imported_key.unwrap_or("")),
            );
            if let Some(expected) = test_case.expected_error {
                assert!(!output.status.success(), "{what}");
                assert!(output.stdout.is_empty(), "{what}: nothing printed");
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert!(stderr.contains(expected), "{what}: {stderr}");
                let listed = administer(&control, &["list-service-accounts"]);
                assert_eq!(listed, Value::Array(Vec::new()), "{what}: nothing created");
            } else {
                assert!(
                    output.status.success(),
                    "{what}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let created: Value = serde_json::from_slice(&output.stdout).expect("JSON result");
                let key = match test_case.imported_key {
                    Some(imported) => {
                        assert!(
                            created.get("key").is_none(),
                            "{what}: an imported key isn't echoed"
                        );
                        imported.to_owned()
                    }
                    None => created["key"].as_str().expect("printed key").to_owned(),
                };
                assert_eq!(created["key_prefix"].as_str(), Some(&key[..11]), "{what}");
                assert_eq!(
                    bootstrap_status(&url, &key).await,
                    StatusCode::OK,
                    "{what}: key works"
                );
                let listed = administer(&control, &["list-service-accounts"]).to_string();
                assert!(
                    listed.contains("orders-deploy") && !listed.contains(&key[5..]),
                    "{what}: listed without its key"
                );

                let rotated = administer(
                    &control,
                    &[
                        "rotate-service-account",
                        "orders-deploy",
                        "--overlap-minutes",
                        "30",
                    ],
                );
                let second = rotated["key"].as_str().expect("rotated key").to_owned();
                assert_eq!(
                    bootstrap_status(&url, &key).await,
                    StatusCode::OK,
                    "{what}: overlap"
                );
                assert_eq!(
                    bootstrap_status(&url, &second).await,
                    StatusCode::OK,
                    "{what}: new key"
                );
                let third =
                    administer(&control, &["rotate-service-account", "orders-deploy"])["key"]
                        .as_str()
                        .expect("third key")
                        .to_owned();
                for stopped in [&key, &second] {
                    assert_eq!(
                        bootstrap_status(&url, stopped).await,
                        StatusCode::UNAUTHORIZED,
                        "{what}: replaced"
                    );
                }
                administer(&control, &["disable-service-account", "orders-deploy"]);
                assert_eq!(
                    bootstrap_status(&url, &third).await,
                    StatusCode::UNAUTHORIZED,
                    "{what}: disabled"
                );
                administer(&control, &["enable-service-account", "orders-deploy"]);
                assert_eq!(
                    bootstrap_status(&url, &third).await,
                    StatusCode::OK,
                    "{what}: enabled"
                );
            }
        });
    }
}
