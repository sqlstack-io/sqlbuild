//! Standalone owner provisioning enables browser identity, never machine-route authority.

use std::time::Duration;

use reqwest::{StatusCode, header};
use serde_json::{Value, json};

use crate::admin_helpers::{administer, invoke_admin, invoke_password};
use crate::helpers::{api_ready, start_control, wait_for_exit};
use crate::test_types::{AdministratorCliTestCase, PasswordCliTestCase, PasswordOriginTestCase};

#[test]
fn given_local_store_ownership_when_create_user_runs_then_only_the_first_process_initializes_an_admin()
 {
    let test_cases = [AdministratorCliTestCase {
        description: "once-only owner bootstrap and authenticated administrator changes",
        expected_administrators: 1,
    }];
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    for test_case in test_cases {
        runtime.block_on(async {
        let mut control = start_control(&["server","--port","0","--no-scheduler","--password-origin","https://control.example.com"]);
        let url = api_ready(&mut control).await;
        let ordinary = administer(&control,&["create-principal","orders-reader"]);
        let first = administer(&control,&["create-user","orders-admin"]);
        assert_eq!(first["role"],"admin");
        let connection = rusqlite::Connection::open(control.directory.path().join(".sqlbuild/control.sqlite3")).expect("inspect initialization");
        let (count,id): (i64,String) = connection.query_row("SELECT COUNT(*),MIN(principal_id) FROM sqb_role_assignments WHERE role_key='admin' AND revoked_at IS NULL",[],|row| Ok((row.get(0)?,row.get(1)?))).expect("initial administrator membership");
        assert_eq!(count,test_case.expected_administrators,"{}",test_case.description);
        assert_eq!(id,first["principal_id"].as_str().expect("initial administrator UUID"));
        assert_ne!(id,ordinary["principal_id"].as_str().expect("ordinary person UUID"));
        let client = reqwest::Client::builder().timeout(Duration::from_secs(5)).build().expect("HTTP client");
        let mut sessions = Vec::new();
        for person in [&first,&ordinary] {
            let output = invoke_password(&control,person["principal_id"].as_str().expect("person UUID"),b"orders-admin-password-2026");
            assert!(output.status.success());
            let response = client.post(format!("{url}/api/auth/password/login"))
                .header(header::HOST,"control.example.com").header(header::ORIGIN,"https://control.example.com")
                .json(&json!({"username":person["name"],"password":"orders-admin-password-2026"})).send().await.expect("password login");
            assert_eq!(response.status(),StatusCode::OK);
            let cookie = response.headers().get_all(header::SET_COOKIE).iter().filter_map(|value| value.to_str().ok())
                .find(|value| value.starts_with("__Host-sqb-session=")).expect("session cookie").split(';').next().expect("cookie pair").to_owned();
            let body: Value = response.json().await.expect("session body");
            sessions.push((cookie,body["csrf_token"].as_str().expect("CSRF token").to_owned()));
        }
        let ordinary_id = ordinary["principal_id"].as_str().expect("ordinary person UUID");
        for (actor,target,action,expected) in [
            (1,ordinary_id,"grant",StatusCode::FORBIDDEN),
            (0,id.as_str(),"revoke",StatusCode::FORBIDDEN),
            (0,ordinary_id,"grant",StatusCode::OK),
            (1,id.as_str(),"revoke",StatusCode::OK),
            (0,id.as_str(),"grant",StatusCode::FORBIDDEN),
            (1,ordinary_id,"disable",StatusCode::FORBIDDEN),
        ] {
            let response = client.post(format!("{url}/api/admin/people/{target}"))
                .header(header::HOST,"control.example.com").header(header::ORIGIN,"https://control.example.com")
                .header(header::COOKIE,&sessions[actor].0).header("x-sqb-csrf",&sessions[actor].1)
                .json(&action).send().await.expect("administrator mutation response");
            assert_eq!(response.status(),expected,"{action} by actor {actor}");
            let body: Value = response.json().await.expect("mutation result");
            assert_eq!(body["applied"].as_bool(), (expected == StatusCode::OK).then_some(true));
            assert_eq!(body["error"].as_str().is_some_and(|message| message.contains("decision")),expected == StatusCode::FORBIDDEN);
        }
        for disabled_at in [None,Some("2026-01-01T00:00:00Z")] {
            connection.execute("UPDATE sqb_principals SET disabled_at=?1",[disabled_at]).expect("simulate out-of-band administrator loss");
            let rejected = invoke_admin(&control,&["create-user","replacement-admin"]);
            assert!(!rejected.status.success());
            assert!(rejected.stdout.is_empty());
            assert!(String::from_utf8_lossy(&rejected.stderr).contains("bootstrap is closed"));
        }
        let initialized: i64 = connection.query_row("SELECT COUNT(*) FROM sqb_security_audit WHERE action='admin.initialized' AND actor='local-control-store-owner'",[],|row| row.get(0)).expect("bootstrap audit");
        assert_eq!(initialized,1);
        #[cfg(unix)]
        crate::helpers::terminate(&mut control).await;
      });
    }
}

