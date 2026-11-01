//! A standalone server loads explicit proxy bindings and rejects ambiguous authentication modes.

use reqwest::{Client, StatusCode, header};
use serde_json::{Value, json};

use crate::helpers::{api_ready, start_control_in, wait_for_exit};
use crate::test_types::{ProxyCliTestCase, ProxyConfigurationTestCase};

#[test]
fn given_an_owner_configured_proxy_when_a_standalone_server_authenticates_then_only_explicit_people_resolve_without_machine_authority()
 {
    let test_cases = [ProxyCliTestCase {
        description: "explicit upstream subject differs from the local username",
        subject: "external-orders-reader",
        expected_status: StatusCode::OK,
    }];
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    for test_case in test_cases {
        runtime.block_on(async {
            let directory = tempfile::tempdir().expect("prepared proxy installation");
            let provisioned = std::process::Command::new(env!("CARGO_BIN_EXE_sqb"))
                .args(["admin", "create-principal", "orders-reader"])
                .current_dir(directory.path()).env("PATH", "")
                .env_remove("SQLBUILD_CONTROL_DATABASE_URL").output().expect("native owner provisioning");
            assert!(provisioned.status.success());
            let person: Value = serde_json::from_slice(&provisioned.stdout).expect("person JSON");
            let principal = person["principal_id"].as_str().expect("person UUID");
            let configuration = json!({"origin": "https://control.example.com", "trusted_peers": ["127.0.0.1"], "identities": {test_case.subject: principal}});
            std::fs::write(directory.path().join("proxy.json"), configuration.to_string()).expect("server-owned identity bindings");
            let mut control = start_control_in(directory, &["server", "--port", "0", "--no-scheduler", "--trusted-proxy-config", "proxy.json"]);
            let url = api_ready(&mut control).await;
            let client = Client::builder().timeout(std::time::Duration::from_secs(5)).build().expect("HTTP client");
            let response = client.get(format!("{url}/api/auth/proxy"))
                .header(header::HOST, "control.example.com").header(header::ORIGIN, "https://control.example.com")
                .header("x-sqb-user", test_case.subject).send().await.expect("upstream identity");
            assert_eq!(response.status(), test_case.expected_status, "{}", test_case.description);
            assert!(!response.headers().contains_key(header::SET_COOKIE));
            assert_eq!(response.json::<Value>().await.expect("identity"), json!({"principal_id": principal}));
            let machine = client.get(format!("{url}/api/locations/{}/health", uuid::Uuid::now_v7()))
                .header("x-sqb-user", test_case.subject).send().await.expect("machine boundary");
            assert_eq!(machine.status(), StatusCode::UNAUTHORIZED);
            #[cfg(unix)]
            crate::helpers::terminate(&mut control).await;
        });
    }
}

#[test]
fn given_invalid_or_conflicting_proxy_configuration_when_a_standalone_server_starts_then_it_fails_closed()
 {
    let test_cases = [
        ProxyConfigurationTestCase {
            description: "malformed JSON",
            configuration: "{",
            arguments: &[],
            expected_error: "trusted-proxy configuration requires valid JSON",
        },
        ProxyConfigurationTestCase {
            description: "unknown configuration fields",
            configuration: r#"{"origin":"https://control.example.com","trusted_peers":["127.0.0.1"],"identities":{},"unknown":"synthetic-unrecognized-value"}"#,
            arguments: &[],
            expected_error: "trusted-proxy configuration requires valid JSON",
        },
        ProxyConfigurationTestCase {
            description: "no trusted peer boundary",
            configuration: r#"{"origin":"https://control.example.com","trusted_peers":[],"identities":{}}"#,
            arguments: &[],
            expected_error: "requires explicit trusted peer IP addresses",
        },
        ProxyConfigurationTestCase {
            description: "non-HTTPS external origin",
            configuration: r#"{"origin":"http://control.example.com","trusted_peers":["127.0.0.1"],"identities":{}}"#,
            arguments: &[],
            expected_error: "trusted-proxy authentication requires an HTTPS origin",
        },
        ProxyConfigurationTestCase {
            description: "subject cannot be represented by the header transport",
            configuration: r#"{"origin":"https://control.example.com","trusted_peers":["127.0.0.1"],"identities":{"external-café":"00000000-0000-0000-0000-000000000001"}}"#,
            arguments: &[],
            expected_error: "subjects must be nonempty, unpadded ASCII header values",
        },
        ProxyConfigurationTestCase {
            description: "ambiguous password/proxy modes",
            configuration: r#"{"origin":"https://control.example.com","trusted_peers":["127.0.0.1"],"identities":{}}"#,
            arguments: &["--password-origin", "https://control.example.com"],
            expected_error: "trusted-proxy authentication cannot be combined",
        },
    ];
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    for test_case in test_cases {
        runtime.block_on(async {
            let directory = tempfile::tempdir().expect("invalid proxy installation");
            std::fs::write(directory.path().join("proxy.json"), test_case.configuration)
                .expect("configuration input");
            let mut arguments = vec![
                "server",
                "--port",
                "0",
                "--no-scheduler",
                "--trusted-proxy-config",
                "proxy.json",
            ];
            arguments.extend_from_slice(test_case.arguments);
            let mut control = start_control_in(directory, &arguments);
            let status = wait_for_exit(&mut control).await;
            assert!(!status.success(), "{}", test_case.description);
            let log = std::fs::read_to_string(&control.stderr).expect("startup failure");
            assert!(
                log.contains(test_case.expected_error),
                "{}",
                test_case.description
            );
            assert!(
                !log.contains(test_case.configuration),
                "configuration contents must not be echoed"
            );
        });
    }
}
