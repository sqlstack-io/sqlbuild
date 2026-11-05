//! Signed ID-token groups grant mapped roles through the live mapping and never more.

use std::collections::BTreeMap;

use serde_json::Value;
use uuid::Uuid;

use crate::api::_helpers::authentication::oidc_provider::configure;
use crate::api::models::{GroupRole, OidcAuthentication};
use crate::api::tests::helpers::stop_api;
use crate::api::tests::oidc_helpers::{
    authorize_in, sign_in, start_oidc_api, start_provider, stop_provider,
};
use crate::api::tests::oidc_test_types::ProviderFault;
use crate::api::tests::test_types::OidcGroupsTestCase;
use crate::credentials::main::identities::create_principal::create_principal;
use crate::credentials::models::{Action, AuditContext};
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_oidc_group_claims_when_mapped_then_roles_follow_the_live_mapping_and_bad_claims_grant_nothing()
 {
    let test_cases = [
        OidcGroupsTestCase {
            description: "SQLite OIDC groups",
            backend: Backend::Sqlite,
            expected_granting_role: "group:orders-operators:operator",
            expected_rejected_mappings: 4,
        },
        OidcGroupsTestCase {
            description: "PostgreSQL OIDC groups",
            backend: Backend::Postgres,
            expected_granting_role: "group:orders-operators:operator",
            expected_rejected_mappings: 4,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let provider = start_provider().await;
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../benchmarks/duckshop/edge_cases/orchestration/oidc_groups.json");
            let inputs: Value =
                serde_json::from_str(&std::fs::read_to_string(path).expect("DuckShop fixture"))
                    .expect("DuckShop OIDC group cases");
            let context = AuditContext {
                actor: "local-control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let person = create_principal(store, &context, "orders-person")
                .await
                .expect("person");
            let (orders, customers) = (Uuid::now_v7(), Uuid::now_v7());
            let mapped = GroupRole {
                group: inputs["mapped_group"].as_str().expect("group").into(),
                role: serde_json::from_value(inputs["mapped_role"].clone()).expect("role"),
                location_id: Some(orders),
                target: None,
            };
            let settings = OidcAuthentication {
                origin: "https://control.example.com".into(),
                issuer: provider.state.issuer.clone(),
                client_id: "orders-control".into(),
                client_secret_env: None,
                certificate_authority: Some(provider.authority.clone()),
                identities: BTreeMap::from([("orders-person".into(), person)]),
                groups_claim: Some(inputs["groups_claim"].as_str().expect("claim").into()),
                group_roles: vec![mapped],
            };
            let rejected = inputs["rejected_mappings"]
                .as_array()
                .expect("rejections")
                .iter()
                .filter(|case| {
                    let mut rejected = settings.clone();
                    rejected.group_roles =
                        vec![serde_json::from_value(case["mapping"].clone()).expect("mapping")];
                    configure(rejected).is_err()
                })
                .count();
            assert_eq!(
                rejected, test_case.expected_rejected_mappings,
                "{}: every unsafe mapping fails configuration",
                test_case.description
            );
            let mut unclaimed = settings.clone();
            unclaimed.groups_claim = None;
            assert!(configure(unclaimed).is_err(), "mapping needs a claim");

            let asserted = ProviderFault {
                claims: serde_json::Map::from_iter([(
                    inputs["groups_claim"].as_str().expect("claim").into(),
                    inputs["asserted_groups"].clone(),
                )]),
                ..Default::default()
            };
            let api = start_oidc_api(store, &settings).await;
            sign_in(&provider, &api.url, asserted.clone()).await;
            let granted = authorize_in(store, person, (Action::JobRun, orders)).await;
            assert_eq!(
                granted.expect("mapped group runs jobs in its location"),
                test_case.expected_granting_role
            );
            assert!(
                authorize_in(store, person, (Action::JobRun, customers))
                    .await
                    .is_err(),
                "the mapping's location bounds the role"
            );
            assert!(
                authorize_in(store, person, (Action::ViewSql, orders))
                    .await
                    .is_err(),
                "an operator cannot read SQL"
            );
            for case in inputs["claims_without_groups"]
                .as_array()
                .expect("bad claims")
            {
                let fault = ProviderFault {
                    claims: case["claims"].as_object().expect("claims").clone(),
                    ..Default::default()
                };
                sign_in(&provider, &api.url, fault).await;
                assert!(
                    authorize_in(store, person, (Action::JobRun, orders))
                        .await
                        .is_err(),
                    "{}: {} replaces earlier groups with none",
                    test_case.description,
                    case["description"]
                );
                sign_in(&provider, &api.url, asserted.clone()).await;
                assert!(
                    authorize_in(store, person, (Action::JobRun, orders))
                        .await
                        .is_ok()
                );
            }
            stop_api(api).await;
            let mut unmapped = settings.clone();
            unmapped.group_roles = Vec::new();
            let api = start_oidc_api(store, &unmapped).await;
            assert!(
                authorize_in(store, person, (Action::JobRun, orders))
                    .await
                    .is_err(),
                "{}: removing the mapping narrows access without a new sign-in",
                test_case.description
            );
            stop_api(api).await;
            close_test_store(fixture).await;
            stop_provider(provider).await;
        });
    }
}
