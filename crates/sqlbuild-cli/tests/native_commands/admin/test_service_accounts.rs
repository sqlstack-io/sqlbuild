//! The control-store owner manages service accounts with sqb admin; a key printed once calls the server.

use serde_json::Value;

use crate::admin_helpers::{administer, invoke_admin_with, keyed_bootstrap_status};
use crate::helpers::{api_ready, start_control};
use crate::test_types::{
    GeneratedKeyCliTestCase, ImportedKeyCliTestCase, RefusedServiceAccountCliTestCase,
};

const KEY_VARIABLE: &str = "SQB_TEST_IMPORTED_KEY";

#[test]
fn given_a_generated_key_when_the_owner_rotates_and_disables_with_sqb_admin_then_only_working_keys_call_the_server()
 {
    let test_cases = [GeneratedKeyCliTestCase {
        description: "a key printed once works until rotated away or disabled",
        arguments: &["--role", "developer", "--description", "Release pipeline"],
        expected_role: "developer",
        expected_statuses: [200, 200, 401, 401, 200],
    }];
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    for test_case in test_cases {
        runtime.block_on(async {
            let mut control = start_control(&["server", "--port", "0", "--no-scheduler"]);
            let url = api_ready(&mut control).await;
            let mut arguments = vec!["create-service-account", "orders-deploy"];
            arguments.extend_from_slice(test_case.arguments);
            let created = administer(&control, &arguments);
            let key = created["key"].as_str().expect("printed key").to_owned();
            let listed = administer(&control, &["list-service-accounts"]).to_string();
            assert_eq!(
                (
                    created["role"].as_str(),
                    created["key_prefix"].as_str(),
                    listed.contains("orders-deploy") && !listed.contains(&key[5..])
                ),
                (Some(test_case.expected_role), Some(&key[..11]), true),
                "{}",
                test_case.description
            );
            let made = keyed_bootstrap_status(&url, &key).await;
            administer(
                &control,
                &[
                    "rotate-service-account",
                    "orders-deploy",
                    "--overlap-minutes",
                    "30",
                ],
            );
            let in_overlap = keyed_bootstrap_status(&url, &key).await;
            let third = administer(&control, &["rotate-service-account", "orders-deploy"]);
            let third = third["key"].as_str().expect("third key").to_owned();
            let replaced = keyed_bootstrap_status(&url, &key).await;
            administer(&control, &["disable-service-account", "orders-deploy"]);
            let disabled = keyed_bootstrap_status(&url, &third).await;
            administer(&control, &["enable-service-account", "orders-deploy"]);
            let enabled = keyed_bootstrap_status(&url, &third).await;
            assert_eq!(
                [made, in_overlap, replaced, disabled, enabled],
                test_case.expected_statuses,
                "{}",
                test_case.description
            );
        });
    }
}

#[test]
fn given_a_key_in_an_environment_variable_when_the_owner_imports_it_then_it_calls_the_server_unprinted()
 {
    let test_cases = [ImportedKeyCliTestCase {
        description: "an imported key of 32 characters after sqbk_",
        key: "sqbk_0123456789abcdefghijABCDEFGHIJKL",
        expected_status: 200,
        expected_key_printed: false,
    }];
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    for test_case in test_cases {
        runtime.block_on(async {
            let mut control = start_control(&["server", "--port", "0", "--no-scheduler"]);
            let url = api_ready(&mut control).await;
            let output = invoke_admin_with(
                &control,
                &[
                    "create-service-account",
                    "orders-deploy",
                    "--role",
                    "viewer",
                    "--key-from-env",
                    KEY_VARIABLE,
                ],
                (KEY_VARIABLE, test_case.key),
            );
            let created: Value = serde_json::from_slice(&output.stdout).unwrap_or(Value::Null);
            assert_eq!(
                (
                    output.status.success(),
                    created.get("key").is_some(),
                    keyed_bootstrap_status(&url, test_case.key).await
                ),
                (
                    true,
                    test_case.expected_key_printed,
                    test_case.expected_status
                ),
                "{}",
                test_case.description
            );
        });
    }
}

#[test]
fn given_a_bad_import_when_the_owner_creates_a_service_account_then_it_is_refused_and_nothing_is_created()
 {
    let test_cases = [
        RefusedServiceAccountCliTestCase {
            description: "an imported key shorter than 32 characters after sqbk_",
            arguments: &["--role", "viewer", "--key-from-env", KEY_VARIABLE],
            imported_key: "sqbk_too-short-for-a-key",
            expected_error: "at least 32 characters",
        },
        RefusedServiceAccountCliTestCase {
            description: "--key-from-env naming an unset variable",
            arguments: &["--role", "viewer", "--key-from-env", "SQB_TEST_UNSET_KEY"],
            imported_key: "",
            expected_error: "SQB_TEST_UNSET_KEY, which is not set",
        },
    ];
    for test_case in test_cases {
        let control = start_control(&["server", "--port", "0", "--no-scheduler"]);
        let mut arguments = vec!["create-service-account", "orders-deploy"];
        arguments.extend_from_slice(test_case.arguments);
        let output =
            invoke_admin_with(&control, &arguments, (KEY_VARIABLE, test_case.imported_key));
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        assert_eq!(
            (
                output.status.success(),
                output.stdout.is_empty(),
                stderr.contains(test_case.expected_error),
                administer(&control, &["list-service-accounts"])
            ),
            (false, true, true, Value::Array(Vec::new())),
            "{}: {stderr}",
            test_case.description
        );
    }
}
