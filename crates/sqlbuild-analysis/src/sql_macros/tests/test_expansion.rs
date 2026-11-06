use sqlbuild_render::macro_arguments::main::parse_macro_arguments::parse_macro_arguments;
use sqlbuild_render::macro_arguments::models::ArgumentValue;
use sqlbuild_render::macro_calls::main::scan_macro_call_sites::scan_macro_call_sites;

use crate::sql_macros::tests::helpers::expanded_for;
use crate::sql_macros::tests::test_types::{
    AsciiArguments, ExpansionErrorTestCase, ExpansionTestCase, PythonArgumentTestCase,
};

const CENTS_TO_DOLLARS: (&str, &str) = (
    "cents_to_dollars",
    "MACRO (\n  description \"Cents to dollars\",\n  arguments (amount_cents BIGINT, places INT CONST DEFAULT 2),\n  returns DECIMAL(18, 2),\n);\n\nCAST(ROUND(@amount_cents / 100, @places) AS DECIMAL(18, 2))\n",
);
const NET_AMOUNT: (&str, &str) = (
    "net_amount",
    "MACRO (\n  description \"Net amount in dollars\",\n  arguments (gross_cents BIGINT, refund_cents BIGINT),\n  returns DECIMAL(18, 2),\n);\n\n@sql.cents_to_dollars(@gross_cents - COALESCE(@refund_cents, 0))\n",
);
const CLEAN_ORDERS: (&str, &str) = (
    "clean_orders",
    "MACRO (\n  description \"Deduplicated, non-deleted orders\",\n  arguments (src RELATION (order_id BIGINT, updated_at TIMESTAMP, is_deleted BOOLEAN)),\n);\n\nWITH live AS (SELECT * FROM @src WHERE NOT is_deleted),\nranked AS (\n  SELECT *, ROW_NUMBER() OVER (PARTITION BY order_id ORDER BY updated_at DESC) AS row_num\n  FROM live\n),\nlatest AS (SELECT * EXCLUDE (row_num) FROM ranked WHERE row_num = 1)\n",
);
const AUDIT_COLUMNS: (&str, &str) = (
    "audit_columns",
    "MACRO (\n  description \"Standard audit columns\",\n  arguments (src RELATION (created_at TIMESTAMP, updated_at TIMESTAMP)),\n);\n\nSELECT @src.created_at, @src.updated_at, @src.updated_at > @src.created_at AS was_updated\n",
);
const IS_OPEN_STATUS: (&str, &str) = (
    "is_open_status",
    "MACRO (\n  description \"Whether a status is still open\",\n  arguments (status VARCHAR, closed ENUM order_status DEFAULT @enum(\"order_status\").CLOSED),\n  returns BOOLEAN,\n);\n\n@status <> @closed\n",
);
const IN_SUPPORTED_COUNTRY: (&str, &str) = (
    "in_supported_country",
    "MACRO (\n  description \"Whether a country code is supported\",\n  arguments (country VARCHAR, countries LIST VARCHAR DEFAULT [\"GB\", \"FR\"]),\n  returns BOOLEAN,\n);\n\n@country IN @countries\n",
);
const LATEST_PER_KEY: (&str, &str) = (
    "latest_per_key",
    "MACRO (\n  description \"Latest row per key\",\n  arguments (\n    src RELATION (updated_at TIMESTAMP, keys COLUMNS),\n  ),\n);\n\nWITH latest_ranked AS (\n  SELECT @src.*, ROW_NUMBER() OVER (PARTITION BY @src.keys ORDER BY updated_at DESC) AS row_num FROM @src\n),\nlatest_rows AS (SELECT * EXCLUDE (row_num) FROM latest_ranked WHERE row_num = 1)\n",
);
const LATEST_LIVE_ORDERS: (&str, &str) = (
    "latest_live_orders",
    "MACRO (\n  description \"Latest live row per order\",\n  arguments (src RELATION (order_id BIGINT, updated_at TIMESTAMP, is_deleted BOOLEAN)),\n);\n\nWITH c2 AS (SELECT * FROM @src WHERE NOT is_deleted),\nc3 AS (SELECT *, ROW_NUMBER() OVER (PARTITION BY order_id ORDER BY updated_at DESC) AS row_num FROM c2),\nc4 AS (SELECT * EXCLUDE (row_num) FROM c3 WHERE row_num = 1)\n",
);
const PING: (&str, &str) = (
    "ping",
    "MACRO (\n  description \"Ping\",\n  arguments (value BIGINT),\n  returns BIGINT,\n);\n\n@sql.pong(@value)\n",
);
const PONG: (&str, &str) = (
    "pong",
    "MACRO (\n  description \"Pong\",\n  arguments (value BIGINT),\n  returns BIGINT,\n);\n\n@sql.ping(@value)\n",
);

const STATUS_MATCHES: (&str, &str) = (
    "status_matches",
    "MACRO (\n  description \"Whether the status column matches\",\n  arguments (status VARCHAR),\n  returns BOOLEAN,\n);\n\nstatus = @status\n",
);
const ALIASED_COLUMNS: (&str, &str) = (
    "aliased_ids",
    "MACRO (\n  description \"Ids of an aliased relation\",\n  arguments (src RELATION (id BIGINT)),\n);\n\nWITH picked_ids AS (SELECT s.id FROM @src AS s)\n",
);

