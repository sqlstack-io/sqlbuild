//! A standalone server reads `server.toml`, authenticates each request from explicit proxy bindings and rejects ambiguous modes.

use reqwest::{Client, StatusCode, header};
use serde_json::{Value, json};

#[cfg(unix)]
use crate::admin_helpers::administer;
use crate::helpers::{api_ready, start_control_in, wait_for_exit};
#[cfg(unix)]
use crate::test_types::NetworkKeyTestCase;
use crate::test_types::{
    ProxyCliTestCase, ProxyRemotePeerTestCase, ProxyStartupTestCase, ServerConfigurationTestCase,
};

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
                .args(["admin", "create-principal", "orders-reader", "--json"])
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
            description: "an allowed host written as a URL",
            configuration: "[trusted_proxy]\norigin = \"https://control.example.com\"\nallowed_hosts = [\"https://control-short\"]\n",
            arguments: &[],
            expected_error: "[trusted_proxy] allowed_hosts lists host names without a scheme, port or path; 'https://control-short' is not one",
        },
        ServerConfigurationTestCase {
            description: "neither an HTTP nor an HTTPS external origin",
            configuration: "[trusted_proxy]\norigin = \"ftp://control.example.com\"\ntrusted_peers = [\"127.0.0.1\"]\n",
            arguments: &[],
            expected_error: "trusted-proxy authentication requires an HTTP or HTTPS origin",
        },
        ServerConfigurationTestCase {
            description: "a username header a browser sets",
            configuration: "[trusted_proxy]\norigin = \"https://control.example.com\"\ntrusted_peers = [\"127.0.0.1\"]\nusername_header = \"Cookie\"\n",
            arguments: &[],
            expected_error: "[trusted_proxy] username_header cannot be cookie",
        },
        ServerConfigurationTestCase {
            description: "a fetch-metadata header a browser sets",
            configuration: "[trusted_proxy]\norigin = \"https://control.example.com\"\nusername_header = \"Sec-Fetch-User\"\n",
            arguments: &[],
            expected_error: "[trusted_proxy] username_header cannot be sec-fetch-user",
        },
        ServerConfigurationTestCase {
            description: "the display name in the username header",
            configuration: "[trusted_proxy]\norigin = \"https://control.example.com\"\ntrusted_peers = [\"127.0.0.1\"]\nusername_header = \"X-Example-User\"\ndisplay_name_header = \"x-example-user\"\n",
            arguments: &[],
            expected_error: "username_header, display_name_header and email_header must differ",
        },
        ServerConfigurationTestCase {
            description: "a script as the sign-out URL",
            configuration: "[trusted_proxy]\norigin = \"https://control.example.com\"\ntrusted_peers = [\"127.0.0.1\"]\nproxy_logout_url = \"javascript:alert(1)\"\n",
            arguments: &[],
            expected_error: "proxy_logout_url must be an http:// or https:// URL",
        },
        ServerConfigurationTestCase {
            description: "an unknown policy for unknown users",
            configuration: "[trusted_proxy]\norigin = \"https://control.example.com\"\ntrusted_peers = [\"127.0.0.1\"]\nunknown_users = \"provision_admin\"\n",
            arguments: &[],
            expected_error: "unknown variant `provision_admin`",
        },
        ServerConfigurationTestCase {
            description: "two bindings for one username",
            configuration: "[trusted_proxy]\norigin = \"https://control.example.com\"\ntrusted_peers = [\"127.0.0.1\"]\n\n[trusted_proxy.identities]\n\"Orders-Reader\" = \"00000000-0000-0000-0000-000000000001\"\n\"orders-reader\" = \"00000000-0000-0000-0000-000000000002\"\n",
            arguments: &[],
            expected_error: "binds 'orders-reader' more than once",
        },
        ServerConfigurationTestCase {
            description: "subject cannot be represented by the header transport",
            configuration: "[trusted_proxy]\norigin = \"https://control.example.com\"\ntrusted_peers = [\"127.0.0.1\"]\n\n[trusted_proxy.identities]\n\"external-café\" = \"00000000-0000-0000-0000-000000000001\"\n",
            arguments: &[],
            expected_error: "trusted-proxy usernames must be 1-128 characters",
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
            description: "an attention snooze that isn't a duration names its key",
            configuration: "[server]\nattention_snooze = \"a day\"\n",
            arguments: &[],
            expected_error: "[server] attention_snooze must be a duration such as \"24h\" or \"1d12h\", or \"0\" for no time limit",
        },
        ServerConfigurationTestCase {
            description: "an attention snooze in months is refused",
            configuration: "[server]\nattention_snooze = \"1mo\"\n",
            arguments: &[],
            expected_error: "[server] attention_snooze can't use years or months",
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

#[test]
fn given_a_misconfigured_proxy_when_requests_arrive_then_startup_warns_and_each_refusal_is_logged_once()
 {
    let test_cases = [
        ProxyStartupTestCase {
            description: "the proxy's real address differs from trusted_peers",
            configuration: "[trusted_proxy]\norigin = \"http://control.example.com\"\ntrusted_peers = [\"192.0.2.1\"]\nusername_header = \"X-Example-User\"\n",
            username: "orders-confidential-reader",
            host: "127.0.0.1",
            expected_status: StatusCode::UNAUTHORIZED,
            expected_warnings: &[
                (
                    "warning: the trusted-proxy origin http://control.example.com is plain HTTP",
                    1,
                ),
                (
                    "warning: ignored the trusted-proxy username header x-example-user from 127.0.0.1, which is not in [trusted_proxy] trusted_peers",
                    1,
                ),
            ],
        },
        ProxyStartupTestCase {
            description: "a trusted peer's Host names neither this server nor allowed_hosts",
            configuration: "[trusted_proxy]\norigin = \"https://control.example.com\"\nusername_header = \"X-Example-User\"\n",
            username: "orders-rebound-reader",
            host: "evil.example:8000",
            expected_status: StatusCode::FORBIDDEN,
            expected_warnings: &[(
                "warning: refused a request from trusted proxy peer 127.0.0.1 with Host 'evil.example:8000', which is not a loopback name, the [trusted_proxy] origin's host or in [trusted_proxy] allowed_hosts; if your proxy keeps the browser's host name (ProxyPreserveHost On) or forwards to this server by another name or address, add it to allowed_hosts",
                1,
            )],
        },
    ];
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    for test_case in test_cases {
        runtime.block_on(async {
            let directory = tempfile::tempdir().expect("proxy installation");
            std::fs::write(
                directory.path().join("server.toml"),
                test_case.configuration,
            )
            .expect("server configuration");
            let mut control = start_control_in(
                directory,
                &[
                    "server",
                    "--host",
                    "127.0.0.1",
                    "--port",
                    "0",
                    "--no-scheduler",
                    "--config",
                    "server.toml",
                ],
            );
            let url = api_ready(&mut control).await;
            let client = Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("HTTP client");
            let mut statuses = Vec::new();
            for _ in 0..2 {
                let response = client
                    .get(format!("{url}/api/control/bootstrap"))
                    .header(header::HOST, test_case.host)
                    .header("x-forwarded-host", "control.example.com")
                    .header("x-example-user", test_case.username)
                    .send()
                    .await
                    .expect("refused proxy request");
                statuses.push(response.status());
            }
            let log = std::fs::read_to_string(&control.stderr).expect("server log");
            let counts: Vec<(&str, usize)> = test_case
                .expected_warnings
                .iter()
                .map(|(warning, _)| (*warning, log.matches(warning).count()))
                .collect();
            assert_eq!(
                (
                    statuses,
                    counts.as_slice(),
                    log.contains(test_case.username)
                ),
                (
                    vec![test_case.expected_status; 2],
                    test_case.expected_warnings,
                    false
                ),
                "{}: {log}",
                test_case.description
            );
            #[cfg(unix)]
            crate::helpers::terminate(&mut control).await;
        });
    }
}

#[test]
fn given_the_default_trusted_peers_when_a_non_loopback_peer_sends_the_username_header_then_it_is_ignored_and_logged_once()
 {
    let test_cases = [ProxyRemotePeerTestCase {
        description: "a request from another address of this machine is not from loopback",
        configuration: "[trusted_proxy]\norigin = \"https://control.example.com\"\n",
        username: "orders-remote-reader",
        expected_status: StatusCode::UNAUTHORIZED,
        expected_warnings: 1,
    }];
    // The address this machine routes from; connecting a UDP socket sends nothing.
    let route = std::net::UdpSocket::bind("0.0.0.0:0").expect("UDP socket");
    route.connect("192.0.2.1:9").expect("a non-loopback route");
    let host = route.local_addr().expect("route address").ip().to_string();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    for test_case in test_cases {
        runtime.block_on(async {
            let directory = tempfile::tempdir().expect("proxy installation");
            std::fs::write(
                directory.path().join("server.toml"),
                test_case.configuration,
            )
            .expect("server configuration");
            let mut control = start_control_in(
                directory,
                &[
                    "server",
                    "--host",
                    &host,
                    "--port",
                    "0",
                    "--no-scheduler",
                    "--config",
                    "server.toml",
                ],
            );
            let url = api_ready(&mut control).await;
            let client = Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("HTTP client");
            let mut statuses = Vec::new();
            for _ in 0..2 {
                let response = client
                    .get(format!("{url}/api/control/bootstrap"))
                    .header("x-forwarded-host", "control.example.com")
                    .header("x-forwarded-for", "127.0.0.1")
                    .header("x-sqb-user", test_case.username)
                    .send()
                    .await
                    .expect("remote request");
                statuses.push(response.status());
            }
            let log = std::fs::read_to_string(&control.stderr).expect("server log");
            let warning = format!(
                "warning: ignored the trusted-proxy username header x-sqb-user from {host}, which is not in [trusted_proxy] trusted_peers"
            );
            assert_eq!(
                (
                    statuses,
                    log.matches(&warning).count(),
                    log.contains(test_case.username)
                ),
                (
                    vec![test_case.expected_status; 2],
                    test_case.expected_warnings,
                    false
                ),
                "{}: {log}",
                test_case.description
            );
            #[cfg(unix)]
            crate::helpers::terminate(&mut control).await;
        });
    }
}

#[cfg(unix)]
#[test]
fn given_a_plain_http_proxy_server_open_to_the_network_when_an_untrusted_peer_calls_then_only_its_key_gets_in()
 {
    let test_cases = [NetworkKeyTestCase {
        description: "an http:// origin, a foreign Host and a peer outside trusted_peers",
        configuration: "[server]\nhost = \"0.0.0.0\"\n\n[trusted_proxy]\norigin = \"http://control.example.com\"\ntrusted_peers = [\"192.0.2.10\"]\n",
        host: "scripts.example.net:8000",
        expected_statuses: [
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::UNAUTHORIZED,
            StatusCode::UNAUTHORIZED,
        ],
        expected_name: "orders-deploy",
    }];
    let route = std::net::UdpSocket::bind("0.0.0.0:0").expect("UDP socket");
    route.connect("192.0.2.1:9").expect("a non-loopback route");
    let peer = route.local_addr().expect("route address").ip();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    for test_case in test_cases {
        runtime.block_on(async {
            let directory = tempfile::tempdir().expect("proxy installation");
            std::fs::write(
                directory.path().join("server.toml"),
                test_case.configuration,
            )
            .expect("server configuration");
            let mut control = start_control_in(
                directory,
                &[
                    "server",
                    "--port",
                    "0",
                    "--no-scheduler",
                    "--config",
                    "server.toml",
                ],
            );
            let ready = api_ready(&mut control).await;
            let (_, port) = ready.rsplit_once(':').expect("listening port");
            let url = format!("http://{peer}:{port}");
            let created = administer(
                &control,
                &["create-service-account", "orders-deploy", "--role", "admin"],
            );
            let key = created["key"].as_str().expect("printed key");
            let client = Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("HTTP client");
            let bootstrap = format!("{url}/api/control/bootstrap");
            let read = client
                .get(&bootstrap)
                .header(header::HOST, test_case.host)
                .bearer_auth(key)
                .send()
                .await
                .expect("keyed read");
            let read_status = read.status();
            let body: Value = read.json().await.unwrap_or(Value::Null);
            let write = client
                .post(format!("{url}/api/admin/service-accounts"))
                .header(header::HOST, test_case.host)
                .bearer_auth(key)
                .json(&json!({"name": "report-export", "role": "viewer"}))
                .send()
                .await
                .expect("keyed write");
            let named = |host: &'static str| {
                client
                    .get(&bootstrap)
                    .header(header::HOST, host)
                    .header("x-sqb-user", "orders-reader")
                    .send()
            };
            let statuses = [
                read_status,
                write.status(),
                named(test_case.host)
                    .await
                    .expect("username under the foreign Host")
                    .status(),
                named("control.example.com")
                    .await
                    .expect("username under the origin's Host")
                    .status(),
            ];
            assert_eq!(
                (statuses, body["access"]["name"].as_str()),
                (test_case.expected_statuses, Some(test_case.expected_name)),
                "{}: peer {peer}",
                test_case.description
            );
            crate::helpers::terminate(&mut control).await;
        });
    }
}
