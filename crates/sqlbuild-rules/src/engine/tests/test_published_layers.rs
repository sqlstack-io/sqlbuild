use crate::tests::helpers::install_test_identity;
use serde_json::json;

use crate::engine::tests::helpers::published_layer_faults;
use crate::engine::tests::test_types::PublishedLayerTestCase;

#[test]
fn given_published_models_when_evaluating_layer_rules_then_reads_and_shapes_match()
-> Result<(), String> {
    install_test_identity();
    let test_cases = [
        PublishedLayerTestCase {
            description: "a consumer variant reads its own domain's published model",
            code: "SQBRGRAPH103",
            models: &[
                ("sales__mart__orders", &["sales__int_clean__orders"]),
                ("sales__mart__orders__finance", &["sales__mart__orders"]),
            ],
            options: json!({}),
            expected_faults: &[(
                "sql/models/sales/mart/sales__mart__orders__finance.sql",
                "sales__mart__orders__finance reads sales__mart__orders; published models must not read published models of their own domain",
            )],
        },
        PublishedLayerTestCase {
            description: "the finding gives the intermediate move command",
            code: "SQBRGRAPH103",
            models: &[("sales__mart__orders__finance", &["sales__mart__orders"])],
            options: json!({}),
            expected_faults: &[(
                "sql/models/sales/mart/sales__mart__orders__finance.sql",
                "sqb mv sales__mart__orders__finance sql/models/sales/intermediate/enriched/sales__int_enriched__orders__finance.sql",
            )],
        },
        PublishedLayerTestCase {
            description: "a view of its own entity may read the table",
            code: "SQBRGRAPH103",
            models: &[
                ("sales__mart_v__orders", &["sales__mart__orders"]),
                (
                    "sales__mart_v__orders__csv_export",
                    &["sales__mart__orders"],
                ),
            ],
            options: json!({}),
            expected_faults: &[],
        },
        PublishedLayerTestCase {
            description: "a qualified view may read the table with its qualifier",
            code: "SQBRGRAPH103",
            models: &[("sales__mart_v__orders__fra", &["sales__mart__orders__fra"])],
            options: json!({}),
            expected_faults: &[],
        },
        PublishedLayerTestCase {
            description: "a qualified view may not read the table with another qualifier",
            code: "SQBRGRAPH103",
            models: &[("sales__mart_v__orders__fra", &["sales__mart__orders__deu"])],
            options: json!({}),
            expected_faults: &[(
                "sql/models/sales/mart/sales__mart_v__orders__fra.sql",
                "sales__mart_v__orders__fra reads sales__mart__orders__deu",
            )],
        },
        PublishedLayerTestCase {
            description: "a view of another entity may not",
            code: "SQBRGRAPH103",
            models: &[("sales__mart_v__customers", &["sales__mart__orders"])],
            options: json!({}),
            expected_faults: &[(
                "sql/models/sales/mart/sales__mart_v__customers.sql",
                "sales__mart_v__customers reads sales__mart__orders",
            )],
        },
        PublishedLayerTestCase {
            description: "reading another domain's published model is allowed",
            code: "SQBRGRAPH103",
            models: &[("finance__mart__revenue", &["sales__mart__orders"])],
            options: json!({}),
            expected_faults: &[],
        },
        PublishedLayerTestCase {
            description: "names outside the grammar are not checked",
            code: "SQBRGRAPH103",
            models: &[("orders_report", &["orders_summary"])],
            options: json!({}),
            expected_faults: &[],
        },
        PublishedLayerTestCase {
            description: "configured published layers replace the defaults",
            code: "SQBRGRAPH103",
            models: &[("sales__mart__orders__finance", &["sales__mart__orders"])],
            options: json!({"published_layers": ["mart_v"]}),
            expected_faults: &[],
        },
        PublishedLayerTestCase {
            description: "a qualified variant in an entity-shaped domain",
            code: "SQBRPROJECT107",
            models: &[
                ("sales__mart__orders", &[]),
                ("sales__mart__customers", &[]),
                ("sales__mart__orders__finance", &[]),
            ],
            options: json!({}),
            expected_faults: &[(
                "sql/models/sales/mart/sales__mart__orders__finance.sql",
                "domain sales publishes 2 models as <domain>__<layer>__<entity> and 1 as <domain>__<layer>__<entity>__<qualifier>; sales__mart__orders__finance does not fit the majority shape <domain>__<layer>__<entity>",
            )],
        },
        PublishedLayerTestCase {
            description: "an unqualified model in a qualified domain gets a rename command",
            code: "SQBRPROJECT107",
            models: &[
                ("sales__mart__orders__fra", &[]),
                ("sales__mart__orders__deu", &[]),
                ("sales__mart__customers", &[]),
            ],
            options: json!({}),
            expected_faults: &[(
                "sql/models/sales/mart/sales__mart__customers.sql",
                "sqb rename sales__mart__customers sales__mart__customers__core",
            )],
        },
        PublishedLayerTestCase {
            description: "an allowlisted qualifier does not count towards the shape",
            code: "SQBRPROJECT107",
            models: &[
                ("sales__mart__orders", &[]),
                ("sales__mart_v__orders__csv_export", &[]),
                ("finance__mart__revenue", &[]),
                ("finance__mart__revenue__fra", &[]),
            ],
            options: json!({"allowed_qualifiers": [
                {"qualifier": "csv_export", "reason": "File export views of a published entity."},
                {"domain": "finance", "qualifier": "fra", "reason": "Published once per country."}
            ]}),
            expected_faults: &[],
        },
        PublishedLayerTestCase {
            description: "a domain-scoped allowlist entry does not cover other domains",
            code: "SQBRPROJECT107",
            models: &[
                ("sales__mart__orders", &[]),
                ("sales__mart__orders__fra", &[]),
            ],
            options: json!({"allowed_qualifiers": [
                {"domain": "finance", "qualifier": "fra", "reason": "Published once per country."}
            ]}),
            expected_faults: &[(
                "sql/models/sales/mart/sales__mart__orders__fra.sql",
                "allowed_qualifiers = [{ domain = \"sales\", qualifier = \"fra\"",
            )],
        },
        PublishedLayerTestCase {
            description: "one qualifier glob replaces a literal entry per country",
            code: "SQBRPROJECT107",
            models: &[
                ("sales__mart__orders", &[]),
                ("sales__mart__customers", &[]),
                ("sales__mart__orders__fra", &[]),
                ("sales__mart__orders__deu", &[]),
                ("sales__mart__orders__ita", &[]),
                ("sales__mart__returns__fra_snapshots", &[]),
            ],
            options: json!({"allowed_qualifiers": [
                {"domain": "sales", "qualifier": "[a-z][a-z][a-z]", "reason": "Published once per country."},
                {"domain": "sales", "qualifier": "[a-z][a-z][a-z]_snapshots", "reason": "Daily snapshots per country."}
            ]}),
            expected_faults: &[],
        },
        PublishedLayerTestCase {
            description: "a qualifier glob is case-sensitive and leaves other qualifiers flagged",
            code: "SQBRPROJECT107",
            models: &[
                ("sales__mart__orders", &[]),
                ("sales__mart__customers", &[]),
                ("sales__mart__orders__fra", &[]),
                ("sales__mart__orders__finance", &[]),
            ],
            options: json!({"allowed_qualifiers": [
                {"qualifier": "[A-Z][A-Z][A-Z]", "reason": "Published once per country."},
                {"domain": "finance", "qualifier": "[a-z][a-z][a-z]", "reason": "Per country."}
            ]}),
            expected_faults: &[
                (
                    "sql/models/sales/mart/sales__mart__orders__finance.sql",
                    "domain sales publishes 2 models as <domain>__<layer>__<entity> and 2 as",
                ),
                (
                    "sql/models/sales/mart/sales__mart__orders__fra.sql",
                    "sales__mart__orders__fra does not fit the majority shape",
                ),
            ],
        },
        PublishedLayerTestCase {
            description: "a paths star matches across folder separators",
            code: "SQBRPROJECT107",
            models: &[
                ("sales__mart__orders", &[]),
                ("sales__mart__orders__fra", &[]),
                ("sales__mart__orders__deu", &[]),
            ],
            options: json!({"allowed_qualifiers": [
                {"paths": ["sql/models/sales/*"], "reason": "Published once per country."}
            ]}),
            expected_faults: &[],
        },
        PublishedLayerTestCase {
            description: "a paths entry leaves out the published models in its folder",
            code: "SQBRPROJECT107",
            models: &[
                ("sales__mart__orders", &[]),
                ("sales__mart__orders__fra", &[]),
                ("sales__mart__orders__deu", &[]),
                ("finance__mart__revenue", &[]),
                ("finance__mart__revenue__fra", &[]),
            ],
            options: json!({"allowed_qualifiers": [
                {"paths": ["sql/models/sales/mart/*"], "reason": "Published once per country."}
            ]}),
            expected_faults: &[(
                "sql/models/finance/mart/finance__mart__revenue__fra.sql",
                "domain finance publishes 1 model as <domain>__<layer>__<entity> and 1 as",
            )],
        },
    ];

    for test_case in test_cases {
        let found: Vec<(String, String)> = published_layer_faults(&test_case)?;

        assert_eq!(
            found.len(),
            test_case.expected_faults.len(),
            "{}: {found:?}",
            test_case.description
        );
        for ((path, text), (expected_path, fragment)) in found.iter().zip(test_case.expected_faults)
        {
            assert_eq!(path, expected_path, "{}", test_case.description);
            assert!(text.contains(fragment), "{}: {text}", test_case.description);
        }
    }
    Ok(())
}
