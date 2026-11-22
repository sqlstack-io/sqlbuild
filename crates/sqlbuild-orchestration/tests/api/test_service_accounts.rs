//! Service accounts call with a key on any address; only its hash is kept, and it acts under its own name.

use std::collections::{BTreeMap, BTreeSet};

use reqwest::StatusCode;
use reqwest::header::HOST;
use serde_json::{Value, json};

use crate::api::models::{
    ApiAuthentication, HumanLogin, TrustedProxyAuthentication, UnknownProxyUsers,
};
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::password_people_helpers::{
    ORIGIN_URL, PASSWORD, browser, password_administrator, password_person, session,
};
use crate::api::tests::service_accounts_helpers::{
    NETWORK_HOST, audited, end_overlaps, keyed_bootstrap, no_sign_in, owner, password_sign_in,
    person_get, person_post, rows, script, service_account, with_key, without_key,
};
use crate::api::tests::test_types::{
    ImportedKeyAcceptedTestCase, ImportedKeyTestCase, KeyOnlyTestCase, KeyedManagementTestCase,
    LoginModeTestCase, PersonImportTestCase, ProxyBindingTestCase, ProxyKeyTestCase,
    ServiceAccountLifecycleTestCase, ServiceAccountPermissionTestCase, ServiceAccountRoleTestCase,
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

#[test]
fn given_a_service_account_when_its_key_is_used_rotated_and_disabled_then_only_working_keys_authenticate()
 {
    let test_cases = [
        ServiceAccountLifecycleTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_key_length: 45,
            expected_overlap_keys: 2,
        },
        ServiceAccountLifecycleTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_key_length: 45,
            expected_overlap_keys: 2,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
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

            let (status, issued) = person_post(
                &client,
                (url, "/api/admin/service-accounts"),
                &ada,
                &json!({"name": "orders-deploy", "description": "Release pipeline", "role": "developer"}),
            )
            .await;
            assert_eq!(
                status,
                StatusCode::OK,
                "{}: create {issued}",
                test_case.description
            );
            let key = issued["key"].as_str().expect("key").to_owned();
            let id = issued["account"]["principal_id"]
                .as_str()
                .expect("id")
                .to_owned();
            assert_eq!(
                (key.starts_with("sqbk_"), key.len()),
                (true, test_case.expected_key_length),
                "{}: key shape",
                test_case.description
            );
            assert_eq!(
                (
                    &issued["account"]["keys"][0]["prefix"],
                    &issued["account"]["role"]
                ),
                (&json!(key[..11]), &json!("developer")),
                "{}: prefix and role",
                test_case.description
            );
            let (_, listed) = person_get(&client, (url, "/api/admin/service-accounts"), &ada).await;
            assert!(
                listed.to_string().contains("orders-deploy") && !listed.to_string().contains(&key),
                "{}: listed without its key",
                test_case.description
            );
            let stored = rows(store, "sqb_service_account_keys").await;
            assert_eq!(
                stored[0]["secret_hash"],
                json!(key_hash(&key)),
                "{}: only the hash is stored",
                test_case.description
            );
            for table in [
                "sqb_service_account_keys",
                "sqb_service_accounts",
                "sqb_principals",
                "sqb_security_audit",
            ] {
                let text = serde_json::to_string(&rows(store, table).await).expect("rows");
                assert!(
                    !text.contains(&key[5..]),
                    "{}: {table} keeps no key",
                    test_case.description
                );
            }
            let (_, people) = person_get(&client, (url, "/api/admin/people"), &ada).await;
            assert!(
                !people.to_string().contains("orders-deploy"),
                "{}: not listed as a person",
                test_case.description
            );

            assert_eq!(
                keyed_bootstrap(&keyed, url, &key).await,
                (StatusCode::OK, "orders-deploy".to_owned()),
                "{}: a keyed request from any address",
                test_case.description
            );
            let session_read = keyed
                .get(format!("{url}/api/auth/session"))
                .header(HOST, NETWORK_HOST)
                .bearer_auth(&key)
                .send()
                .await
                .expect("session read");
            let foreign_login = keyed
                .post(format!("{url}/api/auth/password/login"))
                .header(HOST, NETWORK_HOST)
                .bearer_auth(&key)
                .json(&json!({"username": "ada", "password": PASSWORD}))
                .send()
                .await
                .expect("login under a foreign host");
            assert_eq!(
                (session_read.status(), foreign_login.status()),
                (StatusCode::UNAUTHORIZED, StatusCode::UNAUTHORIZED),
                "{}: no session, and a key doesn't lift sign-in's host check",
                test_case.description
            );

            let ops = service_account(store, "ops-bot", BuiltinRole::Admin).await;
            let sam = password_person(store, "sam").await;
            let added = keyed
                .post(format!("{url}/api/admin/people/{sam}"))
                .header(HOST, NETWORK_HOST)
                .bearer_auth(&ops.key)
                .json(&json!("disable"))
                .send()
                .await
                .expect("keyed change");
            let refused = keyed
                .get(format!("{url}/api/admin/service-accounts"))
                .header(HOST, NETWORK_HOST)
                .bearer_auth(&key)
                .send()
                .await
                .expect("keyed refusal");
            assert_eq!(
                (added.status(), refused.status()),
                (StatusCode::OK, StatusCode::FORBIDDEN),
                "{}: keyed calls follow the account's role",
                test_case.description
            );
            let (_, audit) = person_get(&client, (url, "/api/control/audit?limit=100"), &ada).await;
            assert!(
                audited(&audit, ("ops-bot", "principal.disabled"))
                    && audited(&audit, ("orders-deploy", "authorization.denied")),
                "{}: its actions and refusals are audited under its name",
                test_case.description
            );

            let keys = format!("/api/admin/service-accounts/{id}/keys");
            let (status, rotated) =
                person_post(&client, (url, &keys), &ada, &json!({"overlap_minutes": 60})).await;
            let second = rotated["key"].as_str().expect("new key").to_owned();
            assert_eq!(
                (status, rotated["account"]["keys"].as_array().map(Vec::len)),
                (StatusCode::OK, Some(test_case.expected_overlap_keys)),
                "{}: rotation with an overlap",
                test_case.description
            );
            assert_eq!(
                (
                    keyed_bootstrap(&keyed, url, &key).await.0,
                    keyed_bootstrap(&keyed, url, &second).await.0
                ),
                (StatusCode::OK, StatusCode::OK),
                "{}: both keys work during the overlap",
                test_case.description
            );
            end_overlaps(store, uuid::Uuid::parse_str(&id).expect("uuid")).await;
            let (_, audit) = person_get(&client, (url, "/api/control/audit?limit=100"), &ada).await;
            assert_eq!(
                (
                    keyed_bootstrap(&keyed, url, &key).await.0,
                    keyed_bootstrap(&keyed, url, &second).await.0
                ),
                (StatusCode::UNAUTHORIZED, StatusCode::OK),
                "{}: the old key stops when the overlap ends",
                test_case.description
            );
            let (_, audit_after) =
                person_get(&client, (url, "/api/control/audit?limit=100"), &ada).await;
            assert!(
                !audited(&audit, ("orders-deploy", "service_account.key_refused"))
                    && audited(
                        &audit_after,
                        ("orders-deploy", "service_account.key_refused")
                    ),
                "{}: a known key refused is audited under its account",
                test_case.description
            );

            let (_, rotated) = person_post(
                &client,
                (url, &keys),
                &ada,
                &json!({"overlap_minutes": null}),
            )
            .await;
            let third = rotated["key"].as_str().expect("third key").to_owned();
            let disabled = format!("/api/admin/service-accounts/{id}/disabled");
            let after_rotation = keyed_bootstrap(&keyed, url, &second).await.0;
            person_post(&client, (url, &disabled), &ada, &json!({"disabled": true})).await;
            let while_disabled = keyed_bootstrap(&keyed, url, &third).await.0;
            person_post(&client, (url, &disabled), &ada, &json!({"disabled": false})).await;
            let once_enabled = keyed_bootstrap(&keyed, url, &third).await.0;
            assert_eq!(
                (after_rotation, while_disabled, once_enabled),
                (
                    StatusCode::UNAUTHORIZED,
                    StatusCode::UNAUTHORIZED,
                    StatusCode::OK
                ),
                "{}: no overlap stops the old key; disabling stops every key until enabled",
                test_case.description
            );

            let (_, listed) = person_get(&client, (url, "/api/admin/service-accounts"), &ada).await;
            let account = listed
                .as_array()
                .into_iter()
                .flatten()
                .find(|a| a["name"] == json!("orders-deploy"))
                .cloned()
                .unwrap_or(Value::Null);
            let key_id = account["keys"][0]["key_id"].as_str().expect("key id");
            let revoke = format!("/api/admin/service-accounts/{id}/keys/{key_id}/revoke");
            let (status, after) = person_post(&client, (url, &revoke), &ada, &Value::Null).await;
            assert_eq!(
                (
                    account["last_used_at"].is_string(),
                    status,
                    after["keys"].as_array().map(Vec::len),
                    keyed_bootstrap(&keyed, url, &third).await.0
                ),
                (true, StatusCode::OK, Some(0), StatusCode::UNAUTHORIZED),
                "{}: last use recorded, and a key can be stopped outright",
                test_case.description
            );

            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_people_with_built_in_roles_when_they_manage_service_accounts_then_only_administrators_can()
{
    let test_cases = [
        ServiceAccountPermissionTestCase {
            description: "an administrator lists and adds",
            role: BuiltinRole::Admin,
            expected_list: StatusCode::OK,
            expected_create: StatusCode::OK,
        },
        ServiceAccountPermissionTestCase {
            description: "a developer is refused both",
            role: BuiltinRole::Developer,
            expected_list: StatusCode::FORBIDDEN,
            expected_create: StatusCode::FORBIDDEN,
        },
        ServiceAccountPermissionTestCase {
            description: "a viewer is refused both",
            role: BuiltinRole::Viewer,
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
            let role = RoleAssignmentRequest {
                principal_id: person,
                role: RoleKey::Builtin(test_case.role),
                location_id: None,
                target: None,
                revoke: false,
            };
            assign_role_in(
                begin(store, TransactionMode::Write).await.expect("write"),
                &owner(),
                &role,
            )
            .await
            .expect("role");
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
            let (list, _) =
                person_get(&client, (&api.url, "/api/admin/service-accounts"), &sam).await;
            let (create, _) = person_post(
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
fn given_a_chosen_key_that_breaks_the_import_rules_when_the_owner_imports_it_then_it_is_refused() {
    let test_cases = [
        ImportedKeyTestCase {
            description: "a key of 31 characters after the prefix is refused",
            key: "sqbk_0123456789abcdefghijABCDEFGHIJK",
            expected_error: "at least 32 characters",
        },
        ImportedKeyTestCase {
            description: "a key without the prefix is refused",
            key: "0123456789abcdefghijABCDEFGHIJKLMNOPQRSTUV",
            expected_error: "must start with sqbk_",
        },
        ImportedKeyTestCase {
            description: "a key with a space is refused",
            key: "sqbk_0123456789abcdefghij ABCDEFGHIJKLMNOP",
            expected_error: "without spaces",
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
            let refused = create_service_account(
                store,
                (&owner(), ServiceAccountAuthority::Owner),
                (&request, Some(test_case.key)),
            )
            .await
            .map(|issued| issued.account.name);
            let error = refused.map_err(|error| error.to_string());
            assert_eq!(
                (
                    error
                        .as_ref()
                        .err()
                        .is_some_and(|text| text.contains(test_case.expected_error)),
                    rows(store, "sqb_service_accounts").await.len()
                ),
                (true, 0),
                "{}: {error:?}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_chosen_key_of_at_least_32_characters_when_the_owner_imports_it_then_it_works() {
    let test_cases = [ImportedKeyAcceptedTestCase {
        description: "a key of 32 characters after the prefix",
        key: "sqbk_0123456789abcdefghijABCDEFGHIJKL",
        expected_origin: "imported",
        expected_status: StatusCode::OK,
    }];
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
            let issued = create_service_account(
                store,
                (&owner(), ServiceAccountAuthority::Owner),
                (&request, Some(test_case.key)),
            )
            .await
            .expect("imported");
            let api = start_api_with_authentication(store, ApiAuthentication::default()).await;
            assert_eq!(
                (
                    issued.key.as_str(),
                    serde_json::to_value(issued.account.keys[0].origin).expect("origin"),
                    keyed_bootstrap(&script(), &api.url, test_case.key).await.0
                ),
                (
                    test_case.key,
                    json!(test_case.expected_origin),
                    test_case.expected_status
                ),
                "{}",
                test_case.description
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_person_when_they_offer_a_chosen_key_then_only_the_cli_may_import_one() {
    let test_cases = [PersonImportTestCase {
        description: "a person's request with a chosen key",
        expected_error: "only be imported with the CLI",
    }];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(Backend::Sqlite).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let person = create_principal(store, &owner(), "ada")
                .await
                .expect("person");
            let request = NewServiceAccount {
                name: "ci-runner".into(),
                description: None,
                role: BuiltinRole::Viewer,
            };
            let refused = create_service_account(
                store,
                (&owner(), ServiceAccountAuthority::Person(person)),
                (&request, Some("sqbk_0123456789abcdefghijABCDEFGHIJKLMNOP")),
            )
            .await;
            assert!(
                refused.is_err_and(|error| error.to_string().contains(test_case.expected_error)),
                "{}",
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
            trusted_peer: "192.0.2.10",
            authorize: without_key,
            expected_status: StatusCode::UNAUTHORIZED,
            expected_name: "",
        },
        ProxyKeyTestCase {
            description: "a direct keyed request is its service account, whatever the header says",
            trusted_peer: "192.0.2.10",
            authorize: with_key,
            expected_status: StatusCode::OK,
            expected_name: "orders-deploy",
        },
        ProxyKeyTestCase {
            description: "the proxy's request is the person it names",
            trusted_peer: "::ffff:127.0.0.1",
            authorize: without_key,
            expected_status: StatusCode::OK,
            expected_name: "ada",
        },
        ProxyKeyTestCase {
            description: "a keyed request through the proxy is still the service account",
            trusted_peer: "::ffff:127.0.0.1",
            authorize: with_key,
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
            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    login: HumanLogin::TrustedProxy(TrustedProxyAuthentication {
                        allowed_hosts: Vec::new(),
                        origin: ORIGIN_URL.into(),
                        trusted_peers: BTreeSet::from([test_case
                            .trusted_peer
                            .parse()
                            .expect("peer")]),
                        identities: BTreeMap::from([("ada".into(), ada)]),
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
            let request = script()
                .get(format!("{}/api/control/bootstrap", api.url))
                .header(HOST, "control.example.com")
                .header("x-sqb-user", "ada");
            let response = (test_case.authorize)(request, &deploy.key)
                .send()
                .await
                .expect("request");
            let status = response.status();
            let body: Value = response.json().await.unwrap_or(Value::Null);
            assert_eq!(
                (status, body["access"]["name"].as_str().unwrap_or_default()),
                (test_case.expected_status, test_case.expected_name),
                "{}",
                test_case.description
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_proxy_binding_to_a_service_account_when_the_proxy_names_it_then_it_never_signs_in() {
    let test_cases = [ProxyBindingTestCase {
        description: "a trusted proxy names a username bound to a service account",
        expected_status: StatusCode::UNAUTHORIZED,
    }];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(Backend::Sqlite).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let deploy = service_account(store, "orders-deploy", BuiltinRole::Developer).await;
            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    login: HumanLogin::TrustedProxy(TrustedProxyAuthentication {
                        allowed_hosts: Vec::new(),
                        origin: ORIGIN_URL.into(),
                        trusted_peers: BTreeSet::from(["::ffff:127.0.0.1".parse().expect("peer")]),
                        identities: BTreeMap::from([(
                            "deploy".into(),
                            deploy.account.principal_id,
                        )]),
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
            let response = script()
                .get(format!("{}/api/control/bootstrap", api.url))
                .header(HOST, "control.example.com")
                .header("x-sqb-user", "deploy")
                .send()
                .await
                .expect("request");
            assert_eq!(
                response.status(),
                test_case.expected_status,
                "{}",
                test_case.description
            );
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
            authorize: with_key,
            expected_status: StatusCode::OK,
            expected_cache_control: "no-store",
        },
        KeyOnlyTestCase {
            description: "a request without a key is refused",
            authorize: without_key,
            expected_status: StatusCode::UNAUTHORIZED,
            expected_cache_control: "no-store",
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
            let response = (test_case.authorize)(request, &deploy.key)
                .send()
                .await
                .expect("request");
            assert_eq!(
                (
                    response.status(),
                    response
                        .headers()
                        .get("cache-control")
                        .and_then(|v| v.to_str().ok())
                ),
                (
                    test_case.expected_status,
                    Some(test_case.expected_cache_control)
                ),
                "{}",
                test_case.description
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_service_account_with_the_admin_role_when_its_key_manages_accounts_or_roles_then_it_is_refused_and_audited()
 {
    let test_cases = [
        KeyedManagementTestCase {
            description: "adding a service account",
            path: "/api/admin/service-accounts",
            body: r#"{"name": "planted", "role": "admin"}"#,
            expected_status: StatusCode::FORBIDDEN,
        },
        KeyedManagementTestCase {
            description: "rotating a service account's key",
            path: "/api/admin/service-accounts/{target}/keys",
            body: r#"{"overlap_minutes": 60}"#,
            expected_status: StatusCode::FORBIDDEN,
        },
        KeyedManagementTestCase {
            description: "enabling or disabling a service account",
            path: "/api/admin/service-accounts/{target}/disabled",
            body: r#"{"disabled": true}"#,
            expected_status: StatusCode::FORBIDDEN,
        },
        KeyedManagementTestCase {
            description: "changing a service account's role",
            path: "/api/admin/service-accounts/{target}/role",
            body: r#"{"role": "admin"}"#,
            expected_status: StatusCode::FORBIDDEN,
        },
        KeyedManagementTestCase {
            description: "stopping a service account's key",
            path: "/api/admin/service-accounts/{target}/keys/{key}/revoke",
            body: "null",
            expected_status: StatusCode::FORBIDDEN,
        },
        KeyedManagementTestCase {
            description: "giving a person a role",
            path: "/api/admin/roles",
            body: r#"{"principal_id": "{person}", "role": "developer", "location_id": null, "target": null, "revoke": false}"#,
            expected_status: StatusCode::FORBIDDEN,
        },
        KeyedManagementTestCase {
            description: "making a person an admin",
            path: "/api/admin/people/{person}",
            body: r#""grant""#,
            expected_status: StatusCode::FORBIDDEN,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(Backend::Sqlite).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let person = password_person(store, "sam").await;
            let ops = service_account(store, "ops-bot", BuiltinRole::Admin).await;
            let target = service_account(store, "orders-deploy", BuiltinRole::Viewer).await;
            let fill = |text: &str| {
                text.replace("{target}", &target.account.principal_id.to_string())
                    .replace("{key}", &target.account.keys[0].key_id.to_string())
                    .replace("{person}", &person.to_string())
            };
            let api = start_api_with_authentication(store, ApiAuthentication::default()).await;
            let body: Value = serde_json::from_str(&fill(test_case.body)).expect("body");
            let response = script()
                .post(format!("{}{}", api.url, fill(test_case.path)))
                .header(HOST, NETWORK_HOST)
                .bearer_auth(&ops.key)
                .json(&body)
                .send()
                .await
                .expect("keyed change");
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            let denials = rows(store, "sqb_security_audit")
                .await
                .into_iter()
                .filter(|row| row["action"] == json!("authorization.denied"))
                .count();
            assert_eq!(
                (
                    status,
                    text.contains("a person must make this change"),
                    denials
                ),
                (test_case.expected_status, true, 1),
                "{}: {text}",
                test_case.description
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_service_account_role_change_when_it_applies_then_the_new_role_replaces_the_old_in_one_change()
 {
    let test_cases = [
        ServiceAccountRoleTestCase {
            description: "admin to viewer on SQLite",
            backend: Backend::Sqlite,
            from: BuiltinRole::Admin,
            to: "viewer",
            expected_roles: &["viewer"],
        },
        ServiceAccountRoleTestCase {
            description: "admin to viewer on PostgreSQL",
            backend: Backend::Postgres,
            from: BuiltinRole::Admin,
            to: "viewer",
            expected_roles: &["viewer"],
        },
        ServiceAccountRoleTestCase {
            description: "viewer to admin on PostgreSQL",
            backend: Backend::Postgres,
            from: BuiltinRole::Viewer,
            to: "admin",
            expected_roles: &["admin"],
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            password_administrator(store, "ada").await;
            let deploy = service_account(store, "orders-deploy", test_case.from).await;
            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    login: HumanLogin::Password(ORIGIN_URL.into()),
                    ..ApiAuthentication::default()
                },
            )
            .await;
            let client = browser();
            let ada = session(&client, &api.url, "ada").await;
            let path = format!(
                "/api/admin/service-accounts/{}/role",
                deploy.account.principal_id
            );
            let (status, account) = person_post(
                &client,
                (&api.url, &path),
                &ada,
                &json!({"role": test_case.to}),
            )
            .await;
            let mut held: Vec<String> = rows(store, "sqb_role_assignments")
                .await
                .into_iter()
                .filter(|row| row["principal_id"] == json!(deploy.account.principal_id.to_string()))
                .filter_map(|row| row["role_key"].as_str().map(str::to_owned))
                .collect();
            held.sort();
            assert_eq!(
                (status, account["role"].clone(), held),
                (
                    StatusCode::OK,
                    json!(test_case.to),
                    test_case
                        .expected_roles
                        .iter()
                        .map(|r| (*r).to_owned())
                        .collect::<Vec<_>>()
                ),
                "{}",
                test_case.description
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_server_without_sign_in_when_asked_how_people_sign_in_then_it_answers_keys_only() {
    let test_cases = [
        LoginModeTestCase {
            description: "a server without sign-in for people",
            login: no_sign_in,
            expected_status: StatusCode::OK,
            expected_mode: "keys",
        },
        LoginModeTestCase {
            description: "a server with password sign-in",
            login: password_sign_in,
            expected_status: StatusCode::NOT_FOUND,
            expected_mode: "",
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(Backend::Sqlite).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    login: (test_case.login)(),
                    ..ApiAuthentication::default()
                },
            )
            .await;
            let response = script()
                .get(format!("{}/api/auth/login-mode", api.url))
                .header(HOST, "control.example.com")
                .send()
                .await
                .expect("login mode");
            let status = response.status();
            let body: Value = response.json().await.unwrap_or(Value::Null);
            assert_eq!(
                (status, body["login_mode"].as_str().unwrap_or_default()),
                (test_case.expected_status, test_case.expected_mode),
                "{}",
                test_case.description
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}
