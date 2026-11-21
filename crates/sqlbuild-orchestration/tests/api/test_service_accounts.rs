//! Service accounts call with a key on any address; only its hash is kept, and it acts under its own name.

use std::collections::{BTreeMap, BTreeSet};

use reqwest::header::{COOKIE, HOST};
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};

use crate::api::models::{
    ApiAuthentication, HumanLogin, TrustedProxyAuthentication, UnknownProxyUsers,
};
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::password_people_helpers::{
    ORIGIN_URL, browser, password_administrator, password_person, session,
};
use crate::api::tests::service_accounts_helpers::{
    NETWORK_HOST, end_overlaps, keyed_bootstrap, owner, rows, script, service_account,
};
use crate::api::tests::test_types::{
    ImportedKeyTestCase, KeyOnlyTestCase, ProxyKeyTestCase, ServiceAccountLifecycleTestCase,
    ServiceAccountPermissionTestCase,
};
use crate::authorization::_helpers::administrator_roles::assign_role_in;
use crate::authorization::main::service_accounts::create_service_account::create_service_account;
use crate::credentials::_helpers::service_keys::key_hash;
use crate::credentials::main::identities::create_principal::create_principal;
use crate::credentials::models::{
    BuiltinRole, NewServiceAccount, RoleAssignmentRequest, RoleKey, ServiceAccountAuthority,
};
use crate::store::main::{begin::begin, migrate_store::migrate_store};
use crate::store::models::TransactionMode;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

