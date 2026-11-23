//! A tick an earlier release stored in a form that no longer decodes loses only its own details.

use chrono::{TimeDelta, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::DefaultRole;
use crate::credentials::tests::role_helpers::{credentials, role_fixture};
use crate::declarations::main::list_person_triggers::list_person_triggers;
use crate::declarations::main::list_triggers::list_triggers;
use crate::scheduler::tests::contracts::helpers::{acquire_lease, execute_with};
use crate::sensors::constants::UNREADABLE_TICK;
use crate::sensors::main::begin_sensor_evaluation::begin_sensor_evaluation;
use crate::sensors::main::read_person_sensor::read_person_sensor;
use crate::sensors::tests::helpers::{
    append_events, complete_starting_job, completed_tick, failed_tick, record_tick, snapshot,
};
use crate::sensors::tests::test_types::UnreadableTickTestCase;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::models::Parameter;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_tick_an_earlier_release_stored_unreadable_when_people_read_the_sensor_then_the_page_loads_and_that_tick_says_so()
 {
    let test_cases = [
        UnreadableTickTestCase {
            description: "Sqlite: every phase of a failed tick",
            backend: Backend::Sqlite,
            tick: failed_tick,
            unreadable: "evaluation_id=$3 AND $2 IS NOT NULL",
            expected_ticks: [("failed", true), ("started", false)],
            expected_failing: Some(Some(UNREADABLE_TICK)),
        },
        UnreadableTickTestCase {
            description: "Sqlite: only how the failed tick ended",
            backend: Backend::Sqlite,
            tick: failed_tick,
            unreadable: "evaluation_id=$3 AND phase<>'requested' AND $2 IS NOT NULL",
            expected_ticks: [("failed", true), ("started", false)],
            expected_failing: Some(Some(UNREADABLE_TICK)),
        },
        UnreadableTickTestCase {
            description: "Sqlite: only the request of a failed tick",
            backend: Backend::Sqlite,
            tick: failed_tick,
            unreadable: "evaluation_id=$3 AND phase='requested' AND $2 IS NOT NULL",
            expected_ticks: [("failed", false), ("started", false)],
            expected_failing: Some(None),
        },
        UnreadableTickTestCase {
            description: "Sqlite: a tick that started runs",
            backend: Backend::Sqlite,
            tick: failed_tick,
            unreadable: "evaluation_id=$2 AND $3 IS NOT NULL",
            expected_ticks: [("failed", false), ("started", true)],
            expected_failing: Some(None),
        },
        UnreadableTickTestCase {
            description: "Sqlite: every tick, the newest handled",
            backend: Backend::Sqlite,
            tick: completed_tick,
            unreadable: "evaluation_id IN ($2, $3)",
            expected_ticks: [("handled", true), ("started", true)],
            expected_failing: None,
        },
        UnreadableTickTestCase {
            description: "Postgres: every phase of a failed tick",
            backend: Backend::Postgres,
            tick: failed_tick,
            unreadable: "evaluation_id=$3 AND $2 IS NOT NULL",
            expected_ticks: [("failed", true), ("started", false)],
            expected_failing: Some(Some(UNREADABLE_TICK)),
        },
        UnreadableTickTestCase {
            description: "Postgres: only how the failed tick ended",
            backend: Backend::Postgres,
            tick: failed_tick,
            unreadable: "evaluation_id=$3 AND phase<>'requested' AND $2 IS NOT NULL",
            expected_ticks: [("failed", true), ("started", false)],
            expected_failing: Some(Some(UNREADABLE_TICK)),
        },
        UnreadableTickTestCase {
            description: "Postgres: only the request of a failed tick",
            backend: Backend::Postgres,
            tick: failed_tick,
            unreadable: "evaluation_id=$3 AND phase='requested' AND $2 IS NOT NULL",
            expected_ticks: [("failed", false), ("started", false)],
            expected_failing: Some(None),
        },
        UnreadableTickTestCase {
            description: "Postgres: a tick that started runs",
            backend: Backend::Postgres,
            tick: failed_tick,
            unreadable: "evaluation_id=$2 AND $3 IS NOT NULL",
            expected_ticks: [("failed", false), ("started", true)],
            expected_failing: Some(None),
        },
        UnreadableTickTestCase {
            description: "Postgres: every tick, the newest handled",
            backend: Backend::Postgres,
            tick: completed_tick,
            unreadable: "evaluation_id IN ($2, $3)",
            expected_ticks: [("handled", true), ("started", true)],
            expected_failing: None,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let roles = role_fixture(store).await;
            let location = roles.location_id;
            let mut input = snapshot(location);
            input.generation = 2;
            publish_snapshot(store, &input).await.expect("publish");
            let trigger = list_triggers(store, location, "development")
                .await
                .expect("triggers")
                .remove(0);
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            append_events(store, 0..1, Utc::now()).await;
            let first = begin_sensor_evaluation(store, &lease, (trigger.trigger_id, None))
                .await
                .expect("request")
                .expect("due");
            let runs = complete_starting_job(store, &lease, &first)
                .await
                .expect("started");
            let second =
                record_tick(store, &first, TimeDelta::seconds(1), (test_case.tick)()).await;
            execute_with(
                store,
                &format!(
                    "UPDATE sqb_sensor_evaluation_events SET fact_json=$1 WHERE {}",
                    test_case.unreadable
                ),
                vec![
                    Parameter::Json(json!({"written_by": "an earlier release"})),
                    Parameter::Id(first.evaluation_id),
                    Parameter::Id(second.evaluation_id),
                ],
            )
            .await;
            let who = (&roles.context, credentials(&roles.administrator));
            let state = read_person_sensor(
                store,
                who,
                ((location, "development"), (trigger.trigger_id, 10)),
                DefaultRole::None,
            )
            .await
            .expect("the page loads")
            .expect("sensor");
            let listed =
                list_person_triggers(store, who, (location, "development"), DefaultRole::None)
                    .await
                    .expect("the trigger list loads")
                    .remove(0);
            let ticks: Vec<(&str, bool)> = state
                .evaluations
                .iter()
                .map(|tick| (tick.outcome.as_str(), tick.unreadable.is_some()))
                .collect();
            let failing = state
                .failing
                .as_ref()
                .map(|failing| failing.reason.as_deref());

            assert_eq!(
                (ticks, failing, state.evaluations[1].run_ids.len()),
                (
                    test_case.expected_ticks.to_vec(),
                    test_case.expected_failing,
                    runs,
                ),
                "{}: only that tick loses its details",
                test_case.description
            );
            assert_eq!(
                listed.sensor_failing, state.failing,
                "{}: the list says the same",
                test_case.description
            );
            assert_eq!(
                state.evaluations[0].event_id,
                second.event.as_ref().map(|event| event.id().to_owned()),
                "{}: the tick keeps its event",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
