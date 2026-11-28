use crate::yaml_entries::_helpers::freshness::freshness_minutes;
use crate::yaml_entries::tests::helpers::{
    dlt_group, loaded_sources, map, sources, sources_outcome, text,
};
use crate::yaml_entries::tests::test_types::{
    FreshnessDurationTestCase, Node, SourcesDocumentTestCase, lines, parse_error,
};

#[test]
fn given_sources_documents_when_parsing_then_matches_python_sources_and_errors() {
    let test_cases = [
        SourcesDocumentTestCase {
            description: "a dlt_sources group is a removed integration",
            document: dlt_group(vec![
                ("type", text("sql_database")),
                (
                    "resources",
                    Node::List(vec![map(vec![("name", text("orders"))])]),
                ),
            ]),
            expected_sources: parse_error(
                "/project/sources/shop.yml declares dlt_sources, a dlt integration, which \
                 SQLBuild no longer supports; declare each source under `sources:` with \
                 `managed: true` and load it with a Python `@loader` named after the source \
                 instead",
            ),
        },
        SourcesDocumentTestCase {
            description: "an ingestr block on a source is a removed integration",
            document: sources(vec![
                ("name", text("orders")),
                (
                    "ingestr",
                    map(vec![("source_table", text("public.orders"))]),
                ),
            ]),
            expected_sources: parse_error(
                "/project/sources/shop.yml source 'orders' declares an ingestr integration, \
                 which SQLBuild no longer supports; declare the source with `managed: true` and \
                 load it with a Python `@loader` named after the source instead",
            ),
        },
        SourcesDocumentTestCase {
            description: "final reads a table through its deduplicating form",
            document: sources(vec![
                ("name", text("customers")),
                ("schema", text("raw")),
                ("final", Node::Bool(true)),
            ]),
            expected_sources: lines(&["customers loader=- managed=false schema=raw table=- final"]),
        },
        SourcesDocumentTestCase {
            description: "final false is a plain read",
            document: sources(vec![
                ("name", text("customers")),
                ("final", Node::Bool(false)),
            ]),
            expected_sources: lines(&["customers loader=- managed=false schema=- table=-"]),
        },
        SourcesDocumentTestCase {
            description: "final on an expression source",
            document: sources(vec![
                ("name", text("customers")),
                ("expression", text("SELECT 1 AS id")),
                ("final", Node::Bool(true)),
            ]),
            expected_sources: parse_error(
                "/project/sources/shop.yml source 'customers' cannot define final with expression",
            ),
        },
        SourcesDocumentTestCase {
            description: "a loading setting on a source a loader fills",
            document: sources(vec![
                ("name", text("orders")),
                ("managed", Node::Bool(true)),
                ("write_strategy", text("merge")),
            ]),
            expected_sources: parse_error(
                "/project/sources/shop.yml source 'orders' is filled by @loader 'orders', so it \
                 can't set write_strategy; set write_strategy= on @loader, which takes SQLLoad's \
                 table options",
            ),
        },
        SourcesDocumentTestCase {
            description: "a write strategy on an unmanaged source",
            document: sources(vec![
                ("name", text("orders")),
                ("write_strategy", text("append")),
            ]),
            expected_sources: parse_error(
                "/project/sources/shop.yml source 'orders' can't set write_strategy; a source that \
                 isn't loaded has no loading settings, and the @loader or load that fills one sets \
                 them",
            ),
        },
        SourcesDocumentTestCase {
            description: "the removed loader key",
            document: sources(vec![
                ("name", text("orders")),
                ("loader", text("load_orders")),
            ]),
            expected_sources: parse_error(
                "/project/sources/shop.yml source 'loader' is not supported; use managed: true \
                 and name the terminal loader after the source",
            ),
        },
        SourcesDocumentTestCase {
            description: "freshness type without column or query",
            document: sources(vec![
                ("name", text("orders")),
                ("freshness", map(vec![("type", text("timestamp"))])),
            ]),
            expected_sources: parse_error(
                "/project/sources/shop.yml source freshness type and filter require column or query",
            ),
        },
        SourcesDocumentTestCase {
            description: "a load fills the sources named after it, and no others",
            document: loaded_sources(
                vec![
                    ("name", text("shop")),
                    ("kind", text("postgres")),
                    ("connection", text("shop_db")),
                ],
                vec![
                    vec![
                        ("name", text("shop__orders")),
                        ("description", text("Orders")),
                    ],
                    vec![("name", text("shopping__carts"))],
                    vec![("name", text("shop__"))],
                ],
            ),
            expected_sources: lines(&[
                "load shop",
                "shop__orders loader=- managed=false schema=- table=- load=shop",
                "shopping__carts loader=- managed=false schema=- table=-",
                "shop__ loader=- managed=false schema=- table=-",
            ]),
        },
        SourcesDocumentTestCase {
            description: "a source a load fills can't set what the load decides",
            document: loaded_sources(
                vec![("name", text("shop"))],
                vec![vec![
                    ("name", text("shop__orders")),
                    ("write_strategy", text("merge")),
                ]],
            ),
            expected_sources: parse_error(
                "/project/sources/shop.yml source 'shop__orders' is filled by the load 'shop', \
                 so it can't set write_strategy; set it in the load, with SQLLoad's keys, under \
                 `load` or `load.tables`",
            ),
        },
        SourcesDocumentTestCase {
            description: "a source a load fills takes its schema from the load",
            document: loaded_sources(
                vec![("name", text("shop"))],
                vec![vec![
                    ("name", text("shop__orders")),
                    ("schema", text("raw")),
                ]],
            ),
            expected_sources: parse_error(
                "/project/sources/shop.yml source 'shop__orders' is filled by the load 'shop', \
                 so it can't set schema; the load writes into the target's loader_schema, or its \
                 own destination_schema",
            ),
        },
        SourcesDocumentTestCase {
            description: "a load needs a name",
            document: loaded_sources(vec![("kind", text("postgres"))], Vec::new()),
            expected_sources: parse_error(
                "/project/sources/shop.yml load must define non-empty string 'name'",
            ),
        },
        SourcesDocumentTestCase {
            description: "a load is a mapping",
            document: map(vec![("load", text("shop"))]),
            expected_sources: parse_error(
                "/project/sources/shop.yml load must be a mapping of SQLLoad source settings \
                 with a name",
            ),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            sources_outcome(&test_case.document),
            test_case.expected_sources,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_freshness_durations_when_measuring_then_matches_python_duration_rules() {
    let test_cases = [
        FreshnessDurationTestCase {
            description: "minutes",
            text: "15m",
            expected_minutes: Some(15),
        },
        FreshnessDurationTestCase {
            description: "hours",
            text: "2h",
            expected_minutes: Some(120),
        },
        FreshnessDurationTestCase {
            description: "days",
            text: "1d",
            expected_minutes: Some(1_440),
        },
        FreshnessDurationTestCase {
            description: "two units",
            text: "1d2h",
            expected_minutes: None,
        },
        FreshnessDurationTestCase {
            description: "seconds",
            text: "30s",
            expected_minutes: None,
        },
        FreshnessDurationTestCase {
            description: "zero",
            text: "0m",
            expected_minutes: None,
        },
        FreshnessDurationTestCase {
            description: "non-ASCII decimal digits",
            text: "\u{0661}\u{0665}m",
            expected_minutes: Some(15),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            freshness_minutes(test_case.text),
            test_case.expected_minutes,
            "{}",
            test_case.description
        );
    }
}
