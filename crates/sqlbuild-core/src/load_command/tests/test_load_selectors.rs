use crate::load_command::main::load_selectors::load_selectors;
use crate::load_command::models::LoadKinds;
use crate::load_command::tests::test_types::LoadSelectorsTestCase;

const EVERYTHING: LoadKinds = LoadKinds {
    loads: true,
    loaders: true,
    sources: true,
};

#[test]
fn given_load_selectors_when_narrowing_them_then_only_loading_nodes_are_built() {
    let test_cases = [
        LoadSelectorsTestCase {
            description: "no selectors load every load, loader and source",
            selectors: &[],
            kinds: EVERYTHING,
            expected_selectors: &["kind:loader", "kind:load", "kind:source"],
        },
        LoadSelectorsTestCase {
            description: "a name loads only what it is; + brings in what it reads",
            selectors: &["raw_orders", "+raw_events"],
            kinds: EVERYTHING,
            expected_selectors: &[
                "raw_orders,kind:loader",
                "raw_orders,kind:load",
                "raw_orders,kind:source",
                "+raw_events,kind:loader",
                "+raw_events,kind:load",
                "+raw_events,kind:source",
            ],
        },
        LoadSelectorsTestCase {
            description: "kinds the project has none of are left out",
            selectors: &["tag:daily"],
            kinds: LoadKinds {
                loads: false,
                loaders: true,
                sources: true,
            },
            expected_selectors: &["tag:daily,kind:loader", "tag:daily,kind:source"],
        },
        LoadSelectorsTestCase {
            description: "a project with sources but no load or loader loads nothing",
            selectors: &[],
            kinds: LoadKinds {
                loads: false,
                loaders: false,
                sources: true,
            },
            expected_selectors: &[],
        },
    ];
    for test_case in test_cases {
        let selectors: Vec<String> = test_case
            .selectors
            .iter()
            .map(|selector| (*selector).to_owned())
            .collect();
        assert_eq!(
            load_selectors(&selectors, test_case.kinds),
            test_case.expected_selectors,
            "{}",
            test_case.description
        );
    }
}
