//! A location builds the same warehouse state from a served copy as a fixture stores.

use sqlbuild_state::warehouse_state::main::warehouse_state::warehouse_state;
use sqlbuild_state::warehouse_state::models::{StateLocation, WarehouseState, WarehouseStateParts};

use crate::warehouse_metadata::main::location_reads::preview_state_parts::preview_state_parts;
use crate::warehouse_metadata::models::PreviewCopy;
use crate::warehouse_metadata::tests::helpers::{served_copy, stored_parts};
use crate::warehouse_metadata::tests::test_types::{PreviewStateTestCase, StateAnswers};

#[test]
fn given_a_served_copy_and_a_stored_fixture_when_built_then_the_states_and_their_answers_match() {
    let test_cases = [PreviewStateTestCase {
        description: "one fact of every kind",
        served: served_copy,
        stored: stored_parts,
        expected_answers: || {
            (
                Some("41".into()),
                vec!["run-1".into()],
                1,
                Some("mirror-7".into()),
            )
        },
    }];
    let shop = StateLocation {
        database: Some("warehouse".into()),
        schema: "shop".into(),
    };
    let answers = |state: &WarehouseState| -> StateAnswers {
        (
            state.target_max("orders").map(str::to_owned),
            state
                .latest_fingerprints(&shop)
                .into_iter()
                .map(|fingerprint| fingerprint.run_id.clone())
                .collect(),
            state.consumption_rows(&shop).len(),
            state.version().map(str::to_owned),
        )
    };
    for test_case in test_cases {
        let copy: Option<PreviewCopy> = serde_json::from_value((test_case.served)()).ok();
        let fixture: Option<WarehouseStateParts> =
            serde_json::from_value((test_case.stored)()).ok();
        let from_copy: Option<WarehouseState> =
            copy.map(|copy| warehouse_state(preview_state_parts(copy, true)));
        let from_fixture: Option<WarehouseState> = fixture.map(warehouse_state);
        assert_eq!(
            (
                from_copy.as_ref().map(WarehouseState::parts),
                from_copy.as_ref().map(answers),
            ),
            (
                from_fixture.as_ref().map(WarehouseState::parts),
                Some((test_case.expected_answers)()),
            ),
            "{}",
            test_case.description
        );
    }
}
