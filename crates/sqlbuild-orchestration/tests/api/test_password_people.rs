//! Administrators add password people over HTTP; only password sign-in offers it.

use reqwest::header::COOKIE;
use reqwest::{Client, StatusCode};
use serde_json::json;
use uuid::Uuid;

use crate::api::models::{ApiAuthentication, DevelopmentScope, HumanLogin};
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::password_people_helpers::{
    NEW_PASSWORD, ORIGIN_URL, browser, context, password_administrator, password_person, roles_of,
    session, sign_in_status,
};
use crate::api::tests::test_types::{NoPasswordPeopleTestCase, PasswordPersonTestCase};
use crate::credentials::main::identities::development_person::development_person;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_password_sign_in_when_adding_a_person_then_only_people_managers_can_and_the_person_signs_in()
 {
    let test_cases = [
        PasswordPersonTestCase {
            description: "an administrator adds a person with a role",
            signed_in_as: &["ada"],
            body: json!({"name": "morgan", "password": NEW_PASSWORD, "role": "developer"}),
            expected_status: StatusCode::OK,
            expected_sign_in: true,
            expected_roles: &["developer"],
        },
        PasswordPersonTestCase {
            description: "an administrator adds a person with only the default role",
            signed_in_as: &["ada"],
            body: json!({"name": "morgan", "password": NEW_PASSWORD}),
            expected_status: StatusCode::OK,
            expected_sign_in: true,
            expected_roles: &[],
        },
        PasswordPersonTestCase {
            description: "someone without user.manage is refused",
            signed_in_as: &["vic"],
            body: json!({"name": "morgan", "password": NEW_PASSWORD}),
            expected_status: StatusCode::FORBIDDEN,
            expected_sign_in: false,
            expected_roles: &[],
        },
        PasswordPersonTestCase {
            description: "a signed-out request is refused",
            signed_in_as: &[],
            body: json!({"name": "morgan", "password": NEW_PASSWORD}),
            expected_status: StatusCode::UNAUTHORIZED,
            expected_sign_in: false,
            expected_roles: &[],
        },
        PasswordPersonTestCase {
            description: "a password shorter than the minimum is refused",
            signed_in_as: &["ada"],
            body: json!({"name": "morgan", "password": "short"}),
            expected_status: StatusCode::BAD_REQUEST,
            expected_sign_in: false,
            expected_roles: &[],
        },
        PasswordPersonTestCase {
            description: "a name someone already has is refused",
            signed_in_as: &["ada"],
            body: json!({"name": "vic", "password": NEW_PASSWORD}),
            expected_status: StatusCode::BAD_REQUEST,
            expected_sign_in: false,
            expected_roles: &[],
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
            password_person(store, "vic").await;
            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    login: HumanLogin::Password(ORIGIN_URL.into()),
                    ..ApiAuthentication::default()
                },
            )
            .await;
            let client = browser();
            let admin = session(&client, &api.url, "ada").await;
            let mut headers: Vec<(String, String)> = Vec::new();
            for name in test_case.signed_in_as {
                headers.push(session(&client, &api.url, name).await);
            }
            let request = headers.iter().fold(
                client.post(format!("{}/api/auth/password/people", api.url)),
                |request, (cookie, csrf)| request.header(COOKIE, cookie).header("x-sqb-csrf", csrf),
            );
            let status = request
                .json(&test_case.body)
                .send()
                .await
                .expect("add person")
                .status();
            let name = test_case.body["name"].as_str().expect("name");
            let password = test_case.body["password"].as_str().expect("password");
            let signs_in =
                sign_in_status(&client, &api.url, (name, password)).await == StatusCode::OK;
            let roles = roles_of(&client, &api.url, &admin, "morgan").await;

            assert_eq!(
                (status, signs_in, roles),
                (
                    test_case.expected_status,
                    test_case.expected_sign_in,
                    test_case
                        .expected_roles
                        .iter()
                        .map(|role| (*role).to_owned())
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
fn given_sign_in_without_passwords_when_adding_a_person_then_the_server_has_no_such_route() {
    let test_cases = [
        NoPasswordPeopleTestCase {
            description: "the development person administers everything, yet cannot add password people",
            login: |principal_id| {
                HumanLogin::Development(DevelopmentScope {
                    location_id: Uuid::now_v7(),
                    target: "development".into(),
                    triggers: false,
                    principal_id,
                    allowed_hosts: Vec::new(),
                    listens_beyond_loopback: false,
                })
            },
            expected_status: StatusCode::NOT_FOUND,
        },
        NoPasswordPeopleTestCase {
            description: "a server without human sign-in has no such route",
            login: |_| HumanLogin::None,
            expected_status: StatusCode::NOT_FOUND,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(Backend::Sqlite).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let principal_id = development_person(store, &context())
                .await
                .expect("development person");
            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    login: (test_case.login)(principal_id),
                    ..ApiAuthentication::default()
                },
            )
            .await;
            let status = Client::new()
                .post(format!("{}/api/auth/password/people", api.url))
                .json(&json!({"name": "morgan", "password": NEW_PASSWORD}))
                .send()
                .await
                .expect("add person")
                .status();

            assert_eq!(
                status, test_case.expected_status,
                "{}",
                test_case.description
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}
