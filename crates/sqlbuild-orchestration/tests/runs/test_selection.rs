//! People who may view a target evaluate selections and read selector values against its current publication.

use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::credentials::tests::location_helpers::create_location;
use crate::runs::main::requests::evaluate_selection::evaluate_selection;
use crate::runs::main::requests::read_selection_vocabulary::read_selection_vocabulary;
use crate::runs::models::{SelectionQuery, VocabularyEntry};
use crate::runs::tests::helpers::{
    adhoc_fixture, launch_outcome, owned_selection, selection_outcome,
};
use crate::runs::tests::test_types::{AdhocPerson, ExactKeyPlanner, SelectionRequestTestCase};
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_selection_requests_when_people_evaluate_them_then_viewers_get_pages_and_refusals() {
    let test_cases = [
        SelectionRequestTestCase {
            description: "a viewer pages through the selected keys in order",
            person: AdhocPerson::Developer,
            select: "model:orders model:customers",
            exclude: "",
            limit: Some(1),
            expected_outcome: Ok((2, &["model:customers"], Some(1), None)),
        },
        SelectionRequestTestCase {
            description: "an unknown selector is refused without failing the request",
            person: AdhocPerson::Developer,
            select: "model:invoices",
            exclude: "",
            limit: None,
            expected_outcome: Ok((0, &[], None, Some("unknown selector name 'model:invoices'"))),
        },
        SelectionRequestTestCase {
            description: "a person without view on the target is refused",
            person: AdhocPerson::Stranger,
            select: "model:orders",
            exclude: "",
            limit: None,
            expected_outcome: Err(403),
        },
    ];
    for backend in [Backend::Sqlite, Backend::Postgres] {
        run(async {
            let Some(fixture) = open_test_store(backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let context = AuditContext {
                actor: "local-control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let (location_id, _, people) = adhoc_fixture(store, &context).await;
            for test_case in &test_cases {
                let query = SelectionQuery {
                    select: test_case.select.into(),
                    exclude: test_case.exclude.into(),
                    offset: None,
                    limit: test_case.limit,
                };
                let evaluated = evaluate_selection(
                    store,
                    (&ExactKeyPlanner, (&context, people.of(test_case.person))),
                    (location_id, "development", &query),
                    DefaultRole::None,
                )
                .await;
                assert_eq!(
                    evaluated
                        .map(|evaluation| selection_outcome(&evaluation))
                        .map_err(|error| launch_outcome(Err(error)).err()),
                    test_case
                        .expected_outcome
                        .map(owned_selection)
                        .map_err(Some),
                    "{backend:?}: {}",
                    test_case.description
                );
            }
        });
    }
}

#[test]
fn given_a_target_when_people_read_its_selector_values_then_they_describe_its_current_publication()
{
    let test_cases = [SelectionRequestTestCase {
        description: "a viewer reads each kind's node count; a target without a publication has none",
        person: AdhocPerson::Developer,
        select: "",
        exclude: "",
        limit: None,
        expected_outcome: Ok((3, &["model:2", "seed:1", "source:3"], None, None)),
    }];
    for test_case in test_cases {
        for backend in [Backend::Sqlite, Backend::Postgres] {
            run(async {
                let Some(fixture) = open_test_store(backend).await else {
                    return;
                };
                let store = &fixture.store;
                migrate_store(store).await.expect("migrate");
                let context = AuditContext {
                    actor: "local-control-store-owner".into(),
                    granting_role: "control-store-owner".into(),
                    request_id: Uuid::now_v7(),
                    ip_address: None,
                };
                let (location_id, _, people) = adhoc_fixture(store, &context).await;
                let unpublished = create_location(store, "customers", "test-owner")
                    .await
                    .expect("location");
                let credentials = (&context, people.of(test_case.person));
                let read = |location| {
                    read_selection_vocabulary(
                        store,
                        (&ExactKeyPlanner, credentials),
                        (location, "development"),
                        DefaultRole::None,
                    )
                };
                let published = read(location_id).await.expect("vocabulary");
                let empty = read(unpublished).await.expect("empty vocabulary");
                let kinds: Vec<String> = published
                    .kinds
                    .iter()
                    .map(|VocabularyEntry { value, count }| format!("{value}:{count}"))
                    .collect();
                assert_eq!(
                    (
                        Ok((published.kinds.len() as u64, kinds)),
                        empty.snapshot_id,
                        empty.kinds.len()
                    ),
                    (
                        test_case
                            .expected_outcome
                            .map(|expected| (expected.0, owned_selection(expected).1)),
                        None,
                        0
                    ),
                    "{backend:?}: {}",
                    test_case.description
                );
            });
        }
    }
}
