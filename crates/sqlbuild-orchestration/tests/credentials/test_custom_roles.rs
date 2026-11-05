//! Published custom roles take effect at once: widening, narrowing and removal alike.

use serde_json::{Value, json};

use crate::credentials::models::Action;
use crate::credentials::tests::role_helpers::{
    granting_role, orders_grant, publish_support_role, role_fixture,
};
use crate::credentials::tests::test_types::CustomRoleTestCase;
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_published_custom_roles_when_republished_then_each_definition_applies_at_once() {
    let test_cases = [
        CustomRoleTestCase {
            description: "SQLite applies each publication",
            backend: Backend::Sqlite,
            expected_after_removal: json!({"view": null, "job.run": null}),
        },
        CustomRoleTestCase {
            description: "PostgreSQL applies each publication",
            backend: Backend::Postgres,
            expected_after_removal: json!({"view": null, "job.run": null}),
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
                "../../benchmarks/duckshop/edge_cases/orchestration/custom_role_publication.json",
            );
            let case: Value =
                serde_json::from_str(&std::fs::read_to_string(path).expect("role fixture"))
                    .expect("role case");
            let roles = role_fixture(store).await;
            let grants = || async {
                json!({
                    "view": granting_role(store, &roles, Action::View).await,
                    "job.run": granting_role(store, &roles, Action::JobRun).await,
                })
            };
            let publications = case["publications"].as_array().expect("publications");
            for (generation, publication) in (2..).zip(publications) {
                publish_support_role(
                    store,
                    roles.location_id,
                    (
                        generation,
                        json!([orders_grant(publication["permissions"].clone(), &["orders"])]),
                    ),
                )
                .await;
                assert_eq!(
                    grants().await,
                    publication["expected"],
                    "{}: {}",
                    test_case.description,
                    publication["permissions"]
                );
            }
            let mut removed = snapshot(roles.location_id);
            removed.generation = 9;
            publish_snapshot(store, &removed)
                .await
                .expect("publish without roles");
            assert_eq!(grants().await, test_case.expected_after_removal);
            close_test_store(fixture).await;
        });
    }
}