#[test]
fn given_owner_provisioned_passwords_when_a_standalone_server_authenticates_then_resets_invalidate_sessions_without_granting_authority()
 {
    let test_cases = [PasswordCliTestCase {
        description: "owner-provisioned identity remains separate from location authority",
        password: "  orders-password-café  ",
        replacement: "customers-password-2026",
        expected_password_rows: 1,
        expected_audits: 2,
    }];
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    for test_case in test_cases {
        runtime.block_on(async {
        let mut control = start_control(&["server", "--port", "0", "--no-scheduler", "--password-origin", "https://control.example.com"]);
        let url = api_ready(&mut control).await;
        let person = administer(&control, &["create-principal", "orders-reader"]);
        let principal = person["principal_id"].as_str().expect("person identity");
        let service = administer(&control, &["create-principal", "orders-service", "--kind", "service_account"]);
        let password = test_case.password;
        let invalid_inputs = [Vec::new(), b"short\n".to_vec(), vec![b'x'; 5000], vec![0xff, b'\n']];
        for input in invalid_inputs {
            let output = invoke_password(&control, principal, &input);
            assert!(!output.status.success(), "invalid stdin must not set a password");
            assert!(output.stdout.is_empty());
        }
        let rejected = invoke_password(&control, service["principal_id"].as_str().expect("service identity"), password.as_bytes());
        assert!(!rejected.status.success());
        assert!(rejected.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&rejected.stderr).contains(password));

        let client = reqwest::Client::builder().timeout(Duration::from_secs(5)).build().expect("HTTP client");
        let login = |password: &str| client.post(format!("{url}/api/auth/password/login"))
            .header(header::HOST, "control.example.com")
            .header(header::ORIGIN, "https://control.example.com")
            .json(&json!({"username": "orders-reader", "password": password})).send();
            let output = invoke_password(&control, principal, format!("{password}\r\n").as_bytes());
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
            let result: Value = serde_json::from_slice(&output.stdout).expect("JSON-only provisioning result");
            assert_eq!(result, json!({"principal_id": principal, "password_set": true}));
            assert!(!String::from_utf8_lossy(&output.stderr).contains(password));
            let response = login(password).await.expect("provisioned password login");
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            let cookie = response.headers().get_all(header::SET_COOKIE).iter()
                .filter_map(|header| header.to_str().ok())
                .find(|value| value.starts_with("__Host-sqb-session="))
                .expect("host-only session cookie").split(';').next().expect("cookie pair").to_owned();
            let body: Value = response.json().await.expect("session identity");
            assert_eq!(body["identity"]["principal_id"], principal);
            let session = client.get(format!("{url}/api/auth/session"))
                .header(header::HOST, "control.example.com").header(header::COOKIE, &cookie)
                .send().await.expect("current session");
            assert_eq!(session.status(), StatusCode::OK);
            let machine = client.get(format!("{url}/api/locations/{}/targets/development/triggers", uuid::Uuid::now_v7()))
                .header(header::HOST, "control.example.com").header(header::COOKIE, &cookie)
                .send().await.expect("browser cookie has no location authority");
            assert_eq!(machine.status(), StatusCode::UNAUTHORIZED);
            let reset = invoke_password(&control, principal, test_case.replacement.as_bytes());
            assert!(reset.status.success(), "{}", String::from_utf8_lossy(&reset.stderr));
            assert_eq!(serde_json::from_slice::<Value>(&reset.stdout).expect("reset JSON"), json!({"principal_id": principal, "password_set": true}));
            assert!(!String::from_utf8_lossy(&reset.stderr).contains(test_case.replacement));
            let previous = client.get(format!("{url}/api/auth/session"))
                .header(header::HOST, "control.example.com").header(header::COOKIE, cookie)
                .send().await.expect("previous session after child-process reset");
            assert_eq!(previous.status(), StatusCode::UNAUTHORIZED);
            assert_eq!(login(password).await.expect("old password rejected").status(), StatusCode::UNAUTHORIZED);
            assert_eq!(login(test_case.replacement).await.expect("replacement password accepted").status(), StatusCode::OK);
        let connection = rusqlite::Connection::open(control.directory.path().join(".sqlbuild/control.sqlite3")).expect("inspect durable provisioning");
        let (count, hash): (i64, String) = connection.query_row(
            "SELECT COUNT(*), MIN(password_hash) FROM sqb_password_credentials", [],
            |row| Ok((row.get(0)?, row.get(1)?))).expect("password storage");
        assert_eq!(count, test_case.expected_password_rows, "{}", test_case.description);
        assert!(hash.starts_with("$argon2id$"));
        let audits: i64 = connection.query_row(
            "SELECT COUNT(*) FROM sqb_security_audit WHERE action='password.changed' AND actor='local-control-store-owner' AND granting_role='control-store-owner'", [],
            |row| row.get(0)).expect("owner authority audit");
        assert_eq!(audits, test_case.expected_audits, "{}", test_case.description);
        #[cfg(unix)]
        crate::helpers::terminate(&mut control).await;
      });
    }
}