const PLUS_ONE: (&str, &str) = (
    "plus_one",
    "MACRO (description \"Plus one\", arguments (v BIGINT), returns BIGINT);\n\n@v + 1\n",
);
const ROUND_TO: (&str, &str) = (
    "round_to",
    "MACRO (description \"Rounded\", arguments (v DECIMAL(18, 4), places INT), returns DECIMAL(18, 2));\n\nROUND(@v, @places)\n",
);
const MONTH_OF: (&str, &str) = (
    "month_of",
    "MACRO (description \"Truncated\", arguments (at TIMESTAMP, unit VARCHAR), returns TIMESTAMP);\n\nDATE_TRUNC(@unit, @at)\n",
);
const MONTH_START: (&str, &str) = (
    "month_start",
    "MACRO (description \"Month start\", arguments (at TIMESTAMP), returns TIMESTAMP);\n\nDATE_TRUNC(@at, MONTH)\n",
);
const SCALED: (&str, &str) = (
    "scaled",
    "MACRO (description \"Scaled\", arguments (v BIGINT, factor INT CONST), returns BIGINT);\n\n@v * @factor\n",
);
const SCALED_BY_RUNTIME: (&str, &str) = (
    "scaled_by_runtime",
    "MACRO (description \"Scaled\", arguments (v BIGINT, factor INT), returns BIGINT);\n\n@sql.scaled(@v, @factor)\n",
);
const SCALED_BY_CONST: (&str, &str) = (
    "scaled_by_const",
    "MACRO (description \"Scaled\", arguments (v BIGINT, factor INT CONST), returns BIGINT);\n\n@sql.scaled(@v, @factor)\n",
);
const STATUS_LABEL: (&str, &str) = (
    "status_label",
    "MACRO (description \"Label\", arguments (status VARCHAR CONST), returns VARCHAR);\n\n'status: ' || @status\n",
);
const HAS_TAG: (&str, &str) = (
    "has_tag",
    "MACRO (description \"Tagged\", arguments (tags VARCHAR[], tag VARCHAR), returns BOOLEAN);\n\nlist_contains(@tags, @tag)\n",
);
const IS_BLANK: (&str, &str) = (
    "is_blank",
    "MACRO (description \"Blank\", arguments (value VARCHAR), returns BOOLEAN);\n\n@value IS NULL OR TRIM(@value) = ''\n",
);
const IS_BLANK_PARENTHESISED: (&str, &str) = (
    "is_blank",
    "MACRO (description \"Blank\", arguments (value VARCHAR), returns BOOLEAN);\n\n((@value) IS NULL AND (@value) IS NULL) OR TRIM(@value) = ''\n",
);
const BOTH_MISSING: (&str, &str) = (
    "both_missing",
    "MACRO (description \"Both missing\", arguments (x VARCHAR, y VARCHAR), returns BOOLEAN);\n\n@x IS NULL AND (@y IS NULL)\n",
);
const IS_LETTER: (&str, &str) = (
    "is_letter",
    "MACRO (description \"Letter\", arguments (x VARCHAR), returns BOOLEAN);\n\nCASE WHEN @x IN ('a', 'b') THEN TRUE ELSE FALSE END\n",
);
const TRAILING_COMMENT: (&str, &str) = (
    "has_x",
    "MACRO (description \"Has x\", arguments (value VARCHAR), returns BOOLEAN);\n\nCONTAINS(@value, 'x') -- why x\n",
);
const SPACED_DECLARATIONS: (&str, &str) = (
    "spaced",
    "MACRO (description \"Spaced\", arguments (status VARCHAR, places INT), returns BOOLEAN);\n\n@status <> @enum (\"order_status\").CLOSED AND @places = @const (\"price_places\")\n",
);
const ORDER_KEY: (&str, &str) = (
    "order_key",
    "MACRO (description \"Order key\", arguments (store_code VARCHAR, order_number ANY), returns VARCHAR);\n\n@store_code || '::' || CAST(@order_number AS VARCHAR)\n",
);
const SHOUTED_WRONG: (&str, &str) = (
    "shouted",
    "MACRO (description \"Shouted\", arguments (value VARCHAR), returns BIGINT);\n\nUPPER(@value)\n",
);
const SHOUTED_UNTYPED: (&str, &str) = (
    "shouted",
    "MACRO (description \"Shouted\", arguments (value VARCHAR));\n\nUPPER(@value)\n",
);
const SHOUTED_ANY: (&str, &str) = (
    "shouted",
    "MACRO (description \"Shouted\", arguments (value ANY), returns BIGINT);\n\n@value\n",
);
const SHOUTED_NESTED: (&str, &str) = (
    "shouted_twice",
    "MACRO (description \"Shouted\", arguments (value VARCHAR), returns BIGINT);\n\n@sql.plus_one(LENGTH(@value)) || 'x'\n",
);
const ROUND_ANY: (&str, &str) = (
    "round_any",
    "MACRO (description \"Rounded\", arguments (v ANY NUMERIC, places INT CONST DEFAULT 2), returns TYPE OF v);\n\nROUND(@v, @places)\n",
);
const SHOUT_NUMBER: (&str, &str) = (
    "shout_number",
    "MACRO (description \"Shouted\", arguments (v ANY NUMERIC));\n\nUPPER(@v)\n",
);
const ODD: (&str, &str) = (
    "odd",
    "MACRO (description \"Odd\", arguments (v FOO BAR), returns BIGINT);\n\n@v + 1\n",
);

const WIDE: (&str, &str) = (
    "wide",
    "MACRO (description \"Wide\", arguments (v DECIMAL(50, 2)), returns BIGINT);\n\n@v + 1\n",
);
const NOT_NULL_ARGUMENT: (&str, &str) = (
    "not_null_argument",
    "MACRO (description \"Not null\", arguments (v INTEGER NOT NULL), returns BIGINT);\n\n@v + 1\n",
);
const WIDE_NUMERIC_RETURNS: (&str, &str) = (
    "wide_numeric",
    "MACRO (description \"Wide\", arguments (v INT64), returns NUMERIC(40, 2));\n\n@v + 1\n",
);

