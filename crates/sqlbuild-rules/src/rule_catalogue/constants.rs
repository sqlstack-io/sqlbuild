//! Threshold defaults, guidance wording and the rule codes guidance sections depend on.

use crate::constants::MIN_CUSTOM_RULE_TEST_CASES_KEY;

/// Effective threshold defaults, sorted by name as guidance renders them.
pub(crate) const THRESHOLD_DEFAULTS: [(&str, u32); 10] = [
    ("max_constant_container_files", 10),
    ("max_enum_container_files", 10),
    ("max_macro_container_files", 10),
    ("max_role_container_depth", 1),
    ("max_subdomain_depth", 1),
    ("min_audits_per_model", 1),
    (MIN_CUSTOM_RULE_TEST_CASES_KEY, 1),
    ("min_shared_container_prefix_files", 2),
    ("min_shared_owner_prefix_directories", 2),
    ("min_tests_per_model", 1),
];
/// The Python type every option value is an instance of.
pub(crate) const OBJECT_TYPE: &str = "object";
/// The first printable code point; Python `repr` escapes control characters below it.
pub(crate) const FIRST_PRINTABLE: u32 = 0x20;
/// The DEL control character, which Python `repr` escapes.
pub(crate) const DELETE_CHARACTER: u32 = 0x7f;
pub(crate) const MINIMUM_RULES_CODES: [&str; 2] = ["SQBRTEST201", "SQBRTEST202"];
pub(crate) const LAYOUT_THRESHOLD_RULE_CODES: [&str; 5] = [
    "SQBRPROJECT203",
    "SQBRPROJECT204",
    "SQBRDECLARATION303",
    "SQBRDECLARATION304",
    "SQBRDECLARATION306",
];
pub(crate) const LAYOUT_RULE_PREFIXES: [&str; 2] = ["SQBRPROJECT2", "SQBRDECLARATION3"];
pub(crate) const TEST_RULE_PREFIX: &str = "SQBRTEST";
pub(crate) const GRAPH_EDGE_RULE_CODE: &str = "SQBRGRAPH101";
pub(crate) const REPLACEABLE_SKILL_STATES: [&str; 3] = ["fresh", "missing", "stale"];
pub(crate) const RULE_EXAMPLES: [(&str, &str); 28] = [
    (
        "SQLCLEAN040",
        "Compute category_key in an input CTE, then JOIN categories AS c ON o.category_key = c.category_key.",
    ),
    (
        "SQLCLEAN041",
        "WITH orders AS (...), final AS (SELECT order_id FROM orders) SELECT order_id FROM final.",
    ),
    (
        "SQBRMODEL101",
        "WITH upstream AS (SELECT * FROM __ref(\"domain__stg__entity\")), ...",
    ),
    (
        "SQBRMODEL102",
        "SELECT id, status FROM final; use a lone SELECT * only when allowed.",
    ),
    (
        "SQBRMODEL103",
        "A view uses stg_v, int_v, or mart_v; a table uses a non-v layer.",
    ),
    (
        "SQBRMODEL104",
        "MODEL() omits schema; central path defaults provide it.",
    ),
    (
        "SQBRGRAPH101",
        "Internal flow is stg -> int_clean -> int_enriched; marts are published interfaces.",
    ),
    (
        "SQBRGRAPH103",
        "sales__mart__orders and sales__mart__orders__finance both read sales__int_enriched__orders; sales__mart_v__orders may read sales__mart__orders.",
    ),
    (
        "SQBRPROJECT101",
        "domain__int_clean__entity or domain__mart_v__entity.",
    ),
    (
        "SQBRPROJECT107",
        "sales publishes sales__mart__orders and sales__mart__customers; the finance variant lives in finance or sales intermediate.",
    ),
    (
        "SQLCLEAN038",
        "JOIN customers AS c ON o.customer_id = c.customer_id, or CROSS JOIN a provably one-row relation such as a one-row seed or an aggregate without GROUP BY.",
    ),
    (
        "SQBRPROJECT105",
        "A domain__int_enriched__entity model lives under intermediate/enriched.",
    ),
    (
        "SQBRPROJECT106",
        "A domain__stg__entity model resolves to the staging logical schema.",
    ),
    (
        "SQBRCONTRACT101",
        "MODEL (contract enforced, columns (...)); declare authoritative columns.",
    ),
    (
        "SQBRPROJECT202",
        "domain/level/owner contains models or child owners, never both.",
    ),
    (
        "SQBRPROJECT201",
        "models/<domain>/<configured-level>/model.sql resolves one domain root.",
    ),
    (
        "SQBRPROJECT203",
        "domain/level/subdomain/model.sql at the default depth of one.",
    ),
    (
        "SQBRPROJECT204",
        "order_status and order_status_history consolidate under order_status.",
    ),
    (
        "SQBRDECLARATION302",
        "_macros/ is flat, or every file uses one concern bucket.",
    ),
    (
        "SQBRDECLARATION303",
        "_macros/normalisation/names.py uses one default bucket level.",
    ),
    (
        "SQBRDECLARATION304",
        "Keep each flat role or concern bucket within its file cap.",
    ),
    (
        "SQBRDECLARATION305",
        "Use temporal/ or scoring/, not utils/ or common/.",
    ),
    (
        "SQBRDECLARATION306",
        "Group normalise_order.py and normalise_person.py under normalise/.",
    ),
    (
        "SQBRTEST101",
        "Keep unit tests in tests/unit/ and scenarios in tests/scenarios/.",
    ),
    (
        "SQBRTEST102",
        "test_stg_orders__excludes_cancelled.sql and daily_revenue__minimal.sql.",
    ),
    (
        "SQBRTEST103",
        "models/staging/stg_orders.sql maps to tests/unit/staging/.",
    ),
    (
        "SQBRTEST104",
        "TEST (name \"stg_orders__excludes_cancelled_orders\");",
    ),
    (
        "SQBRTEST201",
        "Attach not_null/unique audits to the model key.",
    ),
];