/// A signed-in person's write: their session cookie and its CSRF token.
async fn post(
    client: &Client,
    (url, path): (&str, &str),
    person: &(String, String),
    body: &Value,
) -> (StatusCode, Value) {
    let response = client
        .post(format!("{url}{path}"))
        .header(COOKIE, &person.0)
        .header("x-sqb-csrf", &person.1)
        .json(body)
        .send()
        .await
        .expect("write");
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

async fn get(
    client: &Client,
    (url, path): (&str, &str),
    person: &(String, String),
) -> (StatusCode, Value) {
    let response = client
        .get(format!("{url}{path}"))
        .header(COOKIE, &person.0)
        .send()
        .await
        .expect("read");
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

#[test]
fn given_a_service_account_when_its_key_is_used_rotated_and_disabled_then_only_working_keys_authenticate()
 {
    let test_cases = [
        ServiceAccountLifecycleTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
        },
        ServiceAccountLifecycleTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            let what = test_case.description;
            migrate_store(store).await.expect("migrate");
            password_administrator(store, "ada").await;
            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    login: HumanLogin::Password(ORIGIN_URL.into()),
                    ..ApiAuthentication::default()
                },
            )
            .await;
            let (client, keyed) = (browser(), script());
            let url = api.url.as_str();
            let ada = session(&client, url, "ada").await;

            // Created over HTTP: the key comes back once, the store keeps only its hash.
            let (status, issued) = post(
                &client,
                (url, "/api/admin/service-accounts"),
                &ada,
                &json!({"name": "orders-deploy", "description": "Release pipeline", "role": "developer"}),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{what}: create {issued}");
            let key = issued["key"].as_str().expect("key").to_owned();
            let id = issued["account"]["principal_id"]
                .as_str()
                .expect("id")
                .to_owned();
            assert!(
                key.starts_with("sqbk_") && key.len() == 45,
                "{what}: key shape"
            );
            assert_eq!(issued["account"]["keys"][0]["prefix"], json!(key[..11]));
            assert_eq!(issued["account"]["role"], json!("developer"));
            let (_, listed) = get(&client, (url, "/api/admin/service-accounts"), &ada).await;
            assert_eq!(listed[0]["name"], json!("orders-deploy"));
            assert!(
                !listed.to_string().contains(&key),
                "{what}: listing never shows the key"
            );
            let stored = rows(store, "sqb_service_account_keys").await;
            assert_eq!(
                stored[0]["secret_hash"],
                json!(key_hash(&key)),
                "{what}: hash stored"
            );
            for table in [
                "sqb_service_account_keys",
                "sqb_service_accounts",
                "sqb_principals",
                "sqb_security_audit",
            ] {
                let text = serde_json::to_string(&rows(store, table).await).expect("rows");
                assert!(!text.contains(&key[5..]), "{what}: {table} keeps no key");
            }
            let (_, people) = get(&client, (url, "/api/admin/people"), &ada).await;
            assert!(
                !people.to_string().contains("orders-deploy"),
                "{what}: not a person"
            );

            // Used from a network address the browser origin doesn't name.
            assert_eq!(
                keyed_bootstrap(&keyed, url, &key).await,
                (StatusCode::OK, "orders-deploy".to_owned()),
                "{what}: keyed request from any address"
            );
            let session_read = keyed
                .get(format!("{url}/api/auth/session"))
                .header(HOST, NETWORK_HOST)
                .bearer_auth(&key)
                .send()
                .await
                .expect("session read");
            assert_eq!(
                session_read.status(),
                StatusCode::UNAUTHORIZED,
                "{what}: no sessions"
            );
            let foreign_login = keyed
                .post(format!("{url}/api/auth/password/login"))
                .header(HOST, NETWORK_HOST)
                .bearer_auth(&key)
                .json(&json!({"username": "ada", "password": "a long enough password"}))
                .send()
                .await
                .expect("login under a foreign host");
            assert_eq!(
                foreign_login.status(),
                StatusCode::UNAUTHORIZED,
                "{what}: a key doesn't lift sign-in's host check"
            );

            // Its actions are audited under its name: an allowed change and a refusal.
            let ops = service_account(store, "ops-bot", BuiltinRole::Admin).await;
            let added = keyed
                .post(format!("{url}/api/admin/service-accounts"))
                .header(HOST, NETWORK_HOST)
                .bearer_auth(&ops.key)
                .json(&json!({"name": "report-export", "role": "viewer"}))
                .send()
                .await
                .expect("keyed create");
            assert_eq!(added.status(), StatusCode::OK, "{what}: keyed admin call");
            let refused = keyed
                .get(format!("{url}/api/admin/service-accounts"))
                .header(HOST, NETWORK_HOST)
                .bearer_auth(&key)
                .send()
                .await
                .expect("keyed refusal");
            assert_eq!(
                refused.status(),
                StatusCode::FORBIDDEN,
                "{what}: developer can't list"
            );
            let (_, audit) = get(&client, (url, "/api/control/audit?limit=100"), &ada).await;
            let by = |name: &str, action: &str| {
                audit.as_array().expect("audit").iter().any(|event| {
                    event["actor_name"] == json!(name)
                        && event["actor_kind"] == json!("service")
                        && event["action"] == json!(action)
                })
            };
            assert!(
                by("ops-bot", "service_account.created"),
                "{what}: allowed action by name"
            );
            assert!(
                by("orders-deploy", "authorization.denied"),
                "{what}: refusal by name"
            );

            // Rotation with an overlap: both keys work until the overlap ends.
            let path = format!("/api/admin/service-accounts/{id}/keys");
            let (status, rotated) =
                post(&client, (url, &path), &ada, &json!({"overlap_minutes": 60})).await;
            assert_eq!(status, StatusCode::OK, "{what}: rotate {rotated}");
            let second = rotated["key"].as_str().expect("new key").to_owned();
            assert_eq!(rotated["account"]["keys"].as_array().map(Vec::len), Some(2));
            assert!(rotated["account"]["keys"][1]["expires_at"].is_string());
            assert_eq!(
                keyed_bootstrap(&keyed, url, &key).await.0,
                StatusCode::OK,
                "{what}: old key in overlap"
            );
            assert_eq!(
                keyed_bootstrap(&keyed, url, &second).await.0,
                StatusCode::OK,
                "{what}: new key"
            );
            let principal = uuid::Uuid::parse_str(&id).expect("uuid");
            end_overlaps(store, principal).await;
            assert_eq!(
                keyed_bootstrap(&keyed, url, &key).await.0,
                StatusCode::UNAUTHORIZED,
                "{what}: overlap over"
            );
            let (_, audit) = get(&client, (url, "/api/control/audit?limit=100"), &ada).await;
            assert!(
                audit
                    .as_array()
                    .expect("audit")
                    .iter()
                    .any(
                        |event| event["action"] == json!("service_account.key_refused")
                            && event["actor_name"] == json!("orders-deploy")
                    ),
                "{what}: a known key refused is audited under its account"
            );

            // Rotation without an overlap stops the current key at once.
            let (_, rotated) = post(
                &client,
                (url, &path),
                &ada,
                &json!({"overlap_minutes": null}),
            )
            .await;
            let third = rotated["key"].as_str().expect("third key").to_owned();
            assert_eq!(
                keyed_bootstrap(&keyed, url, &second).await.0,
                StatusCode::UNAUTHORIZED,
                "{what}: no overlap"
            );
            assert_eq!(keyed_bootstrap(&keyed, url, &third).await.0, StatusCode::OK);

            // Disabling stops its keys at once; enabling brings the same key back.
            let disabled = format!("/api/admin/service-accounts/{id}/disabled");
            let (status, _) =
                post(&client, (url, &disabled), &ada, &json!({"disabled": true})).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(
                keyed_bootstrap(&keyed, url, &third).await.0,
                StatusCode::UNAUTHORIZED,
                "{what}: disabled"
            );
            post(&client, (url, &disabled), &ada, &json!({"disabled": false})).await;
            assert_eq!(
                keyed_bootstrap(&keyed, url, &third).await.0,
                StatusCode::OK,
                "{what}: enabled"
            );

            // Last use moved only for requests that weren't refused; a key can be stopped outright.
            let (_, listed) = get(&client, (url, "/api/admin/service-accounts"), &ada).await;
            let account = listed
                .as_array()
                .expect("accounts")
                .iter()
                .find(|a| a["name"] == json!("orders-deploy"))
                .expect("account")
                .clone();
            assert!(
                account["last_used_at"].is_string(),
                "{what}: last use recorded"
            );
            let key_id = account["keys"][0]["key_id"].as_str().expect("key id");
            let revoke = format!("/api/admin/service-accounts/{id}/keys/{key_id}/revoke");
            let (status, after) = post(&client, (url, &revoke), &ada, &Value::Null).await;
            assert_eq!(
                (status, after["keys"].as_array().map(Vec::len)),
                (StatusCode::OK, Some(0))
            );
            assert_eq!(
                keyed_bootstrap(&keyed, url, &third).await.0,
                StatusCode::UNAUTHORIZED,
                "{what}: stopped"
            );

            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_people_with_and_without_administration_when_they_manage_service_accounts_then_only_administrators_can()
 {
    let test_cases = [
        ServiceAccountPermissionTestCase {
            description: "an administrator lists and adds",
            role: Some(BuiltinRole::Admin),
            expected_list: StatusCode::OK,
            expected_create: StatusCode::OK,
        },
        ServiceAccountPermissionTestCase {
            description: "a developer is refused both",
            role: Some(BuiltinRole::Developer),
            expected_list: StatusCode::FORBIDDEN,
            expected_create: StatusCode::FORBIDDEN,
        },
        ServiceAccountPermissionTestCase {
            description: "someone with no role is refused both",
            role: None,
            expected_list: StatusCode::FORBIDDEN,
            expected_create: StatusCode::FORBIDDEN,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(Backend::Sqlite).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            password_administrator(store, "ada").await;
            let person = password_person(store, "sam").await;
            if let Some(role) = test_case.role {
                let request = RoleAssignmentRequest {
                    principal_id: person,
                    role: RoleKey::Builtin(role),
                    location_id: None,
                    target: None,
                    revoke: false,
                };
                assign_role_in(
                    begin(store, TransactionMode::Write).await.expect("write"),
                    &owner(),
                    &request,
                )
                .await
                .expect("role");
            }
            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    login: HumanLogin::Password(ORIGIN_URL.into()),
                    ..ApiAuthentication::default()
                },
            )
            .await;
            let client = browser();
            let sam = session(&client, &api.url, "sam").await;
            let (list, _) = get(&client, (&api.url, "/api/admin/service-accounts"), &sam).await;
            let (create, _) = post(
                &client,
                (&api.url, "/api/admin/service-accounts"),
                &sam,
                &json!({"name": "ci-runner", "role": "operator"}),
            )
            .await;
            assert_eq!(
                (list, create),
                (test_case.expected_list, test_case.expected_create),
                "{}",
                test_case.description
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_chosen_key_when_a_service_account_is_created_then_only_the_owner_may_import_one_of_at_least_32_characters()
 {
    let test_cases = [
        ImportedKeyTestCase {
            description: "a key of 31 characters after the prefix is refused",
            key: "sqbk_0123456789abcdefghijABCDEFGHIJK",
            expected_error: Some("at least 32 characters"),
        },
        ImportedKeyTestCase {
            description: "a key without the prefix is refused",
            key: "0123456789abcdefghijABCDEFGHIJKLMNOPQRSTUV",
            expected_error: Some("must start with sqbk_"),
        },
        ImportedKeyTestCase {
            description: "a key with a space is refused",
            key: "sqbk_0123456789abcdefghij ABCDEFGHIJKLMNOP",
            expected_error: Some("without spaces"),
        },
        ImportedKeyTestCase {
            description: "a key of 32 characters after the prefix is imported and works",
            key: "sqbk_0123456789abcdefghijABCDEFGHIJKL",
            expected_error: None,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(Backend::Sqlite).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let request = NewServiceAccount {
                name: "orders-deploy".into(),
                description: None,
                role: BuiltinRole::Viewer,
            };
            let created = create_service_account(
                store,
                (&owner(), ServiceAccountAuthority::Owner),
                (&request, Some(test_case.key)),
            )
            .await;
            match test_case.expected_error {
                Some(expected) => {
                    let error = created.expect_err(test_case.description).to_string();
                    assert!(
                        error.contains(expected),
                        "{}: {error}",
                        test_case.description
                    );
                    assert!(rows(store, "sqb_service_accounts").await.is_empty());
                }
                None => {
                    let issued = created.expect(test_case.description);
                    assert_eq!(issued.key, test_case.key);
                    assert_eq!(
                        serde_json::to_value(issued.account.keys[0].origin).expect("origin"),
                        json!("imported")
                    );
                    let api =
                        start_api_with_authentication(store, ApiAuthentication::default()).await;
                    assert_eq!(
                        keyed_bootstrap(&script(), &api.url, test_case.key).await,
                        (StatusCode::OK, "orders-deploy".to_owned())
                    );
                    stop_api(api).await;
                }
            }
            let person = create_principal(store, &owner(), "ada")
                .await
                .expect("person");
            let refused = create_service_account(
                store,
                (&owner(), ServiceAccountAuthority::Person(person)),
                (
                    &NewServiceAccount {
                        name: "ci-runner".into(),
                        ..request.clone()
                    },
                    Some("sqbk_0123456789abcdefghijABCDEFGHIJKLMNOP"),
                ),
            )
            .await;
            assert!(
                refused.is_err_and(|error| error
                    .to_string()
                    .contains("only be imported with the CLI")),
                "{}: a person can't import a key",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_trusted_proxy_when_a_request_carries_the_identity_header_then_only_proxy_peers_are_trusted_and_a_key_wins()
 {
    let test_cases = [
        ProxyKeyTestCase {
            description: "a direct request with the identity header is not trusted",
            from_trusted_peer: false,
            with_key: false,
            expected_status: StatusCode::UNAUTHORIZED,
            expected_name: "",
        },
        ProxyKeyTestCase {
            description: "a direct keyed request is its service account, whatever the header says",
            from_trusted_peer: false,
            with_key: true,
            expected_status: StatusCode::OK,
            expected_name: "orders-deploy",
        },
        ProxyKeyTestCase {
            description: "the proxy's request is the person it names",
            from_trusted_peer: true,
            with_key: false,
            expected_status: StatusCode::OK,
            expected_name: "ada",
        },
        ProxyKeyTestCase {
            description: "a keyed request through the proxy is still the service account",
            from_trusted_peer: true,
            with_key: true,
            expected_status: StatusCode::OK,
            expected_name: "orders-deploy",
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(Backend::Sqlite).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let ada = create_principal(store, &owner(), "ada")
                .await
                .expect("person");
            let deploy = service_account(store, "orders-deploy", BuiltinRole::Developer).await;
            let peer = if test_case.from_trusted_peer {
                "::ffff:127.0.0.1"
            } else {
                "192.0.2.10"
            };
            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    login: HumanLogin::TrustedProxy(TrustedProxyAuthentication {
                        allowed_hosts: Vec::new(),
                        origin: ORIGIN_URL.into(),
                        trusted_peers: BTreeSet::from([peer.parse().expect("peer")]),
                        identities: BTreeMap::from([
                            ("ada".into(), ada),
                            ("deploy".into(), deploy.account.principal_id),
                        ]),
                        username_header: "X-SQB-User".into(),
                        display_name_header: None,
                        email_header: None,
                        unknown_users: UnknownProxyUsers::Reject,
                        proxy_logout_url: None,
                    }),
                    ..ApiAuthentication::default()
                },
            )
            .await;
            let request = |user: &str| {
                // The browser origin's host, so a request without a key reaches the identity header's check.
                let request = script()
                    .get(format!("{}/api/control/bootstrap", api.url))
                    .header(HOST, "control.example.com")
                    .header("x-sqb-user", user);
                if test_case.with_key {
                    request.bearer_auth(&deploy.key)
                } else {
                    request
                }
            };
            let response = request("ada").send().await.expect("request");
            let status = response.status();
            let body: Value = response.json().await.unwrap_or(Value::Null);
            assert_eq!(
                (status, body["access"]["name"].as_str().unwrap_or_default()),
                (test_case.expected_status, test_case.expected_name),
                "{}",
                test_case.description
            );
            if test_case.from_trusted_peer && !test_case.with_key {
                let bound = request("deploy").send().await.expect("service binding");
                assert_eq!(
                    bound.status(),
                    StatusCode::UNAUTHORIZED,
                    "{}: a proxy binding never signs a service account in",
                    test_case.description
                );
            }
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_server_without_sign_in_when_person_routes_are_called_then_only_service_keys_get_in() {
    let test_cases = [
        KeyOnlyTestCase {
            description: "a keyed request is its service account",
            with_key: true,
            expected_status: StatusCode::OK,
        },
        KeyOnlyTestCase {
            description: "a request without a key is refused",
            with_key: false,
            expected_status: StatusCode::UNAUTHORIZED,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(Backend::Sqlite).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let deploy = service_account(store, "orders-deploy", BuiltinRole::Viewer).await;
            let api = start_api_with_authentication(store, ApiAuthentication::default()).await;
            let request = script()
                .get(format!("{}/api/control/bootstrap", api.url))
                .header(HOST, NETWORK_HOST);
            let request = if test_case.with_key {
                request.bearer_auth(&deploy.key)
            } else {
                request
            };
            let response = request.send().await.expect("request");
            assert_eq!(
                response.status(),
                test_case.expected_status,
                "{}",
                test_case.description
            );
            assert_eq!(
                response
                    .headers()
                    .get("cache-control")
                    .and_then(|v| v.to_str().ok()),
                Some("no-store"),
                "{}: never cached",
                test_case.description
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}
