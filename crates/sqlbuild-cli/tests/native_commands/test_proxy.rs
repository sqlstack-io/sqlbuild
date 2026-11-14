//! A standalone server reads `server.toml`, authenticates each request from explicit proxy bindings and rejects ambiguous modes.

use reqwest::{Client, StatusCode, header};
use serde_json::{Value, json};

use crate::helpers::{api_ready, start_control_in, wait_for_exit};
use crate::test_types::{ProxyCliTestCase, ServerConfigurationTestCase};

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
            let configuration = format!(
                "[server]\nhost = \"203.0.113.9\"\nscheduler = true\n\n[trusted_proxy]\norigin = \"https://control.example.com\"\ntrusted_peers = [\"127.0.0.1\"]\n\n[trusted_proxy.identities]\n{} = \"{principal}\"\n",
                json!(test_case.subject)
            );
            std::fs::write(directory.path().join("server.toml"), configuration).expect("server-owned identity bindings");
            let mut control = start_control_in(directory, &["server", "--host", "127.0.0.1", "--port", "0", "--no-scheduler", "--config", "server.toml"]);
            let url = api_ready(&mut control).await;
            let log = std::fs::read_to_string(&control.stderr).expect("startup log");
            assert!(!log.contains("Starting scheduler"), "{}: --no-scheduler overrides [server] scheduler", test_case.description);
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
fn given_invalid_or_conflicting_server_configuration_when_a_standalone_server_starts_then_it_fails_closed()
 {
    let proxy = "[trusted_proxy]\norigin = \"https://control.example.com\"\ntrusted_peers = [\"127.0.0.1\"]\nidentities = {}\n";
    let oidc = "[oidc]\norigin = \"https://control.example.com\"\nissuer = \"https://identity.example.com\"\nclient_id = \"orders-control\"\nidentities = {}\n";
    let test_cases = [
        ServerConfigurationTestCase {
            description: "malformed TOML",
            configuration: "[trusted_proxy",
            arguments: &[],
            expected_error: "server.toml is not a valid server configuration at line 1",
        },
        ServerConfigurationTestCase {
            description: "unknown section",
            configuration: "[orders]\nsynthetic = true\n",
            arguments: &[],
            expected_error: "server.toml is not a valid server configuration at line 1",
        },
        ServerConfigurationTestCase {
            description: "unknown trusted-proxy field",
            configuration: "[trusted_proxy]\norigin = \"https://control.example.com\"\ntrusted_peers = [\"127.0.0.1\"]\nidentities = {}\nunknown = \"synthetic-unrecognized-value\"\n",
            arguments: &[],
            expected_error: "server.toml is not a valid server configuration at line 5: unknown field `unknown`",
        },
        ServerConfigurationTestCase {
            description: "no trusted peer boundary",
            configuration: "[trusted_proxy]\norigin = \"https://control.example.com\"\ntrusted_peers = []\nidentities = {}\n",
            arguments: &[],
            expected_error: "requires explicit trusted peer IP addresses",
        },
        ServerConfigurationTestCase {
            description: "non-HTTPS external origin",
            configuration: "[trusted_proxy]\norigin = \"http://control.example.com\"\ntrusted_peers = [\"127.0.0.1\"]\nidentities = {}\n",
            arguments: &[],
            expected_error: "trusted-proxy authentication requires an HTTPS origin",
        },
        ServerConfigurationTestCase {
            description: "subject cannot be represented by the header transport",
            configuration: "[trusted_proxy]\norigin = \"https://control.example.com\"\ntrusted_peers = [\"127.0.0.1\"]\n\n[trusted_proxy.identities]\n\"external-café\" = \"00000000-0000-0000-0000-000000000001\"\n",
            arguments: &[],
            expected_error: "subjects must be nonempty, unpadded ASCII header values",
        },
        ServerConfigurationTestCase {
            description: "a password-origin flag conflicts with the file's trusted proxy",
            configuration: proxy,
            arguments: &["--password-origin", "https://control.example.com"],
            expected_error: "trusted-proxy authentication cannot be combined",
        },
        ServerConfigurationTestCase {
            description: "OIDC rejects plaintext issuer",
            configuration: "[oidc]\norigin = \"https://control.example.com\"\nissuer = \"http://identity.example.com\"\nclient_id = \"orders-control\"\nidentities = {}\n",
            arguments: &[],
            expected_error: "OIDC endpoints require HTTPS URLs without credentials",
        },
        ServerConfigurationTestCase {
            description: "OIDC rejects plaintext browser origin",
            configuration: "[oidc]\norigin = \"http://control.example.com\"\nissuer = \"https://identity.example.com\"\nclient_id = \"orders-control\"\nidentities = {}\n",
            arguments: &[],
            expected_error: "OIDC authentication requires an HTTPS origin",
        },
        ServerConfigurationTestCase {
            description: "OIDC rejects missing client secret environment variable",
            configuration: "[oidc]\norigin = \"https://control.example.com\"\nissuer = \"https://identity.example.com\"\nclient_id = \"orders-control\"\nidentities = {}\nclient_secret_env = \"SQLBUILD_SYNTHETIC_OIDC_MISSING\"\n",
            arguments: &[],
            expected_error: "OIDC client-secret environment variable is unavailable",
        },
        ServerConfigurationTestCase {
            description: "OIDC rejects missing certificate authority",
            configuration: "[oidc]\norigin = \"https://control.example.com\"\nissuer = \"https://identity.example.com\"\nclient_id = \"orders-control\"\nidentities = {}\ncertificate_authority = \"absent-orders-ca.pem\"\n",
            arguments: &[],
            expected_error: "could not read OIDC certificate authority",
        },
        ServerConfigurationTestCase {
            description: "the file's OIDC conflicts with password authentication",
            configuration: oidc,
            arguments: &["--password-origin", "https://control.example.com"],
            expected_error: "OIDC authentication cannot be combined",
        },
        ServerConfigurationTestCase {
            description: "a zero retention in the file names its key",
            configuration: "[retention]\nstatistics_days = 0\n",
            arguments: &[],
            expected_error: "[retention] statistics_days must be between 1 and 65535",
        },
        ServerConfigurationTestCase {
            description: "the replaced trusted-proxy flag points to the file",
            configuration: "",
            arguments: &["--trusted-proxy-config", "proxy.json"],
            expected_error: "--trusted-proxy-config was replaced by --config: put its settings in the [trusted_proxy] section",
        },
        ServerConfigurationTestCase {
            description: "the replaced OIDC flag points to the file",
            configuration: "",
            arguments: &["--oidc-config", "oidc.json"],
            expected_error: "--oidc-config was replaced by --config: put its settings in the [oidc] section",
        },
        ServerConfigurationTestCase {
            description: "the replaced launchers flag points to the file",
            configuration: "",
            arguments: &["--launchers", "launchers.json"],
            expected_error: "--launchers was replaced by --config: put its settings in the [[launchers.docker]] or [[launchers.local]] section",
        },
        ServerConfigurationTestCase {
            description: "a launcher names a location the store does not have",
            configuration: "[[launchers.docker]]\nlocation = \"orders\"\ntarget = \"development\"\nserver_url = \"http://127.0.0.1:1\"\nmax_workers = 1\nsocket = \"/var/run/docker.sock\"\nlocation_container = \"orders-location\"\nexecutable = \"sqb\"\n",
            arguments: &[],
            expected_error: "launcher location 'orders' does not exist; create it with `sqb admin create-location orders`",
        },
    ];
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    for test_case in test_cases {
        runtime.block_on(async {
            let directory = tempfile::tempdir().expect("invalid server installation");
            std::fs::write(
                directory.path().join("server.toml"),
                test_case.configuration,
            )
            .expect("configuration input");
            let mut arguments = vec!["server", "--port", "0", "--config", "server.toml"];
            arguments.extend_from_slice(test_case.arguments);
            let mut control = start_control_in(directory, &arguments);
            let status = wait_for_exit(&mut control).await;
            assert!(!status.success(), "{}", test_case.description);
            let log = std::fs::read_to_string(&control.stderr).expect("startup failure");
            assert!(
                log.contains(test_case.expected_error),
                "{}: {log}",
                test_case.description
            );
            assert!(
                test_case.configuration.is_empty() || !log.contains(test_case.configuration),
                "configuration contents must not be echoed"
            );
            assert!(
                !log.contains("Control API ready:"),
                "failed startup must not announce readiness"
            );
        });
    }
}