#[test]
fn given_calls_of_each_form_when_expanding_then_the_ticket_examples_compile() {
    let test_cases = [
        ExpansionTestCase {
            description: "expression with a named constant argument",
            adapter: "",
            macros: &[CENTS_TO_DOLLARS],
            sql: "SELECT @sql.cents_to_dollars(o.fee_cents + o.tax_cents, places := 0) AS fees FROM orders AS o",
            expected_sql: "SELECT CAST(ROUND((o.fee_cents + o.tax_cents) / 100, 0) AS DECIMAL(18, 2)) AS fees FROM orders AS o",
        },
        ExpansionTestCase {
            description: "expression stacking another expression macro",
            adapter: "",
            macros: &[CENTS_TO_DOLLARS, NET_AMOUNT],
            sql: "SELECT @sql.net_amount(o.gross_cents, o.refund_cents) AS net FROM orders AS o",
            expected_sql: "SELECT CAST(ROUND((o.gross_cents - COALESCE(o.refund_cents, 0)) / 100, 2) AS DECIMAL(18, 2)) AS net FROM orders AS o",
        },
        ExpansionTestCase {
            description: "a backslash-escaped quote in a BigQuery string is not a call",
            adapter: "bigquery",
            macros: &[NET_AMOUNT, CENTS_TO_DOLLARS],
            sql: "SELECT 'it\\'s @sql.x(' AS note, @sql.net_amount(o.a, o.b) AS net FROM orders AS o",
            expected_sql: "SELECT 'it\\'s @sql.x(' AS note, CAST(ROUND((o.a - COALESCE(o.b, 0)) / 100, 2) AS DECIMAL(18, 2)) AS net FROM orders AS o",
        },
        ExpansionTestCase {
            description: "an expression call before AT TIME ZONE is parenthesised",
            adapter: "",
            macros: &[PLUS_ONE],
            sql: "SELECT @sql.plus_one(o.v) AT TIME ZONE 'UTC' AS t FROM orders AS o",
            expected_sql: "SELECT (o.v + 1) AT TIME ZONE 'UTC' AS t FROM orders AS o",
        },
        ExpansionTestCase {
            description: "an expression call before FILTER is parenthesised",
            adapter: "",
            macros: &[PLUS_ONE],
            sql: "SELECT @sql.plus_one(o.v) FILTER (WHERE o.v > 0) AS t FROM orders AS o",
            expected_sql: "SELECT (o.v + 1) FILTER (WHERE o.v > 0) AS t FROM orders AS o",
        },
        ExpansionTestCase {
            description: "two different CTE-list calls without commas",
            adapter: "",
            macros: &[CLEAN_ORDERS, ALIASED_COLUMNS],
            sql: "WITH r AS (SELECT 1 AS id) @sql.clean_orders(r) @sql.aliased_ids(latest) final AS (SELECT 1) SELECT * FROM final",
            expected_sql: "WITH r AS (SELECT 1 AS id) , live AS (SELECT * FROM r WHERE NOT is_deleted),\nranked AS (\n  SELECT *, ROW_NUMBER() OVER (PARTITION BY order_id ORDER BY updated_at DESC) AS row_num\n  FROM live\n),\nlatest AS (SELECT * EXCLUDE (row_num) FROM ranked WHERE row_num = 1), picked_ids AS (SELECT s.id FROM latest AS s), final AS (SELECT 1) SELECT * FROM final",
        },
        ExpansionTestCase {
            description: "a comma after the last CTE-list call before the final query",
            adapter: "",
            macros: &[ALIASED_COLUMNS],
            sql: "WITH r AS (SELECT 1 AS id), @sql.aliased_ids(r), SELECT * FROM picked_ids",
            expected_sql: "WITH r AS (SELECT 1 AS id), picked_ids AS (SELECT s.id FROM r AS s) SELECT * FROM picked_ids",
        },
        ExpansionTestCase {
            description: "a column-list call without a comma before it",
            adapter: "",
            macros: &[AUDIT_COLUMNS],
            sql: "SELECT o.order_id @sql.audit_columns(o) FROM orders AS o",
            expected_sql: "SELECT o.order_id , o.created_at, o.updated_at, o.updated_at > o.created_at AS was_updated FROM orders AS o",
        },
        ExpansionTestCase {
            description: "CTE list inserted flat with its own names",
            adapter: "",
            macros: &[CLEAN_ORDERS],
            sql: "WITH orders_raw AS (SELECT * FROM __ref(\"stg_orders\")),\n@sql.clean_orders(orders_raw)\nfinal AS (SELECT order_id, amount FROM latest)\nSELECT * FROM final",
            expected_sql: "WITH orders_raw AS (SELECT * FROM __ref(\"stg_orders\")),\nlive AS (SELECT * FROM orders_raw WHERE NOT is_deleted),\nranked AS (\n  SELECT *, ROW_NUMBER() OVER (PARTITION BY order_id ORDER BY updated_at DESC) AS row_num\n  FROM live\n),\nlatest AS (SELECT * EXCLUDE (row_num) FROM ranked WHERE row_num = 1),\nfinal AS (SELECT order_id, amount FROM latest)\nSELECT * FROM final",
        },
        ExpansionTestCase {
            description: "CTE list called twice, each call naming its CTEs with __cte",
            adapter: "",
            macros: &[CLEAN_ORDERS],
            sql: "WITH eu_raw AS (SELECT 1 AS order_id), us_raw AS (SELECT 2 AS order_id),\n@sql.clean_orders(eu_raw, __cte := eu_orders)\n@sql.clean_orders(us_raw, __CTE := us_orders)\nfinal AS (SELECT * FROM eu_orders UNION ALL SELECT * FROM us_orders)\nSELECT * FROM final",
            expected_sql: "WITH eu_raw AS (SELECT 1 AS order_id), us_raw AS (SELECT 2 AS order_id),\neu_orders__live AS (SELECT * FROM eu_raw WHERE NOT is_deleted),\neu_orders__ranked AS (\n  SELECT *, ROW_NUMBER() OVER (PARTITION BY order_id ORDER BY updated_at DESC) AS row_num\n  FROM eu_orders__live\n),\neu_orders AS (SELECT * EXCLUDE (row_num) FROM eu_orders__ranked WHERE row_num = 1),\nus_orders__live AS (SELECT * FROM us_raw WHERE NOT is_deleted),\nus_orders__ranked AS (\n  SELECT *, ROW_NUMBER() OVER (PARTITION BY order_id ORDER BY updated_at DESC) AS row_num\n  FROM us_orders__live\n),\nus_orders AS (SELECT * EXCLUDE (row_num) FROM us_orders__ranked WHERE row_num = 1),\nfinal AS (SELECT * FROM eu_orders UNION ALL SELECT * FROM us_orders)\nSELECT * FROM final",
        },
        ExpansionTestCase {
            description: "__cte renames the macro's CTEs but never an argument of the same name",
            adapter: "",
            macros: &[CLEAN_ORDERS],
            sql: "WITH live AS (SELECT 1 AS order_id), @sql.clean_orders(live, __cte := picked) SELECT * FROM picked",
            expected_sql: "WITH live AS (SELECT 1 AS order_id), picked__live AS (SELECT * FROM live WHERE NOT is_deleted),\npicked__ranked AS (\n  SELECT *, ROW_NUMBER() OVER (PARTITION BY order_id ORDER BY updated_at DESC) AS row_num\n  FROM picked__live\n),\npicked AS (SELECT * EXCLUDE (row_num) FROM picked__ranked WHERE row_num = 1) SELECT * FROM picked",
        },
        ExpansionTestCase {
            description: "__cte renames qualified references to a renamed CTE",
            adapter: "",
            macros: &[LATEST_PER_KEY],
            sql: "WITH r AS (SELECT 1 AS id, 2 AS updated_at), @sql.latest_per_key(r, src.keys := (id), __cte := l) SELECT * FROM l",
            expected_sql: "WITH r AS (SELECT 1 AS id, 2 AS updated_at), l__latest_ranked AS (\n  SELECT r.*, ROW_NUMBER() OVER (PARTITION BY id ORDER BY updated_at DESC) AS row_num FROM r\n),\nl AS (SELECT * EXCLUDE (row_num) FROM l__latest_ranked WHERE row_num = 1) SELECT * FROM l",
        },
        ExpansionTestCase {
            description: "CTE list extracted from a model compiles to the original",
            adapter: "",
            macros: &[LATEST_LIVE_ORDERS],
            sql: "WITH c1 AS (SELECT * FROM __ref(\"stg_orders\")),\n@sql.latest_live_orders(c1)\nc5 AS (SELECT order_id, amount FROM c4)\nSELECT * FROM c5",
            expected_sql: "WITH c1 AS (SELECT * FROM __ref(\"stg_orders\")),\nc2 AS (SELECT * FROM c1 WHERE NOT is_deleted),\nc3 AS (SELECT *, ROW_NUMBER() OVER (PARTITION BY order_id ORDER BY updated_at DESC) AS row_num FROM c2),\nc4 AS (SELECT * EXCLUDE (row_num) FROM c3 WHERE row_num = 1),\nc5 AS (SELECT order_id, amount FROM c4)\nSELECT * FROM c5",
        },
        ExpansionTestCase {
            description: "CTE list last before the final query, comma after the call optional",
            adapter: "",
            macros: &[CLEAN_ORDERS],
            sql: "WITH orders_raw AS (SELECT 1 AS order_id), @sql.clean_orders(orders_raw), final AS (SELECT * FROM latest) SELECT * FROM final",
            expected_sql: "WITH orders_raw AS (SELECT 1 AS order_id), live AS (SELECT * FROM orders_raw WHERE NOT is_deleted),\nranked AS (\n  SELECT *, ROW_NUMBER() OVER (PARTITION BY order_id ORDER BY updated_at DESC) AS row_num\n  FROM live\n),\nlatest AS (SELECT * EXCLUDE (row_num) FROM ranked WHERE row_num = 1), final AS (SELECT * FROM latest) SELECT * FROM final",
        },
        ExpansionTestCase {
            description: "CTE list reading a reference relation keeps the parameter as its alias",
            adapter: "",
            macros: &[CLEAN_ORDERS],
            sql: "WITH @sql.clean_orders(__ref(\"stg_orders\")) SELECT * FROM latest",
            expected_sql: "WITH live AS (SELECT * FROM __ref(\"stg_orders\") AS src WHERE NOT is_deleted),\nranked AS (\n  SELECT *, ROW_NUMBER() OVER (PARTITION BY order_id ORDER BY updated_at DESC) AS row_num\n  FROM live\n),\nlatest AS (SELECT * EXCLUDE (row_num) FROM ranked WHERE row_num = 1) SELECT * FROM latest",
        },
        ExpansionTestCase {
            description: "column list inserted into the select list",
            adapter: "",
            macros: &[AUDIT_COLUMNS],
            sql: "SELECT o.order_id, @sql.audit_columns(o) FROM orders AS o",
            expected_sql: "SELECT o.order_id, o.created_at, o.updated_at, o.updated_at > o.created_at AS was_updated FROM orders AS o",
        },
        ExpansionTestCase {
            description: "column list followed by another column without a comma",
            adapter: "",
            macros: &[AUDIT_COLUMNS],
            sql: "SELECT @sql.audit_columns(o) o.order_id FROM orders AS o",
            expected_sql: "SELECT o.created_at, o.updated_at, o.updated_at > o.created_at AS was_updated, o.order_id FROM orders AS o",
        },
        ExpansionTestCase {
            description: "enum default resolved in the macro's scope",
            adapter: "",
            macros: &[IS_OPEN_STATUS],
            sql: "SELECT * FROM orders AS o WHERE @sql.is_open_status(o.status)",
            expected_sql: "SELECT * FROM orders AS o WHERE o.status <> 'closed'",
        },
        ExpansionTestCase {
            description: "expression parenthesised between operators",
            adapter: "",
            macros: &[IS_OPEN_STATUS],
            sql: "SELECT * FROM orders AS o WHERE o.id > 1 AND @sql.is_open_status(o.status)",
            expected_sql: "SELECT * FROM orders AS o WHERE o.id > 1 AND (o.status <> 'closed')",
        },
        ExpansionTestCase {
            description: "list constant passed by name",
            adapter: "",
            macros: &[IN_SUPPORTED_COUNTRY],
            sql: "SELECT * FROM orders AS o WHERE @sql.in_supported_country(o.country_code, countries := @const(\"supported_countries\"))",
            expected_sql: "SELECT * FROM orders AS o WHERE o.country_code IN ('GB', 'FR', 'HK')",
        },
        ExpansionTestCase {
            description: "inline list default renders like a constant",
            adapter: "",
            macros: &[IN_SUPPORTED_COUNTRY],
            sql: "SELECT @sql.in_supported_country(o.country_code) AS supported FROM orders AS o",
            expected_sql: "SELECT o.country_code IN ('GB', 'FR') AS supported FROM orders AS o",
        },
        ExpansionTestCase {
            description: "a COLUMNS slot of a relation",
            adapter: "",
            macros: &[LATEST_PER_KEY],
            sql: "WITH customers_raw AS (SELECT 1 AS customer_id), @sql.latest_per_key(customers_raw, src.keys := (customer_id)) SELECT * FROM latest_rows",
            expected_sql: "WITH customers_raw AS (SELECT 1 AS customer_id), latest_ranked AS (\n  SELECT customers_raw.*, ROW_NUMBER() OVER (PARTITION BY customer_id ORDER BY updated_at DESC) AS row_num FROM customers_raw\n),\nlatest_rows AS (SELECT * EXCLUDE (row_num) FROM latest_ranked WHERE row_num = 1) SELECT * FROM latest_rows",
        },
        ExpansionTestCase {
            description: "a bare name is the column and @name the parameter of the same name",
            adapter: "",
            macros: &[STATUS_MATCHES],
            sql: "SELECT * FROM orders AS o WHERE @sql.status_matches('open')",
            expected_sql: "SELECT * FROM orders AS o WHERE status = 'open'",
        },
        ExpansionTestCase {
            description: "an aliased reference relation keeps its alias",
            adapter: "",
            macros: &[ALIASED_COLUMNS],
            sql: "WITH @sql.aliased_ids(__ref(\"orders\")) SELECT * FROM picked_ids",
            expected_sql: "WITH picked_ids AS (SELECT s.id FROM __ref(\"orders\") AS s) SELECT * FROM picked_ids",
        },
        ExpansionTestCase {
            description: "SQL macro result passed to a Python macro as a string",
            adapter: "",
            macros: &[CENTS_TO_DOLLARS],
            sql: "SELECT @py.label(@sql.cents_to_dollars(o.fee)) AS fee FROM orders AS o",
            expected_sql: "SELECT @py.label(\"CAST(ROUND(o.fee / 100, 2) AS DECIMAL(18, 2))\") AS fee FROM orders AS o",
        },
        ExpansionTestCase {
            description: "runtime ROUND precision where the adapter allows one",
            adapter: "postgres",
            macros: &[ROUND_TO],
            sql: "SELECT @sql.round_to(o.amount, 2) FROM orders AS o",
            expected_sql: "SELECT ROUND(o.amount, 2) FROM orders AS o",
        },
        ExpansionTestCase {
            description: "DATE_TRUNC with its unit last on BigQuery",
            adapter: "bigquery",
            macros: &[MONTH_START],
            sql: "SELECT @sql.month_start(o.created_at) FROM orders AS o",
            expected_sql: "SELECT DATE_TRUNC(o.created_at, MONTH) FROM orders AS o",
        },
        ExpansionTestCase {
            description: "enum value for a CONST argument",
            adapter: "",
            macros: &[STATUS_LABEL],
            sql: "SELECT @sql.status_label(@enum(\"order_status\").OPEN) AS label",
            expected_sql: "SELECT 'status: ' || 'open' AS label",
        },
        ExpansionTestCase {
            description: "CONST parameter forwarded to a CONST argument",
            adapter: "",
            macros: &[SCALED, SCALED_BY_CONST],
            sql: "SELECT @sql.scaled_by_const(o.amount, 3) AS v FROM orders AS o",
            expected_sql: "SELECT o.amount * 3 AS v FROM orders AS o",
        },
        ExpansionTestCase {
            description: "runtime array column for an array parameter",
            adapter: "duckdb",
            macros: &[HAS_TAG],
            sql: "SELECT @sql.has_tag(o.tags, 'vip') AS vip FROM orders AS o",
            expected_sql: "SELECT list_contains(o.tags, 'vip') AS vip FROM orders AS o",
        },
        ExpansionTestCase {
            description: "a parameter followed by words and a parenthesis is a parameter",
            adapter: "",
            macros: &[IS_BLANK],
            sql: "SELECT @sql.is_blank(o.email) AS b FROM orders AS o",
            expected_sql: "SELECT o.email IS NULL OR TRIM(o.email) = '' AS b FROM orders AS o",
        },
        ExpansionTestCase {
            description: "parenthesised parameters in a body opening with two parentheses",
            adapter: "",
            macros: &[IS_BLANK_PARENTHESISED],
            sql: "SELECT @sql.is_blank(o.email) AS b FROM orders AS o",
            expected_sql: "SELECT ((o.email) IS NULL AND (o.email) IS NULL) OR TRIM(o.email) = '' AS b FROM orders AS o",
        },
        ExpansionTestCase {
            description: "a parameter before AND and a parenthesised predicate",
            adapter: "",
            macros: &[BOTH_MISSING],
            sql: "SELECT @sql.both_missing(o.a, o.b) AS m FROM orders AS o",
            expected_sql: "SELECT o.a IS NULL AND (o.b IS NULL) AS m FROM orders AS o",
        },
        ExpansionTestCase {
            description: "a parameter before IN inside a CASE",
            adapter: "",
            macros: &[IS_LETTER],
            sql: "SELECT @sql.is_letter(o.a) AS l FROM orders AS o",
            expected_sql: "SELECT CASE WHEN o.a IN ('a', 'b') THEN TRUE ELSE FALSE END AS l FROM orders AS o",
        },
        ExpansionTestCase {
            description: "a body ending in a line comment ends its line, so the call's alias stays code",
            adapter: "",
            macros: &[TRAILING_COMMENT],
            sql: "SELECT @sql.has_x(o.a) AS l FROM orders AS o",
            expected_sql: "SELECT CONTAINS(o.a, 'x') -- why x\n AS l FROM orders AS o",
        },
        ExpansionTestCase {
            description: "ANY arguments of any type: a column, a number, a string, an expression and a scalar constant",
            adapter: "duckdb",
            macros: &[ORDER_KEY],
            sql: "SELECT @sql.order_key(o.store, o.number) AS a, @sql.order_key(o.store, 42) AS b, @sql.order_key(o.store, 'x') AS c, @sql.order_key(o.store, o.number + 1) AS d, @sql.order_key(o.store, @const(\"price_places\")) AS e FROM orders AS o",
            expected_sql: "SELECT o.store || '::' || CAST(o.number AS VARCHAR) AS a, o.store || '::' || CAST(42 AS VARCHAR) AS b, o.store || '::' || CAST('x' AS VARCHAR) AS c, o.store || '::' || CAST((o.number + 1) AS VARCHAR) AS d, o.store || '::' || CAST(2 AS VARCHAR) AS e FROM orders AS o",
        },
        ExpansionTestCase {
            description: "a body ending in a line comment, called last on its line, adds no line",
            adapter: "",
            macros: &[TRAILING_COMMENT],
            sql: "SELECT @sql.has_x(o.a)\nFROM orders AS o",
            expected_sql: "SELECT CONTAINS(o.a, 'x') -- why x\nFROM orders AS o",
        },
        ExpansionTestCase {
            description: "a body ending in a line comment, followed by an operator, closes its line",
            adapter: "",
            macros: &[TRAILING_COMMENT],
            sql: "SELECT @sql.has_x(o.a) = TRUE\nFROM orders AS o",
            expected_sql: "SELECT CONTAINS(o.a, 'x') -- why x\n = TRUE\nFROM orders AS o",
        },
        ExpansionTestCase {
            description: "spaced enum and constant references in a body are declarations",
            adapter: "",
            macros: &[SPACED_DECLARATIONS],
            sql: "SELECT @sql.spaced(o.status, o.places) AS s FROM orders AS o",
            expected_sql: "SELECT o.status <> @enum (\"order_status\").CLOSED AND o.places = @const (\"price_places\") AS s FROM orders AS o",
        },
        ExpansionTestCase {
            description: "generic SQL checks no adapter types",
            adapter: "",
            macros: &[ODD],
            sql: "SELECT @sql.odd(1) AS a",
            expected_sql: "SELECT 1 + 1 AS a",
        },
        ExpansionTestCase {
            description: "an ANY family argument of its family",
            adapter: "duckdb",
            macros: &[ROUND_ANY],
            sql: "SELECT @sql.round_any(o.amount, 1) AS a FROM orders AS o",
            expected_sql: "SELECT ROUND(o.amount, 1) AS a FROM orders AS o",
        },
        ExpansionTestCase {
            description: "an expression macro without returns",
            adapter: "duckdb",
            macros: &[SHOUTED_UNTYPED],
            sql: "SELECT @sql.shouted(o.status) AS a FROM orders AS o",
            expected_sql: "SELECT UPPER(o.status) AS a FROM orders AS o",
        },
        ExpansionTestCase {
            description: "an ANY argument leaves the body untyped, so returns is trusted",
            adapter: "duckdb",
            macros: &[SHOUTED_ANY],
            sql: "SELECT @sql.shouted(o.status) AS a FROM orders AS o",
            expected_sql: "SELECT o.status AS a FROM orders AS o",
        },
    ];

    for test_case in test_cases {
        let result = expanded_for(test_case.adapter, test_case.macros, test_case.sql);

        assert_eq!(
            result.as_deref(),
            Ok(test_case.expected_sql),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_invalid_calls_when_expanding_then_each_error_names_the_problem() {
    let test_cases = [
        ExpansionErrorTestCase {
            description: "CTE name clash with the caller",
            adapter: "",
            macros: &[CLEAN_ORDERS],
            sql: "WITH latest AS (SELECT 1 AS x), orders_raw AS (SELECT 1 AS order_id), @sql.clean_orders(orders_raw) SELECT * FROM latest",
            expected_code: "P018",
            expected_message: "CTE \"latest\" from macro clean_orders() clashes with a CTE already defined in model:orders_clean",
        },
        ExpansionErrorTestCase {
            description: "the same CTE macro called twice",
            adapter: "",
            macros: &[CLEAN_ORDERS],
            sql: "WITH a AS (SELECT 1), @sql.clean_orders(a) @sql.clean_orders(a) SELECT 1",
            expected_code: "P018",
            expected_message: "CTE \"live\" from macro clean_orders() clashes with a CTE from macro clean_orders() in model:orders_clean",
        },
        ExpansionErrorTestCase {
            description: "__cte on an expression macro",
            adapter: "",
            macros: &[PLUS_ONE],
            sql: "SELECT @sql.plus_one(1, __cte := x)",
            expected_code: "P017",
            expected_message: "__cte names the CTEs a CTE-list macro inserts; plus_one() is an expression macro",
        },
        ExpansionErrorTestCase {
            description: "__cte that is not a bare name",
            adapter: "",
            macros: &[CLEAN_ORDERS],
            sql: "WITH a AS (SELECT 1), @sql.clean_orders(a, __cte := 'x') SELECT 1",
            expected_code: "P017",
            expected_message: "__cte of macro clean_orders() must be a bare CTE name; got 'x'",
        },
        ExpansionErrorTestCase {
            description: "the same __cte name twice",
            adapter: "",
            macros: &[CLEAN_ORDERS],
            sql: "WITH a AS (SELECT 1), @sql.clean_orders(a, __cte := x) @sql.clean_orders(a, __cte := x) SELECT 1",
            expected_code: "P018",
            expected_message: "CTE \"x__live\" from macro clean_orders() clashes with a CTE from macro clean_orders() in model:orders_clean",
        },
        ExpansionErrorTestCase {
            description: "a __cte name the caller already defines",
            adapter: "",
            macros: &[CLEAN_ORDERS],
            sql: "WITH a AS (SELECT 1), x AS (SELECT 2), @sql.clean_orders(a, __cte := x) SELECT 1",
            expected_code: "P018",
            expected_message: "CTE \"x\" from macro clean_orders() clashes with a CTE already defined in model:orders_clean",
        },
        ExpansionErrorTestCase {
            description: "column clash with the caller",
            adapter: "",
            macros: &[AUDIT_COLUMNS],
            sql: "SELECT o.created_at, @sql.audit_columns(o) FROM orders AS o",
            expected_code: "P018",
            expected_message: "column \"created_at\" from macro audit_columns() clashes with a column already defined in model:orders_clean",
        },
        ExpansionErrorTestCase {
            description: "a body whose known type is not returns",
            adapter: "duckdb",
            macros: &[SHOUTED_WRONG],
            sql: "SELECT @sql.shouted(o.status) FROM orders AS o",
            expected_code: "P024",
            expected_message: "macro shouted() declares returns BIGINT, but its body is VARCHAR",
        },
        ExpansionErrorTestCase {
            description: "a body typed through a nested call",
            adapter: "duckdb",
            macros: &[SHOUTED_NESTED, PLUS_ONE],
            sql: "SELECT @sql.shouted_twice(o.status) FROM orders AS o",
            expected_code: "P024",
            expected_message: "macro shouted_twice() declares returns BIGINT, but its body is VARCHAR",
        },
        ExpansionErrorTestCase {
            description: "a literal outside an ANY family",
            adapter: "duckdb",
            macros: &[ROUND_ANY],
            sql: "SELECT @sql.round_any('ten')",
            expected_code: "P017",
            expected_message: "argument v of macro round_any() expects a number; 'ten' is a string",
        },
        ExpansionErrorTestCase {
            description: "a body that misuses an ANY family argument",
            adapter: "duckdb",
            macros: &[SHOUT_NUMBER],
            sql: "SELECT @sql.shout_number(o.amount) FROM orders AS o",
            expected_code: "P025",
            expected_message: "body of macro shout_number() does not accept every value of v ANY NUMERIC",
        },
        ExpansionErrorTestCase {
            description: "cycle",
            adapter: "",
            macros: &[PING, PONG],
            sql: "SELECT @sql.ping(1)",
            expected_code: "P019",
            expected_message: "SQL macros call each other in a cycle: ping() -> pong() -> ping()",
        },
        ExpansionErrorTestCase {
            description: "CTE list called in a select list",
            adapter: "",
            macros: &[CLEAN_ORDERS],
            sql: "SELECT @sql.clean_orders(o) FROM orders AS o",
            expected_code: "P021",
            expected_message: "CTE-list macro clean_orders() must be called inside a WITH list",
        },
        ExpansionErrorTestCase {
            description: "column list called in an expression",
            adapter: "",
            macros: &[AUDIT_COLUMNS],
            sql: "SELECT 1 + @sql.audit_columns(o) FROM orders AS o",
            expected_code: "P021",
            expected_message: "column-list macro audit_columns() must be called as an item of a select list",
        },
        ExpansionErrorTestCase {
            description: "string literal for a BIGINT argument",
            adapter: "",
            macros: &[CENTS_TO_DOLLARS],
            sql: "SELECT @sql.cents_to_dollars('ten')",
            expected_code: "P017",
            expected_message: "argument amount_cents of macro cents_to_dollars() expects BIGINT; 'ten' is a string",
        },
        ExpansionErrorTestCase {
            description: "column for a CONST argument",
            adapter: "",
            macros: &[CENTS_TO_DOLLARS],
            sql: "SELECT @sql.cents_to_dollars(o.fee, o.places) FROM orders AS o",
            expected_code: "P017",
            expected_message: "argument places of macro cents_to_dollars() expects INT CONST: a literal, a scalar constant or an enum value; got o.places",
        },
        ExpansionErrorTestCase {
            description: "wrong enum",
            adapter: "",
            macros: &[IS_OPEN_STATUS],
            sql: "SELECT @sql.is_open_status(o.status, @enum(\"other\").CLOSED) FROM orders AS o",
            expected_code: "P017",
            expected_message: "argument closed of macro is_open_status() expects ENUM order_status: a member of enum order_status",
        },
        ExpansionErrorTestCase {
            description: "unknown named argument",
            adapter: "",
            macros: &[CENTS_TO_DOLLARS],
            sql: "SELECT @sql.cents_to_dollars(o.fee, digits := 2) FROM orders AS o",
            expected_code: "P017",
            expected_message: "macro cents_to_dollars() has no argument digits",
        },
        ExpansionErrorTestCase {
            description: "missing argument",
            adapter: "",
            macros: &[CENTS_TO_DOLLARS],
            sql: "SELECT @sql.cents_to_dollars()",
            expected_code: "P017",
            expected_message: "call to macro cents_to_dollars() is missing argument amount_cents BIGINT",
        },
        ExpansionErrorTestCase {
            description: "expression for a RELATION argument",
            adapter: "",
            macros: &[CLEAN_ORDERS],
            sql: "WITH @sql.clean_orders(1 + 2) SELECT 1",
            expected_code: "P017",
            expected_message: "argument src of macro clean_orders() expects RELATION (order_id BIGINT, updated_at TIMESTAMP, is_deleted BOOLEAN): a CTE name, a table alias, or __ref(), __source() or __seed(); got 1 + 2",
        },
        ExpansionErrorTestCase {
            description: "runtime ROUND precision on DuckDB",
            adapter: "duckdb",
            macros: &[ROUND_TO],
            sql: "SELECT @sql.round_to(o.amount, 2) FROM orders AS o",
            expected_code: "P023",
            expected_message: "places is used where a literal is required; declare it places INT CONST",
        },
        ExpansionErrorTestCase {
            description: "runtime DATE_TRUNC unit on Snowflake",
            adapter: "snowflake",
            macros: &[MONTH_OF],
            sql: "SELECT @sql.month_of(o.created_at, 'month') FROM orders AS o",
            expected_code: "P023",
            expected_message: "unit is used where a literal is required; declare it unit VARCHAR CONST",
        },
        ExpansionErrorTestCase {
            description: "DATE_TRUNC unit position on Snowflake",
            adapter: "snowflake",
            macros: &[MONTH_START],
            sql: "SELECT @sql.month_start(o.created_at) FROM orders AS o",
            expected_code: "P023",
            expected_message: "at is used where a literal is required",
        },
        ExpansionErrorTestCase {
            description: "runtime parameter forwarded to a CONST argument",
            adapter: "",
            macros: &[SCALED, SCALED_BY_RUNTIME],
            sql: "SELECT @sql.scaled_by_runtime(o.amount, 3) AS v FROM orders AS o",
            expected_code: "P017",
            expected_message: "argument factor of macro scaled() expects INT CONST",
        },
        ExpansionErrorTestCase {
            description: "a list passed to an ANY argument",
            adapter: "",
            macros: &[ORDER_KEY],
            sql: "SELECT @sql.order_key(o.store, [1, 2]) AS k FROM orders AS o",
            expected_code: "P017",
            expected_message: "argument order_number of macro order_key() expects ANY: a scalar SQL expression; got [1, 2]",
        },
        ExpansionErrorTestCase {
            description: "a list constant passed to an ANY argument",
            adapter: "",
            macros: &[ORDER_KEY],
            sql: "SELECT @sql.order_key(o.store, @const(\"supported_countries\")) AS k FROM orders AS o",
            expected_code: "P017",
            expected_message: "argument order_number of macro order_key() expects ANY: a scalar constant",
        },
        ExpansionErrorTestCase {
            description: "a relation passed to an ANY argument",
            adapter: "",
            macros: &[ORDER_KEY],
            sql: "SELECT @sql.order_key(o.store, __ref(\"orders\")) AS k FROM orders AS o",
            expected_code: "P017",
            expected_message: "argument order_number of macro order_key() expects ANY: a scalar SQL expression",
        },
        ExpansionErrorTestCase {
            description: "type the adapter cannot parse",
            adapter: "duckdb",
            macros: &[ODD],
            sql: "SELECT @sql.odd(1) AS a",
            expected_code: "P015",
            expected_message: "argument v of macro odd() type 'FOO BAR' is not valid for adapter 'duckdb'",
        },
        ExpansionErrorTestCase {
            description: "an argument type Postgres cannot parse",
            adapter: "postgres",
            macros: &[ODD],
            sql: "SELECT @sql.odd(1) AS a",
            expected_code: "P015",
            expected_message: "argument v of macro odd() type 'FOO BAR' is not valid for adapter 'postgres'",
        },
        ExpansionErrorTestCase {
            description: "an argument type MotherDuck reads as DuckDB",
            adapter: "motherduck",
            macros: &[NOT_NULL_ARGUMENT],
            sql: "SELECT @sql.not_null_argument(1) AS a",
            expected_code: "P015",
            expected_message: "argument v of macro not_null_argument() type 'INTEGER NOT NULL' is not valid for adapter 'motherduck'",
        },
        ExpansionErrorTestCase {
            description: "a decimal argument wider than DuckDB holds",
            adapter: "duckdb",
            macros: &[WIDE],
            sql: "SELECT @sql.wide(1) AS a",
            expected_code: "P015",
            expected_message: "argument v of macro wide() type 'DECIMAL(50, 2)' is not valid for adapter 'duckdb'",
        },
        ExpansionErrorTestCase {
            description: "a NUMERIC returns wider than BigQuery holds",
            adapter: "bigquery",
            macros: &[WIDE_NUMERIC_RETURNS],
            sql: "SELECT @sql.wide_numeric(1) AS a",
            expected_code: "P015",
            expected_message: "returns of macro wide_numeric() type 'NUMERIC(40, 2)' is not valid for adapter 'bigquery'",
        },
    ];

    for test_case in test_cases {
        let error = expanded_for(test_case.adapter, test_case.macros, test_case.sql)
            .expect_err(test_case.description);

        assert_eq!(
            error.code, test_case.expected_code,
            "{}: {}",
            test_case.description, error.message
        );
        assert!(
            error.message.starts_with(test_case.expected_message),
            "{}: {}",
            test_case.description,
            error.message
        );
    }
}

#[test]
fn given_a_sql_macro_call_in_python_macro_arguments_when_expanding_then_python_receives_its_expansion()
 {
    let wrapped = |sql: String| vec![ArgumentValue::Str(sql)];
    let test_cases = [
        PythonArgumentTestCase {
            description: "a double quote inside a SQL string literal",
            body: "CONTAINS(@value, '\"')",
            sql: "SELECT @py.wrap(@sql.m(o.email)) AS probe",
            expected_suffix: "",
            expected_arguments: wrapped,
        },
        PythonArgumentTestCase {
            description: "a double quote compared directly",
            body: "@value = '\"'",
            sql: "SELECT @py.wrap(@sql.m(o.email)) AS probe",
            expected_suffix: "",
            expected_arguments: wrapped,
        },
        PythonArgumentTestCase {
            description: "a single quote inside a double-quoted identifier",
            body: "@value = \"it's\"",
            sql: "SELECT @py.wrap(@sql.m(o.email)) AS probe",
            expected_suffix: "",
            expected_arguments: wrapped,
        },
        PythonArgumentTestCase {
            description: "doubled single quotes",
            body: "@value = ''''",
            sql: "SELECT @py.wrap(@sql.m(o.email)) AS probe",
            expected_suffix: "",
            expected_arguments: wrapped,
        },
        PythonArgumentTestCase {
            description: "backslashes, one before a quote",
            body: "@value LIKE 'a\\_b' OR @value = '\\'",
            sql: "SELECT @py.wrap(@sql.m(o.email)) AS probe",
            expected_suffix: "",
            expected_arguments: wrapped,
        },
        PythonArgumentTestCase {
            description: "a line break",
            body: "@value = 'a'\n  OR @value = 'b'",
            sql: "SELECT @py.wrap(@sql.m(o.email)) AS probe",
            expected_suffix: "",
            expected_arguments: wrapped,
        },
        PythonArgumentTestCase {
            description: "a line comment, a block comment and a comment ending the body",
            body: "-- why\nCONTAINS(@value, /* sign */ '\"') -- end",
            sql: "SELECT @py.wrap(@sql.m(o.email)) AS probe",
            expected_suffix: "\n",
            expected_arguments: wrapped,
        },
        PythonArgumentTestCase {
            description: "a dict value",
            body: "CONTAINS(@value, '\"')",
            sql: "SELECT @py.pick({\"a\": @sql.m(o.email)}) AS probe",
            expected_suffix: "",
            expected_arguments: |sql| {
                vec![ArgumentValue::Dict(vec![(
                    ArgumentValue::Str("a".to_owned()),
                    ArgumentValue::Str(sql),
                )])]
            },
        },
        PythonArgumentTestCase {
            description: "a nested list",
            body: "CONTAINS(@value, '\"')",
            sql: "SELECT @py.pick([@sql.m(o.email), [@sql.m(o.email)]]) AS probe",
            expected_suffix: "",
            expected_arguments: |sql| {
                vec![ArgumentValue::List(vec![
                    ArgumentValue::Str(sql.clone()),
                    ArgumentValue::List(vec![ArgumentValue::Str(sql)]),
                ])]
            },
        },
    ];

    for test_case in test_cases {
        let source: String = format!(
            "MACRO (description \"M\", arguments (value VARCHAR), returns BOOLEAN);\n\n{}\n",
            test_case.body
        );
        let macros: [(&str, &str); 1] = [("m", source.as_str())];
        let direct: Result<String, _> = expanded_for("", &macros, "@sql.m(o.email)");
        let expanded: Result<String, _> = expanded_for("", &macros, test_case.sql);
        let (Ok(direct), Ok(expanded)) = (direct, expanded) else {
            panic!("{} does not expand", test_case.description);
        };

        let scan = scan_macro_call_sites(&expanded, None);
        let arguments: Option<String> = scan.sites.first().map(|site| {
            expanded
                .chars()
                .skip(site.args_start)
                .take(site.end - 1 - site.args_start)
                .collect()
        });
        let parsed =
            arguments.map(|arguments| parse_macro_arguments(&AsciiArguments, &arguments, &[]));

        assert_eq!(
            (scan.failure, scan.sites.len()),
            (None, 1),
            "{}",
            test_case.description
        );
        assert_eq!(
            parsed.map(|parsed| parsed.map(|arguments| arguments.positional)),
            Some(Ok((test_case.expected_arguments)(
                direct + test_case.expected_suffix
            ))),
            "{}",
            test_case.description
        );
    }
}
