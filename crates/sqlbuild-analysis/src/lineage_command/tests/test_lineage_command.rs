use sqlbuild_core::cli_text::models::CliStyle;

use crate::lineage_command::main::lineage_stdout::lineage_stdout;
use crate::lineage_command::main::normalize_lineage_target::normalize_lineage_target;
use crate::lineage_command::main::render_lineage_graph::render_lineage_graph;
use crate::lineage_command::main::select_target_lineage::select_target_lineage;
use crate::lineage_command::models::{LineageIndex, LineageKey};
use crate::lineage_command::tests::helpers::orders_index;
use crate::lineage_command::tests::test_types::{ResolveTargetTestCase, TargetLineageTestCase};

#[test]
fn given_targets_when_selecting_lineage_then_stdout_matches() {
    let test_cases = [
        TargetLineageTestCase {
            description: "an upstream tree sorts children by type and name",
            targets: vec!["fact_orders".to_owned()],
            output_format: "tree",
            direction: "upstream",
            depth: None,
            expected_stdout: Ok(
                "\nLineage  model  fact_orders  sql/models/fact_orders.sql  upstream\n\
                \u{251c}\u{2500}\u{2500} model  stg_orders  sql/models/stg_orders.sql\n\
                \u{2502}   \u{2514}\u{2500}\u{2500} source  raw_orders\n\
                \u{2514}\u{2500}\u{2500} seed  waffle_types\n\n",
            ),
        },
        TargetLineageTestCase {
            description: "a depth-limited downstream edge list aligns arrows",
            targets: vec!["raw_orders".to_owned(), "waffle_types".to_owned()],
            output_format: "list",
            direction: "downstream",
            depth: Some(1),
            expected_stdout: Ok("\nmodel:stg_orders  -> model:fact_orders\n\
                seed:waffle_types -> model:fact_orders\n\
                source:raw_orders -> model:stg_orders\n\n"),
        },
        TargetLineageTestCase {
            description: "JSON lists nodes, edges, focus and direction",
            targets: vec!["stg_orders".to_owned()],
            output_format: "json",
            direction: "downstream",
            depth: None,
            expected_stdout: Ok(
                "{\n  \"nodes\": [\n    {\n      \"id\": \"model:fact_orders\",\n      \
                \"name\": \"fact_orders\",\n      \"resource_type\": \"model\",\n      \
                \"relative_path\": \"sql/models/fact_orders.sql\"\n    },\n    {\n      \
                \"id\": \"model:stg_orders\",\n      \"name\": \"stg_orders\",\n      \
                \"resource_type\": \"model\",\n      \"relative_path\": \"sql/models/stg_orders.sql\"\n    \
                }\n  ],\n  \"edges\": [\n    {\n      \"from\": \"model:stg_orders\",\n      \
                \"to\": \"model:fact_orders\"\n    }\n  ],\n  \"focus\": [\n    \
                \"model:stg_orders\"\n  ],\n  \"direction\": \"downstream\"\n}\n",
            ),
        },
        TargetLineageTestCase {
            description: "an unknown target is a user error",
            targets: vec!["stg_orders+".to_owned()],
            output_format: "tree",
            direction: "upstream",
            depth: None,
            expected_stdout: Err("C305"),
        },
    ];

    for test_case in test_cases {
        let index: LineageIndex = orders_index();
        let observed: Result<String, &str> = select_target_lineage(
            &index,
            &test_case.targets,
            (test_case.direction, test_case.depth),
        )
        .map_err(|error| error.code)
        .map(|graph| {
            lineage_stdout(
                render_lineage_graph(
                    &graph,
                    test_case.output_format,
                    CliStyle { use_color: false },
                ),
                None,
            )
            .unwrap_or_default()
        });
        assert_eq!(
            observed,
            test_case.expected_stdout.map(str::to_owned),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_prefixed_target_when_normalizing_then_kind_must_match() {
    let test_cases = [
        ResolveTargetTestCase {
            description: "a matching prefix is stripped",
            target: "model:stg_orders",
            expected_name: Ok("stg_orders"),
        },
        ResolveTargetTestCase {
            description: "a column prefix keeps the column",
            target: "column:stg_orders.order_id",
            expected_name: Ok("stg_orders.order_id"),
        },
        ResolveTargetTestCase {
            description: "an unprefixed target is returned unchanged",
            target: "anything.at_all",
            expected_name: Ok("anything.at_all"),
        },
        ResolveTargetTestCase {
            description: "a mismatched prefix is an unknown target",
            target: "seed:stg_orders",
            expected_name: Err("C305"),
        },
    ];

    for test_case in test_cases {
        let index: LineageIndex = orders_index();
        let lookup = |name: &str| -> Option<LineageKey> {
            index
                .all_keys
                .iter()
                .find(|(candidate, _)| candidate == name)
                .map(|(_, key)| key.clone())
        };
        assert_eq!(
            normalize_lineage_target(test_case.target, &lookup).map_err(|error| error.code),
            test_case.expected_name.map(str::to_owned),
            "{}",
            test_case.description
        );
    }
}
