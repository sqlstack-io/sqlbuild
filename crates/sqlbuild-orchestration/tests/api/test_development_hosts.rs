//! A development server takes only local and opted-in names, and leaves loopback only when asked.

use reqwest::{StatusCode, header};
use uuid::Uuid;

use crate::api::main::serve_api::serve_api;
use crate::api::models::{ApiAuthentication, DevelopmentScope, HumanLogin, ServerFacts};
use crate::credentials::main::identities::development_person::development_person;
use crate::credentials::models::AuditContext;
use crate::runs::tests::test_types::ExactKeyPlanner;
use crate::store::main::migrate_store::migrate_store;

use crate::api::tests::helpers::{start_api_on, stop_api};
use crate::api::tests::test_types::{DevelopmentHostTestCase, DevelopmentListenerTestCase};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::tls::main::client_builder::client_builder;

#[test]
fn given_a_development_server_when_a_request_names_a_host_and_origin_then_only_local_and_allowed_names_pass()
 {
    let test_cases = [
        DevelopmentHostTestCase {
            description: "no opt-in takes loopback",
            bind: "127.0.0.1:0",
            allowed: &[],
            host: "127.0.0.1:{port}",
            origin: Some("http://127.0.0.1:{port}"),
            expected_status: StatusCode::OK,
        },
        DevelopmentHostTestCase {
            description: "no opt-in takes localhost",
            bind: "127.0.0.1:0",
            allowed: &[],
            host: "localhost:{port}",
            origin: None,
            expected_status: StatusCode::OK,
        },
        DevelopmentHostTestCase {
            description: "no opt-in refuses a public Host",
            bind: "127.0.0.1:0",
            allowed: &[],
            host: "control.example.com",
            origin: None,
            expected_status: StatusCode::FORBIDDEN,
        },
        DevelopmentHostTestCase {
            description: "no opt-in refuses a public Origin",
            bind: "127.0.0.1:0",
            allowed: &[],
            host: "127.0.0.1:{port}",
            origin: Some("https://control.example.com"),
            expected_status: StatusCode::FORBIDDEN,
        },
        DevelopmentHostTestCase {
            description: "an allowed name through an HTTPS proxy",
            bind: "127.0.0.1:0",
            allowed: &["control.example.com"],
            host: "control.example.com",
            origin: Some("https://control.example.com"),
            expected_status: StatusCode::OK,
        },
        DevelopmentHostTestCase {
            description: "an allowed name through an HTTP proxy",
            bind: "127.0.0.1:0",
            allowed: &["control.example.com"],
            host: "control.example.com",
            origin: Some("http://control.example.com"),
            expected_status: StatusCode::OK,
        },
        DevelopmentHostTestCase {
            description: "a proxy that writes the default port",
            bind: "127.0.0.1:0",
            allowed: &["control.example.com"],
            host: "control.example.com:443",
            origin: Some("https://control.example.com"),
            expected_status: StatusCode::OK,
        },
        DevelopmentHostTestCase {
            description: "a proxy that rewrites Host only",
            bind: "127.0.0.1:0",
            allowed: &["control.example.com"],
            host: "127.0.0.1:{port}",
            origin: Some("https://control.example.com"),
            expected_status: StatusCode::OK,
        },
        DevelopmentHostTestCase {
            description: "an allowed name on the listener port",
            bind: "127.0.0.1:0",
            allowed: &["control.example.com"],
            host: "control.example.com:{port}",
            origin: Some("http://control.example.com:{port}"),
            expected_status: StatusCode::OK,
        },
        DevelopmentHostTestCase {
            description: "names match in any case",
            bind: "127.0.0.1:0",
            allowed: &["control.example.com"],
            host: "Control.Example.COM",
            origin: None,
            expected_status: StatusCode::OK,
        },
        DevelopmentHostTestCase {
            description: "a rebound name stays refused",
            bind: "127.0.0.1:0",
            allowed: &["control.example.com"],
            host: "rebound.example.net:{port}",
            origin: None,
            expected_status: StatusCode::FORBIDDEN,
        },
        DevelopmentHostTestCase {
            description: "another site stays refused",
            bind: "127.0.0.1:0",
            allowed: &["control.example.com"],
            host: "control.example.com",
            origin: Some("https://other.example.net"),
            expected_status: StatusCode::FORBIDDEN,
        },
        DevelopmentHostTestCase {
            description: "a name that extends an allowed one stays refused",
            bind: "127.0.0.1:0",
            allowed: &["control.example.com"],
            host: "control.example.com.example.net",
            origin: None,
            expected_status: StatusCode::FORBIDDEN,
        },
        DevelopmentHostTestCase {
            description: "another port of an allowed name stays refused",
            bind: "127.0.0.1:0",
            allowed: &["control.example.com"],
            host: "control.example.com",
            origin: Some("http://control.example.com:9000"),
            expected_status: StatusCode::FORBIDDEN,
        },
        DevelopmentHostTestCase {
            description: "a null Origin stays refused",
            bind: "127.0.0.1:0",
            allowed: &["control.example.com"],
            host: "control.example.com",
            origin: Some("null"),
            expected_status: StatusCode::FORBIDDEN,
        },
        DevelopmentHostTestCase {
            description: "a named port",
            bind: "127.0.0.1:0",
            allowed: &["control.example.com:8443"],
            host: "control.example.com:8443",
            origin: Some("https://control.example.com:8443"),
            expected_status: StatusCode::OK,
        },
        DevelopmentHostTestCase {
            description: "a named port is the only one",
            bind: "127.0.0.1:0",
            allowed: &["control.example.com:8443"],
            host: "control.example.com",
            origin: None,
            expected_status: StatusCode::FORBIDDEN,
        },
        DevelopmentHostTestCase {
            description: "a named HTTPS port is left out by browsers",
            bind: "127.0.0.1:0",
            allowed: &["control.example.com:443"],
            host: "control.example.com",
            origin: Some("https://control.example.com"),
            expected_status: StatusCode::OK,
        },
        DevelopmentHostTestCase {
            description: "a named HTTPS port is not the HTTP one",
            bind: "127.0.0.1:0",
            allowed: &["control.example.com:443"],
            host: "control.example.com",
            origin: Some("http://control.example.com"),
            expected_status: StatusCode::FORBIDDEN,
        },
        DevelopmentHostTestCase {
            description: "an IPv6 name",
            bind: "127.0.0.1:0",
            allowed: &["[fd00::5]"],
            host: "[fd00::5]:{port}",
            origin: Some("http://[fd00::5]:{port}"),
            expected_status: StatusCode::OK,
        },
        DevelopmentHostTestCase {
            description: "an open listener takes loopback",
            bind: "0.0.0.0:0",
            allowed: &[],
            host: "127.0.0.1:{port}",
            origin: Some("http://127.0.0.1:{port}"),
            expected_status: StatusCode::OK,
        },
        DevelopmentHostTestCase {
            description: "an open listener refuses its wildcard as a name",
            bind: "0.0.0.0:0",
            allowed: &[],
            host: "0.0.0.0:{port}",
            origin: None,
            expected_status: StatusCode::FORBIDDEN,
        },
        DevelopmentHostTestCase {
            description: "an open listener refuses an unlisted name",
            bind: "0.0.0.0:0",
            allowed: &[],
            host: "control.example.com:{port}",
            origin: None,
            expected_status: StatusCode::FORBIDDEN,
        },
        DevelopmentHostTestCase {
            description: "an open listener takes an allowed address",
            bind: "0.0.0.0:0",
            allowed: &["192.0.2.10"],
            host: "192.0.2.10:{port}",
            origin: Some("http://192.0.2.10:{port}"),
            expected_status: StatusCode::OK,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(Backend::Sqlite).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let context = AuditContext {
                actor: "local-development".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let scope = DevelopmentScope {
                location_id: Uuid::now_v7(),
                target: "development".into(),
                triggers: false,
                principal_id: development_person(store, &context).await.expect("person"),
                allowed_hosts: test_case.allowed.iter().map(ToString::to_string).collect(),
                listens_beyond_loopback: true,
            };
            let authentication = ApiAuthentication {
                login: HumanLogin::Development(scope),
                ..ApiAuthentication::default()
            };
            let api = start_api_on(
                store,
                test_case.bind,
                (authentication, ServerFacts::default()),
            )
            .await;
            let (_, port) = api.url.rsplit_once(':').expect("port");
            let headers: header::HeaderMap = [
                (header::HOST, Some(test_case.host)),
                (header::ORIGIN, test_case.origin),
            ]
            .into_iter()
            .filter_map(|(name, value)| {
                value.map(|value| (name, value.replace("{port}", port).parse().expect("header")))
            })
            .collect();
            let response = client_builder()
                .build()
                .expect("HTTP client")
                .get(format!("{}/api/control/bootstrap", api.url))
                .headers(headers)
                .send()
                .await
                .expect("request");
            assert_eq!(
                response.status(),
                test_case.expected_status,
                "{}",
                test_case.description
            );
            drop(response);
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_no_opt_in_when_a_development_server_would_listen_beyond_loopback_then_it_refuses_to_start()
{
    let test_cases = [DevelopmentListenerTestCase {
        description: "every interface without the opt-in",
        bind: "0.0.0.0:0",
        expected_refusal: "disabled authentication requires a loopback listener",
    }];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(Backend::Sqlite).await else {
                return;
            };
            migrate_store(&fixture.store).await.expect("migrate");
            let listener = tokio::net::TcpListener::bind(test_case.bind)
                .await
                .expect("listener");
            let refused = serve_api(
                fixture.store.clone(),
                listener,
                (
                    ApiAuthentication {
                        login: HumanLogin::Development(DevelopmentScope {
                            location_id: Uuid::now_v7(),
                            target: "development".into(),
                            triggers: false,
                            principal_id: Uuid::now_v7(),
                            allowed_hosts: Vec::new(),
                            listens_beyond_loopback: false,
                        }),
                        ..ApiAuthentication::default()
                    },
                    (Vec::new(), std::sync::Arc::new(ExactKeyPlanner)),
                    ServerFacts::default(),
                ),
                (|| {}, async {}),
            )
            .await
            .expect_err("no opt-in");
            assert!(
                refused.to_string().contains(test_case.expected_refusal),
                "{}: {refused}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