#[test]
fn given_invalid_password_origins_when_a_standalone_server_starts_then_it_exits_with_a_configuration_error()
 {
    let test_cases = [
        PasswordOriginTestCase {
            description: "HTTP is not an external HTTPS origin",
            origin: "http://control.example.com",
            expected_error: "password authentication requires an HTTPS origin",
        },
        PasswordOriginTestCase {
            description: "origin cannot contain a path",
            origin: "https://control.example.com/path",
            expected_error: "password authentication requires an HTTPS origin",
        },
        PasswordOriginTestCase {
            description: "origin cannot contain credentials",
            origin: "https://user:password@control.example.com",
            expected_error: "password authentication requires an HTTPS origin",
        },
        PasswordOriginTestCase {
            description: "empty origin is not a request to disable authentication",
            origin: "",
            expected_error: "password authentication requires an HTTPS origin",
        },
    ];
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    for test_case in test_cases {
        runtime.block_on(async {
            let origin = test_case.origin;
            let mut control = start_control(&[
                "server",
                "--port",
                "0",
                "--no-scheduler",
                "--password-origin",
                origin,
            ]);
            let status = wait_for_exit(&mut control).await;
            assert!(!status.success());
            let log = std::fs::read_to_string(&control.stderr).expect("startup error");
            assert!(
                log.contains(test_case.expected_error),
                "{}",
                test_case.description
            );
            assert!(
                !log.contains(origin) || origin.is_empty(),
                "invalid configuration is not echoed"
            );
        });
    }
}
