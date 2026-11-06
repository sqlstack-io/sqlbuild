//! A standalone server authenticates each request from explicit proxy bindings and rejects ambiguous modes.

use reqwest::{Client, StatusCode, header};
use serde_json::{Value, json};

use crate::helpers::{api_ready, start_control_in, wait_for_exit};
use crate::test_types::{AuthenticationConfigurationTestCase, ProxyCliTestCase};

#[test]
fn given_an_owner_configured_proxy_when_requests_carry_a_subject_then_only_explicit_people_resolve_without_sessions_or_machine_authority()
 {
    let test_cases = [ProxyCliTestCase {
        description: "explicit upstream subject differs from the local username",
        subject: "external-orders-reader",
        expected_statuses: [
            StatusCode::FORBIDDEN,
            StatusCode::UNAUTHORIZED,
            StatusCode::UNAUTHORIZED,
        ],
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
            let runs = format!("{url}/api/control/locations/{}/targets/development/runs", uuid::Uuid::now_v7());
            let read = || client.get(&runs).header(header::HOST, "control.example.com");
            let statuses = [
                read().header("x-sqb-user", test_case.subject).send().await.expect("bound subject").status(),
                read().header("x-sqb-user", "orders-reader").send().await.expect("local name is not a subject").status(),
                read().send().await.expect("missing subject").status(),
            ];
            assert_eq!(statuses, test_case.expected_statuses, "{}", test_case.description);
            let session = client.get(format!("{url}/api/auth/session"))
                .header(header::HOST, "control.example.com").header("x-sqb-user", test_case.subject)
                .send().await.expect("no session is minted");
            assert_eq!(session.status(), StatusCode::UNAUTHORIZED);
            let machine = client.get(format!("{url}/api/locations/{}/health", uuid::Uuid::now_v7()))
                .header("x-sqb-user", test_case.subject).send().await.expect("machine boundary");
            assert_eq!(machine.status(), StatusCode::UNAUTHORIZED);
            #[cfg(unix)]
            crate::helpers::terminate(&mut control).await;
        });
    }
}

#[test]
fn given_invalid_or_conflicting_identity_configuration_when_a_standalone_server_starts_then_it_fails_closed()
 {
    let test_cases = [
        AuthenticationConfigurationTestCase {
            description: "malformed JSON",
            option: "--trusted-proxy-config",
            configuration: "{",
            arguments: &[],
            expected_error: "trusted-proxy configuration requires valid JSON",
        },
        AuthenticationConfigurationTestCase {
            description: "unknown configuration fields",
            option: "--trusted-proxy-config",
            configuration: r#"{"origin":"https://control.example.com","trusted_peers":["127.0.0.1"],"identities":{},"unknown":"synthetic-unrecognized-value"}"#,
            arguments: &[],
            expected_error: "trusted-proxy configuration requires valid JSON",
        },
        AuthenticationConfigurationTestCase {
            description: "no trusted peer boundary",
            option: "--trusted-proxy-config",
            configuration: r#"{"origin":"https://control.example.com","trusted_peers":[],"identities":{}}"#,
            arguments: &[],
            expected_error: "requires explicit trusted peer IP addresses",
        },
        AuthenticationConfigurationTestCase {
            description: "non-HTTPS external origin",
            option: "--trusted-proxy-config",
            configuration: r#"{"origin":"http://control.example.com","trusted_peers":["127.0.0.1"],"identities":{}}"#,
            arguments: &[],
            expected_error: "trusted-proxy authentication requires an HTTPS origin",
        },
        AuthenticationConfigurationTestCase {
            description: "subject cannot be represented by the header transport",
            option: "--trusted-proxy-config",
            configuration: r#"{"origin":"https://control.example.com","trusted_peers":["127.0.0.1"],"identities":{"external-café":"00000000-0000-0000-0000-000000000001"}}"#,
            arguments: &[],
            expected_error: "subjects must be nonempty, unpadded ASCII header values",
        },
        AuthenticationConfigurationTestCase {
            description: "ambiguous password/proxy modes",
            option: "--trusted-proxy-config",
            configuration: r#"{"origin":"https://control.example.com","trusted_peers":["127.0.0.1"],"identities":{}}"#,
            arguments: &["--password-origin", "https://control.example.com"],
            expected_error: "trusted-proxy authentication cannot be combined",
        },
        AuthenticationConfigurationTestCase {
            description: "OIDC malformed configuration",
            option: "--oidc-config",
            configuration: "{",
            arguments: &[],
            expected_error: "OIDC configuration requires valid JSON",
        },
        AuthenticationConfigurationTestCase {
            description: "OIDC unknown configuration fields",
            option: "--oidc-config",
            configuration: r#"{"origin":"https://control.example.com","issuer":"https://identity.example.com","client_id":"orders-control","identities":{},"unknown":"synthetic-unrecognized-value"}"#,
            arguments: &[],
            expected_error: "OIDC configuration requires valid JSON",
        },
        AuthenticationConfigurationTestCase {
            description: "OIDC rejects plaintext issuer",
            option: "--oidc-config",
            configuration: r#"{"origin":"https://control.example.com","issuer":"http://identity.example.com","client_id":"orders-control","identities":{}}"#,
            arguments: &[],
            expected_error: "OIDC endpoints require HTTPS URLs without credentials",
        },
        AuthenticationConfigurationTestCase {
            description: "OIDC rejects plaintext browser origin",
            option: "--oidc-config",
            configuration: r#"{"origin":"http://control.example.com","issuer":"https://identity.example.com","client_id":"orders-control","identities":{}}"#,
            arguments: &[],
            expected_error: "OIDC authentication requires an HTTPS origin",
        },
        AuthenticationConfigurationTestCase {
            description: "OIDC rejects missing client secret environment variable",
            option: "--oidc-config",
            configuration: r#"{"origin":"https://control.example.com","issuer":"https://identity.example.com","client_id":"orders-control","identities":{},"client_secret_env":"SQLBUILD_SYNTHETIC_OIDC_MISSING"}"#,
            arguments: &[],
            expected_error: "OIDC client-secret environment variable is unavailable",
        },
        AuthenticationConfigurationTestCase {
            description: "OIDC rejects missing certificate authority",
            option: "--oidc-config",
            configuration: r#"{"origin":"https://control.example.com","issuer":"https://identity.example.com","client_id":"orders-control","identities":{},"certificate_authority":"absent-orders-ca.pem"}"#,
            arguments: &[],
            expected_error: "could not read OIDC certificate authority",
        },
        AuthenticationConfigurationTestCase {
            description: "OIDC conflicts with password authentication",
            option: "--oidc-config",
            configuration: r#"{"origin":"https://control.example.com","issuer":"https://identity.example.com","client_id":"orders-control","identities":{}}"#,
            arguments: &["--password-origin", "https://control.example.com"],
            expected_error: "OIDC authentication cannot be combined",
        },
    ];
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    for test_case in test_cases {
        runtime.block_on(async {
            let directory = tempfile::tempdir().expect("invalid identity installation");
            std::fs::write(
                directory.path().join("identity.json"),
                test_case.configuration,
            )
            .expect("configuration input");
            let mut arguments = vec![
                "server",
                "--port",
                "0",
                "--no-scheduler",
                test_case.option,
                "identity.json",
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
            assert!(
                !log.contains("Control API ready:"),
                "failed startup must not announce readiness"
            );
        });
    }
}
