use crate::lineage::models::FastLineageModel;
use crate::lineage::tests::helpers::{deep_union_outcome_lines, outcome_lines, strings};
use crate::lineage::tests::test_types::{
    DeepUnionTestCase, ParsedLineageTestCase, StarExpansionTestCase,
};

#[test]
fn given_models_without_compact_facts_when_building_then_matches_python_lineage() {
    let test_cases = [
        ParsedLineageTestCase {
            description: "joins, aggregation, cast, constant and expression columns",
            sql: "SELECT o.order_id, c.name AS customer_name, SUM(o.amount) AS total, \
                  CAST(o.amount AS BIGINT) AS amount_big, 1 AS one, o.amount + c.customer_id AS mix \
                  FROM __ref(\"orders\") o JOIN __ref(\"customers\") c ON o.customer_id = c.customer_id \
                  GROUP BY 1, 2",
            dialect: None,
            inferred_columns: &[],
            expected_status: "built",
            expected_lines: &[
                "order_id direct high [model:orders.order_id]",
                "customer_name direct high [model:customers.name]",
                "total aggregation high [model:orders.amount]",
                "amount_big cast high [model:orders.amount]",
                "one constant high []",
                "mix expression high [model:orders.amount model:customers.customer_id]",
            ],
            expected_has_star: false,
            expected_detail: None,
        },
        ParsedLineageTestCase {
            description: "unqualified columns of one resource and an unknown qualifier",
            sql: "SELECT order_id, amount * 2 AS doubled, missing.thing FROM __ref(\"orders\")",
            dialect: None,
            inferred_columns: &[],
            expected_status: "built",
            expected_lines: &[
                "order_id direct medium [model:orders.order_id]",
                "doubled expression medium [model:orders.amount]",
                "thing constant high []",
            ],
            expected_has_star: false,
            expected_detail: None,
        },
        ParsedLineageTestCase {
            description: "a root star expands known columns after explicit outputs",
            sql: "SELECT *, amount AS amt FROM __ref(\"orders\")",
            dialect: None,
            inferred_columns: &[],
            expected_status: "built",
            expected_lines: &[
                "amt direct medium [model:orders.amount]",
                "order_id star medium [model:orders.order_id]",
                "customer_id star medium [model:orders.customer_id]",
                "amount star medium [model:orders.amount]",
            ],
            expected_has_star: true,
            expected_detail: None,
        },
        ParsedLineageTestCase {
            description: "a qualified star expands every referenced resource",
            sql: "SELECT o.* FROM __ref(\"orders\") o JOIN __seed(\"regions\") r ON TRUE",
            dialect: None,
            inferred_columns: &[],
            expected_status: "built",
            expected_lines: &[
                "order_id star medium [model:orders.order_id]",
                "customer_id star medium [model:orders.customer_id]",
                "amount star medium [model:orders.amount]",
                "region_id star medium [seed:regions.region_id]",
                "label star medium [seed:regions.label]",
            ],
            expected_has_star: true,
            expected_detail: None,
        },
        ParsedLineageTestCase {
            description: "a three-branch union merges branch sources",
            sql: "SELECT order_id FROM __ref(\"orders\") UNION ALL SELECT customer_id \
                  FROM __ref(\"customers\") UNION ALL SELECT region_id + 1 FROM __seed(\"regions\")",
            dialect: None,
            inferred_columns: &[],
            expected_status: "built",
            expected_lines: &[
                "order_id expression medium [model:orders.order_id model:customers.customer_id \
                 seed:regions.region_id]",
            ],
            expected_has_star: false,
            expected_detail: None,
        },
        ParsedLineageTestCase {
            description: "a union with a star branch takes inferred names",
            sql: "SELECT order_id, amount FROM __ref(\"orders\") UNION SELECT * FROM __ref(\"customers\")",
            dialect: None,
            inferred_columns: &["first", "second"],
            expected_status: "built",
            expected_lines: &[
                "first direct medium [model:orders.order_id]",
                "second direct medium [model:orders.amount]",
                "order_id star medium [model:orders.order_id]",
                "customer_id star medium [model:orders.customer_id]",
                "amount star medium [model:orders.amount]",
                "name star medium [model:customers.name]",
            ],
            expected_has_star: true,
            expected_detail: None,
        },
        ParsedLineageTestCase {
            description: "a CTE collapses onto its referenced model",
            sql: "WITH base AS (SELECT order_id, amount FROM __ref(\"orders\")) \
                  SELECT order_id, amount FROM base",
            dialect: None,
            inferred_columns: &[],
            expected_status: "built",
            expected_lines: &[
                "order_id direct medium [model:orders.order_id]",
                "amount direct medium [model:orders.amount]",
            ],
            expected_has_star: false,
            expected_detail: None,
        },
        ParsedLineageTestCase {
            description: "unnamed projections take inferred names",
            sql: "SELECT 1 + 1, COUNT(*) FROM __ref(\"orders\")",
            dialect: None,
            inferred_columns: &["a", "b"],
            expected_status: "built",
            expected_lines: &["a constant high []", "b aggregation unknown []"],
            expected_has_star: false,
            expected_detail: None,
        },
        ParsedLineageTestCase {
            description: "quoted and case-varied identifiers keep their spelling",
            sql: "SELECT \"Order_ID\", O.Amount FROM __ref(\"orders\") AS O",
            dialect: None,
            inferred_columns: &[],
            expected_status: "built",
            expected_lines: &[
                "Order_ID direct medium [model:orders.Order_ID]",
                "Amount direct high [model:orders.Amount]",
            ],
            expected_has_star: false,
            expected_detail: None,
        },
        ParsedLineageTestCase {
            description: "CASE branches are tuples Python's payload walk never enters",
            sql: "SELECT CASE WHEN amount >= 9 THEN 'HIGH' ELSE 'LOW' END AS tier, \
                  CASE order_id WHEN 1 THEN amount END AS picked, \
                  COALESCE(amount, order_id) AS filled, amount BETWEEN 1 AND order_id AS ranged, \
                  order_id IN (1, customer_id) AS listed, \
                  ROW_NUMBER() OVER (PARTITION BY customer_id ORDER BY amount) AS rn, \
                  EXTRACT(YEAR FROM amount) AS yr, order_id || '-' || amount AS joined \
                  FROM __ref(\"orders\")",
            dialect: None,
            inferred_columns: &[],
            expected_status: "built",
            expected_lines: &[
                "tier constant high []",
                "picked expression medium [model:orders.order_id]",
                "filled expression medium [model:orders.amount model:orders.order_id]",
                "ranged expression medium [model:orders.amount model:orders.order_id]",
                "listed expression medium [model:orders.order_id model:orders.customer_id]",
                "rn expression medium [model:orders.customer_id model:orders.amount]",
                "yr expression medium [model:orders.amount]",
                "joined expression medium [model:orders.order_id model:orders.amount]",
            ],
            expected_has_star: false,
            expected_detail: None,
        },
        ParsedLineageTestCase {
            description: "a top-level EXCEPT has no fast lineage",
            dialect: None,
            sql: "SELECT order_id FROM __ref(\"orders\") EXCEPT SELECT customer_id FROM __ref(\"customers\")",
            inferred_columns: &[],
            expected_status: "omitted",
            expected_lines: &[],
            expected_has_star: false,
            expected_detail: None,
        },
        ParsedLineageTestCase {
            description: "an unterminated query reports where the dialect's parser stops",
            dialect: Some("duckdb"),
            sql: "SELECT order_id FROM (",
            inferred_columns: &[],
            expected_status: "unparsed",
            expected_lines: &[],
            expected_has_star: false,
            expected_detail: Some(
                "the duckdb parser cannot read Syntax error at or near end of input at line 1, column 23",
            ),
        },
        ParsedLineageTestCase {
            description: "two statements report the statement count",
            dialect: None,
            sql: "SELECT 1; SELECT 2",
            inferred_columns: &[],
            expected_status: "unparsed",
            expected_lines: &[],
            expected_has_star: false,
            expected_detail: Some("Expected 1 statement, found 2"),
        },
        ParsedLineageTestCase {
            description: "truncated T-SQL reports its parse error",
            sql: "SELECT IF(region > 1, re",
            dialect: Some("tsql"),
            inferred_columns: &[],
            expected_status: "unparsed",
            expected_lines: &[],
            expected_has_star: false,
            expected_detail: Some(
                "Parse error at line 1, column 25: Expected ')' to close the IF condition",
            ),
        },
        ParsedLineageTestCase {
            description: "a dialect outside the old parser build is parsed natively",
            dialect: Some("mysql"),
            sql: "SELECT 1",
            inferred_columns: &[],
            expected_status: "built",
            expected_lines: &["1 constant high []"],
            expected_has_star: false,
            expected_detail: None,
        },
        ParsedLineageTestCase {
            description: "an unknown dialect raises Python's error",
            dialect: Some("not-a-dialect"),
            sql: "SELECT 1",
            inferred_columns: &[],
            expected_status: "unknown_dialect",
            expected_lines: &[],
            expected_has_star: false,
            expected_detail: Some("not-a-dialect"),
        },
    ];
    for test_case in test_cases {
        let outcomes: Vec<(&str, Vec<String>, bool, Option<String>)> = outcome_lines(
            test_case.dialect,
            FastLineageModel::Parse {
                query_sql: test_case.sql.to_owned(),
                inferred_columns: strings(test_case.inferred_columns),
                placeholders: Vec::new(),
            },
        );
        assert_eq!(
            outcomes,
            vec![(
                test_case.expected_status,
                strings(test_case.expected_lines),
                test_case.expected_has_star,
                test_case.expected_detail.map(str::to_owned),
            )],
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_compact_facts_with_unresolved_star_when_building_then_appends_unseen_columns() {
    let test_cases = [
        StarExpansionTestCase {
            description: "existing outputs and repeated references are skipped",
            sql: "SELECT *, 1 AS amount FROM __ref(\"orders\") JOIN __ref(\"orders\") USING (order_id) \
                  JOIN __seed(\"regions\") USING (region_id)",
            existing_columns: &["amount"],
            expected_lines: &[
                "order_id star medium [model:orders.order_id]",
                "customer_id star medium [model:orders.customer_id]",
                "region_id star medium [seed:regions.region_id]",
                "label star medium [seed:regions.label]",
            ],
        },
        StarExpansionTestCase {
            description: "resources without known columns add nothing",
            sql: "SELECT * FROM __ref(\"empty\") JOIN __source(\"raw\") ON TRUE",
            existing_columns: &[],
            expected_lines: &[],
        },
    ];
    for test_case in test_cases {
        let outcomes: Vec<(&str, Vec<String>, bool, Option<String>)> = outcome_lines(
            Some("not-a-dialect"),
            FastLineageModel::StarExpansion {
                query_sql: test_case.sql.to_owned(),
                existing_columns: strings(test_case.existing_columns),
            },
        );
        assert_eq!(
            outcomes,
            vec![("star", strings(test_case.expected_lines), true, None)],
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_deep_union_chain_when_building_from_small_stack_then_runs_on_the_analysis_pool() {
    let test_cases = [
        DeepUnionTestCase {
            description: "the deepest chain the parser guard accepts",
            branches: 512,
            stack_bytes: 256 * 1024,
            expected_status: "built",
            expected_lines: &[
                "order_id direct medium [model:orders.order_id]",
                "amount direct medium [model:orders.amount]",
            ],
            expected_detail: None,
        },
        DeepUnionTestCase {
            description: "one branch past the parser guard reports its parse error",
            branches: 513,
            stack_bytes: 256 * 1024,
            expected_status: "unparsed",
            expected_lines: &[],
            expected_detail: Some(
                "Parse error at line 0, column 0: E_GUARD_AST_DEPTH_EXCEEDED: value 513 exceeds configured limit 512",
            ),
        },
    ];
    for test_case in test_cases {
        let outcomes: Vec<(&str, Vec<String>, bool, Option<String>)> =
            deep_union_outcome_lines(test_case.branches, test_case.stack_bytes);
        assert_eq!(
            outcomes,
            vec![(
                test_case.expected_status,
                strings(test_case.expected_lines),
                false,
                test_case.expected_detail.map(str::to_owned),
            )],
            "{}",
            test_case.description
        );
    }
}
