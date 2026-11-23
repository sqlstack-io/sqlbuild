"""Behavior tests for selected built-in rules."""

from dataclasses import replace
from pathlib import Path

import pytest

from sqlbuild._native import CompileSqlReference, Finding, RulesConfig, RulesResult, SelectStarAllow
from sqlbuild._native import evaluate_project_rules as evaluate
from tests.unit.src.sqlbuild.rule_engine.main.evaluate._test_types import (
    PolicyEvaluationTestCase,
)
from tests.unit.src.sqlbuild.rule_engine.main.evaluate.helpers import build_project


@pytest.mark.parametrize(
    "test_case",
    (
        PolicyEvaluationTestCase(
            description="empty selection disables policy",
            model_name="commerce__mart__orders",
            relative_path="sql/models/mart/commerce__mart__orders.sql",
            sql="SELECT * FROM raw.orders",
            config_values={},
            select=(),
            expected_codes=(),
        ),
        PolicyEvaluationTestCase(
            description="contract rule faults missing enforced contract",
            model_name="commerce__mart__orders",
            relative_path="sql/models/mart/commerce__mart__orders.sql",
            sql="WITH orders AS (SELECT id FROM source_orders) SELECT id FROM orders",
            config_values={"materialized": "table"},
            select=("SQBRCONTRACT101",),
            expected_codes=("SQBRCONTRACT101",),
        ),
        PolicyEvaluationTestCase(
            description="direct enum member comparison passes",
            model_name="commerce__int_clean__orders",
            relative_path="sql/models/intermediate/commerce__int_clean__orders.sql",
            sql=(
                'WITH upstream AS (SELECT * FROM __ref("commerce__stg__orders")), '
                "filtered AS (SELECT status FROM upstream WHERE upstream.status = 'active') "
                "SELECT status FROM filtered"
            ),
            authored_sql=(
                'WITH upstream AS (SELECT * FROM __ref("commerce__stg__orders")), '
                "filtered AS (SELECT status FROM upstream "
                'WHERE upstream.status = @enum("status").ACTIVE) SELECT status FROM filtered'
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRDECLARATION101",),
            expected_codes=(),
            references=(CompileSqlReference(ref_kind="ref", ref_name="commerce__stg__orders"),),
            enum_columns=("status",),
        ),
        PolicyEvaluationTestCase(
            description="modified controlled enum column faults",
            model_name="commerce__int_clean__orders",
            relative_path="sql/models/intermediate/commerce__int_clean__orders.sql",
            sql=(
                'WITH upstream AS (SELECT * FROM __ref("commerce__stg__orders")), '
                "filtered AS (SELECT status FROM upstream "
                "WHERE LOWER(upstream.status) = 'active') SELECT status FROM filtered"
            ),
            authored_sql=(
                'WITH upstream AS (SELECT * FROM __ref("commerce__stg__orders")), '
                "filtered AS (SELECT status FROM upstream "
                'WHERE LOWER(upstream.status) = @enum("status").ACTIVE) '
                "SELECT status FROM filtered"
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRDECLARATION101",),
            expected_codes=("SQBRDECLARATION101",),
            references=(CompileSqlReference(ref_kind="ref", ref_name="commerce__stg__orders"),),
            enum_columns=("status",),
        ),
        PolicyEvaluationTestCase(
            description="uppercased controlled enum column faults",
            model_name="commerce__int_clean__orders",
            relative_path="sql/models/intermediate/commerce__int_clean__orders.sql",
            sql=(
                'SELECT status FROM __ref("commerce__stg__orders") AS upstream '
                "WHERE UPPER(upstream.status) = 'active'"
            ),
            authored_sql=(
                'SELECT status FROM __ref("commerce__stg__orders") AS upstream '
                'WHERE UPPER(upstream.status) = @enum("status").ACTIVE'
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRDECLARATION101",),
            expected_codes=("SQBRDECLARATION101",),
            references=(CompileSqlReference(ref_kind="ref", ref_name="commerce__stg__orders"),),
            enum_columns=("status",),
        ),
        PolicyEvaluationTestCase(
            description="cast controlled enum column faults",
            model_name="commerce__int_clean__orders",
            relative_path="sql/models/intermediate/commerce__int_clean__orders.sql",
            sql=(
                'SELECT status FROM __ref("commerce__stg__orders") AS upstream '
                "WHERE CAST(upstream.status AS VARCHAR) = 'active'"
            ),
            authored_sql=(
                'SELECT status FROM __ref("commerce__stg__orders") AS upstream '
                'WHERE CAST(upstream.status AS VARCHAR) = @enum("status").ACTIVE'
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRDECLARATION101",),
            expected_codes=("SQBRDECLARATION101",),
            references=(CompileSqlReference(ref_kind="ref", ref_name="commerce__stg__orders"),),
            enum_columns=("status",),
        ),
        PolicyEvaluationTestCase(
            description="direct source enum column modifier passes",
            model_name="commerce__stg__orders__vendor",
            relative_path="sql/models/staging/commerce__stg__orders__vendor.sql",
            sql=(
                'WITH raw_orders AS (SELECT * FROM __source("vendor_orders")), '
                "filtered AS (SELECT status FROM raw_orders "
                "WHERE LOWER(raw_orders.status) = 'active') SELECT status FROM filtered"
            ),
            authored_sql=(
                'WITH raw_orders AS (SELECT * FROM __source("vendor_orders")), '
                "filtered AS (SELECT status FROM raw_orders "
                'WHERE LOWER(raw_orders.status) = @enum("status").ACTIVE) '
                "SELECT status FROM filtered"
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRDECLARATION101",),
            expected_codes=(),
            references=(CompileSqlReference(ref_kind="source", ref_name="vendor_orders"),),
            enum_columns=("status",),
        ),
        PolicyEvaluationTestCase(
            description="transitive source enum column modifier faults",
            model_name="commerce__stg__orders__vendor",
            relative_path="sql/models/staging/commerce__stg__orders__vendor.sql",
            sql=(
                'WITH raw_orders AS (SELECT * FROM __source("vendor_orders")), '
                "renamed AS (SELECT status FROM raw_orders), "
                "filtered AS (SELECT status FROM renamed "
                "WHERE LOWER(renamed.status) = 'active') SELECT status FROM filtered"
            ),
            authored_sql=(
                'WITH raw_orders AS (SELECT * FROM __source("vendor_orders")), '
                "renamed AS (SELECT status FROM raw_orders), "
                "filtered AS (SELECT status FROM renamed "
                'WHERE LOWER(renamed.status) = @enum("status").ACTIVE) '
                "SELECT status FROM filtered"
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRDECLARATION101",),
            expected_codes=("SQBRDECLARATION101",),
            references=(CompileSqlReference(ref_kind="source", ref_name="vendor_orders"),),
            enum_columns=("status",),
        ),
        PolicyEvaluationTestCase(
            description="modified enum member faults for direct source comparison",
            model_name="commerce__stg__orders__vendor",
            relative_path="sql/models/staging/commerce__stg__orders__vendor.sql",
            sql=(
                'WITH raw_orders AS (SELECT * FROM __source("vendor_orders")), '
                "filtered AS (SELECT status FROM raw_orders "
                "WHERE raw_orders.status = LOWER('active')) SELECT status FROM filtered"
            ),
            authored_sql=(
                'WITH raw_orders AS (SELECT * FROM __source("vendor_orders")), '
                "filtered AS (SELECT status FROM raw_orders "
                'WHERE raw_orders.status = LOWER(@enum("status").ACTIVE)) '
                "SELECT status FROM filtered"
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRDECLARATION101",),
            expected_codes=("SQBRDECLARATION101",),
            references=(CompileSqlReference(ref_kind="source", ref_name="vendor_orders"),),
            enum_columns=("status",),
        ),
        PolicyEvaluationTestCase(
            description="bare string faults for direct source comparison",
            model_name="commerce__stg__orders__vendor",
            relative_path="sql/models/staging/commerce__stg__orders__vendor.sql",
            sql=(
                'WITH raw_orders AS (SELECT * FROM __source("vendor_orders")), '
                "filtered AS (SELECT status FROM raw_orders "
                "WHERE LOWER(raw_orders.status) = 'active') SELECT status FROM filtered"
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRDECLARATION101",),
            expected_codes=("SQBRDECLARATION101",),
            references=(CompileSqlReference(ref_kind="source", ref_name="vendor_orders"),),
            enum_columns=("status",),
        ),
        PolicyEvaluationTestCase(
            description="numeric decision faults",
            model_name="commerce__mart__orders",
            relative_path="sql/models/mart/commerce__mart__orders.sql",
            sql="WITH filtered AS (SELECT id FROM items WHERE item_count > 7) SELECT id FROM filtered",
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRDECLARATION102",),
            expected_codes=("SQBRDECLARATION102",),
        ),
        PolicyEvaluationTestCase(
            description="numeric between bounds fault",
            model_name="commerce__mart__orders",
            relative_path="sql/models/mart/commerce__mart__orders.sql",
            sql="SELECT id FROM items WHERE item_count BETWEEN 30 AND 61",
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRDECLARATION102",),
            expected_codes=("SQBRDECLARATION102",),
        ),
        PolicyEvaluationTestCase(
            description="numeric value list faults",
            model_name="commerce__mart__orders",
            relative_path="sql/models/mart/commerce__mart__orders.sql",
            sql="SELECT id FROM items WHERE item_count IN (30, 31, 45)",
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRDECLARATION102",),
            expected_codes=("SQBRDECLARATION102",),
        ),
        PolicyEvaluationTestCase(
            description="canonical numeric decision passes",
            model_name="commerce__mart__orders",
            relative_path="sql/models/mart/commerce__mart__orders.sql",
            sql="WITH filtered AS (SELECT id FROM items WHERE item_count > 0) SELECT id FROM filtered",
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRDECLARATION102",),
            expected_codes=(),
        ),
        PolicyEvaluationTestCase(
            description="numeric decision in projected case faults",
            model_name="commerce__mart__orders",
            relative_path="sql/models/mart/commerce__mart__orders.sql",
            sql=(
                "SELECT CASE WHEN item_count > 7 THEN 'large' ELSE 'small' END AS batch_size "
                "FROM items"
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRDECLARATION102",),
            expected_codes=("SQBRDECLARATION102",),
        ),
        PolicyEvaluationTestCase(
            description="enum decision in projected case faults",
            model_name="commerce__mart__orders",
            relative_path="sql/models/mart/commerce__mart__orders.sql",
            sql=(
                "SELECT CASE WHEN status = 'active' THEN 1 ELSE 0 END AS is_win "
                'FROM __ref("commerce__stg__orders")'
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRDECLARATION101",),
            expected_codes=("SQBRDECLARATION101",),
            references=(CompileSqlReference(ref_kind="ref", ref_name="commerce__stg__orders"),),
            enum_columns=("status",),
        ),
        PolicyEvaluationTestCase(
            description="constant-backed numeric decision in projected case passes",
            model_name="commerce__mart__orders",
            relative_path="sql/models/mart/commerce__mart__orders.sql",
            sql=(
                "SELECT CASE WHEN item_count > 7 THEN 'large' ELSE 'small' END AS batch_size "
                "FROM items"
            ),
            authored_sql=(
                'SELECT CASE WHEN item_count > @const("large_batch") '
                "THEN 'large' ELSE 'small' END AS batch_size FROM items"
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRDECLARATION102",),
            expected_codes=(),
        ),
        PolicyEvaluationTestCase(
            description="cast precision does not authorize expanded constant value",
            model_name="commerce__mart__orders",
            relative_path="sql/models/mart/commerce__mart__orders.sql",
            sql=(
                "SELECT CAST(CASE WHEN item_count > 38 THEN 1 ELSE 0 END "
                "AS NUMBER(38, 0)) AS batch_size FROM items"
            ),
            authored_sql=(
                'SELECT CAST(CASE WHEN item_count > @const("large_batch") THEN 1 ELSE 0 END '
                "AS NUMBER(38, 0)) AS batch_size FROM items"
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRDECLARATION102",),
            expected_codes=(),
        ),
        PolicyEvaluationTestCase(
            description="authored numeric decision still faults when it matches cast precision",
            model_name="commerce__mart__orders",
            relative_path="sql/models/mart/commerce__mart__orders.sql",
            sql=(
                "SELECT CAST(CASE WHEN item_count > 38 THEN 1 ELSE 0 END "
                "AS NUMBER(38, 0)) AS batch_size FROM items"
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRDECLARATION102",),
            expected_codes=("SQBRDECLARATION102",),
        ),
        PolicyEvaluationTestCase(
            description="constant-backed numeric decision ignores digits in identifiers and strings",
            model_name="commerce__mart__orders",
            relative_path="sql/models/mart/commerce__mart__orders.sql",
            sql=(
                "SELECT CASE WHEN item_count > 7 THEN 'tier 7' ELSE 'small' END AS batch_size_7 "
                "FROM items"
            ),
            authored_sql=(
                'SELECT CASE WHEN item_count > @const("large_batch") '
                "THEN 'tier 7' ELSE 'small' END AS batch_size_7 FROM items"
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRDECLARATION102",),
            expected_codes=(),
        ),
        PolicyEvaluationTestCase(
            description="query-only numeric decision with model alias faults",
            model_name="commerce__mart__orders",
            relative_path="sql/models/mart/commerce__mart__orders.sql",
            sql=(
                "SELECT CASE WHEN item_count > 7 THEN 'large' ELSE 'small' END AS model FROM items;"
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRDECLARATION102",),
            expected_codes=("SQBRDECLARATION102",),
        ),
        PolicyEvaluationTestCase(
            description="negative one and one numeric decisions pass",
            model_name="commerce__mart__orders",
            relative_path="sql/models/mart/commerce__mart__orders.sql",
            sql=("SELECT id FROM items WHERE previous_rank = -1 OR current_rank = 1"),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRDECLARATION102",),
            expected_codes=(),
        ),
        PolicyEvaluationTestCase(
            description="projected boolean comparison outside case passes",
            model_name="commerce__mart__orders",
            relative_path="sql/models/mart/commerce__mart__orders.sql",
            sql="SELECT item_count > 7 AS is_large_batch FROM items",
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRDECLARATION102",),
            expected_codes=(),
        ),
        PolicyEvaluationTestCase(
            description="comparison-valued case result is not a decision site",
            model_name="commerce__mart__orders",
            relative_path="sql/models/mart/commerce__mart__orders.sql",
            sql=(
                "SELECT CASE WHEN enabled THEN item_count > 7 ELSE FALSE END AS is_large FROM items"
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRDECLARATION102",),
            expected_codes=(),
        ),
        PolicyEvaluationTestCase(
            description="view marker mismatch faults",
            model_name="commerce__mart_v__orders",
            relative_path="sql/models/mart/commerce__mart_v__orders.sql",
            sql="WITH orders AS (SELECT id FROM source_orders) SELECT id FROM orders",
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRMODEL103",),
            expected_codes=("SQBRMODEL103",),
        ),
        PolicyEvaluationTestCase(
            description="retired source token faults",
            model_name="sales__stg__orders__legacy_partner",
            relative_path="sql/models/staging/sales__stg__orders__legacy_partner.sql",
            sql="WITH orders AS (SELECT id FROM source_orders) SELECT id FROM orders",
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRPROJECT103",),
            expected_codes=("SQBRPROJECT103",),
            rules_config=RulesConfig(
                retired_source_tokens={"legacy_partner": "replacement_partner"}
            ),
        ),
        PolicyEvaluationTestCase(
            description="function and seed references are not source tokens",
            model_name="sales__stg__orders__partner",
            relative_path="sql/models/staging/sales__stg__orders__partner.sql",
            sql="WITH orders AS (SELECT id FROM source_orders) SELECT id FROM orders",
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRPROJECT103",),
            expected_codes=(),
            rules_config=RulesConfig(approved_source_tokens=("partner",)),
            references=(
                CompileSqlReference(ref_kind="seed", ref_name="order_statuses"),
                CompileSqlReference(ref_kind="udf", ref_name="normalize_quantity"),
                CompileSqlReference(ref_kind="table_fn", ref_name="expand_order"),
            ),
        ),
        PolicyEvaluationTestCase(
            description="lone star exemption passes",
            model_name="commerce__mart_v__orders",
            relative_path="sql/models/mart/commerce__mart_v__orders.sql",
            sql="WITH orders AS (SELECT id FROM source_orders) SELECT * FROM orders",
            config_values={"materialized": "view", "contract": "enforced"},
            select=("SQBRMODEL102",),
            expected_codes=(),
            rules_config=RulesConfig(
                select_star_allow=(
                    SelectStarAllow(
                        paths=("sql/models/mart/*.sql",),
                        reason="Intentional passthrough view",
                    ),
                )
            ),
        ),
        PolicyEvaluationTestCase(
            description="lone star over dynamic pivot passes",
            model_name="commerce__mart__order_totals",
            relative_path="sql/models/mart/commerce__mart__order_totals.sql",
            sql=(
                "SELECT * FROM source_orders "
                "PIVOT(SUM(amount) FOR category IN (ANY ORDER BY category))"
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRMODEL102",),
            expected_codes=(),
        ),
        PolicyEvaluationTestCase(
            description="lone star over static pivot faults",
            model_name="commerce__mart__order_totals",
            relative_path="sql/models/mart/commerce__mart__order_totals.sql",
            sql=(
                "SELECT * FROM source_orders "
                "PIVOT(SUM(amount) FOR category IN ('standard', 'priority'))"
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRMODEL102",),
            expected_codes=("SQBRMODEL102",),
        ),
        PolicyEvaluationTestCase(
            description="mixed star over dynamic pivot faults",
            model_name="commerce__mart__order_totals",
            relative_path="sql/models/mart/commerce__mart__order_totals.sql",
            sql=(
                "SELECT *, 1 AS marker FROM source_orders "
                "PIVOT(SUM(amount) FOR category IN (ANY ORDER BY category))"
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRMODEL102",),
            expected_codes=("SQBRMODEL102",),
        ),
        PolicyEvaluationTestCase(
            description="lone star over joined dynamic pivot faults",
            model_name="commerce__mart__order_totals",
            relative_path="sql/models/mart/commerce__mart__order_totals.sql",
            sql=(
                "SELECT * FROM source_orders "
                "PIVOT(SUM(amount) FOR category IN (ANY ORDER BY category)) totals "
                "JOIN customers ON totals.customer_id = customers.customer_id"
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRMODEL102",),
            expected_codes=("SQBRMODEL102",),
        ),
        PolicyEvaluationTestCase(
            description="valid dependency import passes",
            model_name="commerce__mart__orders",
            relative_path="sql/models/mart/commerce__mart__orders.sql",
            sql=(
                'WITH orders AS (SELECT * FROM __ref("commerce__stg__orders")), '
                "final AS (SELECT id FROM orders) SELECT id FROM final"
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRMODEL101",),
            expected_codes=(),
            references=(CompileSqlReference(ref_kind="ref", ref_name="commerce__stg__orders"),),
        ),
        PolicyEvaluationTestCase(
            description="subject folder after intermediate refinement passes",
            model_name="commerce__int_clean__orders",
            relative_path="sql/models/commerce/intermediate/clean/payments/commerce__int_clean__orders.sql",
            sql="SELECT 1 AS order_id",
            config_values={"materialized": "table"},
            select=("SQBRPROJECT102",),
            expected_codes=(),
        ),
        PolicyEvaluationTestCase(
            description="subject folder before complete intermediate refinement passes",
            model_name="commerce__int_clean__orders",
            relative_path="sql/models/commerce/payments/intermediate/clean/commerce__int_clean__orders.sql",
            sql="SELECT 1 AS order_id",
            config_values={"materialized": "table"},
            select=("SQBRPROJECT102",),
            expected_codes=(),
        ),
        PolicyEvaluationTestCase(
            description="commented and quoted dependency text is ignored",
            model_name="commerce__mart__orders",
            relative_path="sql/models/mart/commerce__mart__orders.sql",
            sql=(
                'WITH orders AS (SELECT * FROM __ref("commerce__stg__orders")), '
                "final AS (SELECT id, '__ref(\"ignored_string\")' AS note FROM orders "
                '-- __ref("ignored_line_comment")\n'
                '/* __source("ignored_block_comment") */) '
                "SELECT id FROM final"
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRMODEL101",),
            expected_codes=(),
            references=(CompileSqlReference(ref_kind="ref", ref_name="commerce__stg__orders"),),
        ),
        PolicyEvaluationTestCase(
            description="transformed dependency import faults",
            model_name="commerce__mart__orders",
            relative_path="sql/models/mart/commerce__mart__orders.sql",
            sql=(
                'WITH orders AS (SELECT p.id FROM __ref("commerce__stg__orders") p '
                "JOIN lookup l ON p.id = l.id), final AS (SELECT id FROM orders) "
                "SELECT id FROM final"
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRMODEL101",),
            expected_codes=("SQBRMODEL101",),
            references=(CompileSqlReference(ref_kind="ref", ref_name="commerce__stg__orders"),),
        ),
        PolicyEvaluationTestCase(
            description="duplicate dependency import faults",
            model_name="commerce__mart__orders",
            relative_path="sql/models/mart/commerce__mart__orders.sql",
            sql=(
                'WITH orders_a AS (SELECT * FROM __ref("commerce__stg__orders")), '
                'orders_b AS (SELECT * FROM __ref("commerce__stg__orders")), '
                "final AS (SELECT id FROM orders_a) SELECT id FROM final"
            ),
            config_values={"materialized": "table", "contract": "enforced"},
            select=("SQBRMODEL101",),
            expected_codes=("SQBRMODEL101",),
            references=(CompileSqlReference(ref_kind="ref", ref_name="commerce__stg__orders"),),
        ),
        PolicyEvaluationTestCase(
            description="passthrough skips minimum checks",
            model_name="commerce__mart_v__orders",
            relative_path="sql/models/mart/commerce__mart_v__orders.sql",
            sql=(
                'WITH orders AS (SELECT * FROM __ref("commerce__stg__orders")) SELECT * FROM orders'
            ),
            config_values={"materialized": "view"},
            select=("SQBRTEST201", "SQBRTEST202"),
            expected_codes=(),
            references=(CompileSqlReference(ref_kind="ref", ref_name="commerce__stg__orders"),),
        ),
        PolicyEvaluationTestCase(
            description="plain column passthrough shares import and exemption classification",
            model_name="commerce__mart_v__orders",
            relative_path="sql/models/mart/commerce__mart_v__orders.sql",
            sql=(
                'WITH orders AS (SELECT * FROM __ref("commerce__stg__orders")) '
                "SELECT id, price AS current_price FROM orders"
            ),
            config_values={"materialized": "view"},
            select=("SQBRMODEL101", "SQBRMODEL102", "SQBRTEST201", "SQBRTEST202"),
            expected_codes=(),
            references=(CompileSqlReference(ref_kind="ref", ref_name="commerce__stg__orders"),),
        ),
        PolicyEvaluationTestCase(
            description="join import is neither star-exempt nor passthrough",
            model_name="commerce__mart_v__orders",
            relative_path="sql/models/mart/commerce__mart_v__orders.sql",
            sql=(
                'WITH orders AS (SELECT * FROM __ref("commerce__stg__orders") p '
                "JOIN lookup l ON p.id = l.id) SELECT id FROM orders"
            ),
            config_values={"materialized": "view"},
            select=("SQBRMODEL102", "SQBRTEST201", "SQBRTEST202"),
            expected_codes=("SQBRMODEL102", "SQBRTEST201", "SQBRTEST202"),
            references=(CompileSqlReference(ref_kind="ref", ref_name="commerce__stg__orders"),),
        ),
        PolicyEvaluationTestCase(
            description="aggregate projection is not passthrough",
            model_name="commerce__mart_v__orders",
            relative_path="sql/models/mart/commerce__mart_v__orders.sql",
            sql=(
                'WITH orders AS (SELECT * FROM __ref("commerce__stg__orders")) '
                "SELECT COUNT(id) AS price_count FROM orders"
            ),
            config_values={"materialized": "view"},
            select=("SQBRTEST201", "SQBRTEST202"),
            expected_codes=("SQBRTEST201", "SQBRTEST202"),
            references=(CompileSqlReference(ref_kind="ref", ref_name="commerce__stg__orders"),),
        ),
        PolicyEvaluationTestCase(
            description="case projection is not passthrough",
            model_name="commerce__mart_v__orders",
            relative_path="sql/models/mart/commerce__mart_v__orders.sql",
            sql=(
                'WITH orders AS (SELECT * FROM __ref("commerce__stg__orders")) '
                "SELECT CASE WHEN price > 0 THEN id ELSE NULL END AS id FROM orders"
            ),
            config_values={"materialized": "view"},
            select=("SQBRTEST201", "SQBRTEST202"),
            expected_codes=("SQBRTEST201", "SQBRTEST202"),
            references=(CompileSqlReference(ref_kind="ref", ref_name="commerce__stg__orders"),),
        ),
        PolicyEvaluationTestCase(
            description="extra logical cte is not passthrough",
            model_name="commerce__mart_v__orders",
            relative_path="sql/models/mart/commerce__mart_v__orders.sql",
            sql=(
                'WITH orders AS (SELECT * FROM __ref("commerce__stg__orders")), '
                "renamed AS (SELECT id FROM orders) SELECT id FROM renamed"
            ),
            config_values={"materialized": "view"},
            select=("SQBRTEST201", "SQBRTEST202"),
            expected_codes=("SQBRTEST201", "SQBRTEST202"),
            references=(CompileSqlReference(ref_kind="ref", ref_name="commerce__stg__orders"),),
        ),
        PolicyEvaluationTestCase(
            description="extra dependency is not passthrough",
            model_name="commerce__mart_v__orders",
            relative_path="sql/models/mart/commerce__mart_v__orders.sql",
            sql=(
                'WITH orders AS (SELECT * FROM __ref("commerce__stg__orders")), '
                'inventory AS (SELECT * FROM __ref("commerce__stg__inventory")) '
                "SELECT id FROM orders"
            ),
            config_values={"materialized": "view"},
            select=("SQBRTEST201", "SQBRTEST202"),
            expected_codes=("SQBRTEST201", "SQBRTEST202"),
            references=(
                CompileSqlReference(ref_kind="ref", ref_name="commerce__stg__orders"),
                CompileSqlReference(ref_kind="ref", ref_name="commerce__stg__inventory"),
            ),
        ),
        PolicyEvaluationTestCase(
            description="derived expression is not passthrough",
            model_name="commerce__mart_v__orders",
            relative_path="sql/models/mart/commerce__mart_v__orders.sql",
            sql=(
                'WITH orders AS (SELECT * FROM __ref("commerce__stg__orders")) '
                "SELECT price * 100 AS price_cents FROM orders"
            ),
            config_values={"materialized": "view"},
            select=("SQBRTEST201", "SQBRTEST202"),
            expected_codes=("SQBRTEST201", "SQBRTEST202"),
            references=(CompileSqlReference(ref_kind="ref", ref_name="commerce__stg__orders"),),
        ),
        PolicyEvaluationTestCase(
            description="nontrivial filter is not passthrough",
            model_name="commerce__mart_v__orders",
            relative_path="sql/models/mart/commerce__mart_v__orders.sql",
            sql=(
                'WITH orders AS (SELECT * FROM __ref("commerce__stg__orders")) '
                "SELECT id FROM orders WHERE price > 0"
            ),
            config_values={"materialized": "view"},
            select=("SQBRTEST201", "SQBRTEST202"),
            expected_codes=("SQBRTEST201", "SQBRTEST202"),
            references=(CompileSqlReference(ref_kind="ref", ref_name="commerce__stg__orders"),),
        ),
        PolicyEvaluationTestCase(
            description="unrelated terminal source is not passthrough",
            model_name="commerce__mart_v__orders",
            relative_path="sql/models/mart/commerce__mart_v__orders.sql",
            sql=(
                'WITH orders AS (SELECT * FROM __ref("commerce__stg__orders")) '
                "SELECT id FROM unrelated"
            ),
            config_values={"materialized": "view"},
            select=("SQBRTEST201", "SQBRTEST202"),
            expected_codes=("SQBRTEST201", "SQBRTEST202"),
            references=(CompileSqlReference(ref_kind="ref", ref_name="commerce__stg__orders"),),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_selected_rules_when_evaluating_then_reports_expected_faults(
    tmp_path: Path, test_case: PolicyEvaluationTestCase
) -> None:
    result: RulesResult = evaluate(
        project=build_project(
            name=test_case.model_name,
            relative_path=test_case.relative_path,
            sql=test_case.sql,
            config_values=test_case.config_values,
            references=test_case.references,
            authored_sql=test_case.authored_sql,
            enum_columns=test_case.enum_columns,
        ),
        config=replace(test_case.rules_config, select=test_case.select),
        project_dir=tmp_path,
    )

    assert tuple(sorted(fault.code for fault in result.findings)) == test_case.expected_codes


@pytest.mark.parametrize(
    "test_case",
    (
        PolicyEvaluationTestCase(
            description="architecture-named subject folder before intermediate refinement",
            model_name="commerce__int_clean__orders",
            relative_path="sql/models/commerce/intermediate/mart/clean/commerce__int_clean__orders.sql",
            sql="SELECT 1 AS order_id",
            config_values={"materialized": "table"},
            select=("SQBRPROJECT102",),
            expected_codes=("SQBRPROJECT102",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_subject_before_layer_when_evaluating_then_reports_canonical_path(
    tmp_path: Path, test_case: PolicyEvaluationTestCase
) -> None:
    result: RulesResult = evaluate(
        project=build_project(
            name=test_case.model_name,
            relative_path=test_case.relative_path,
            sql=test_case.sql,
            config_values=test_case.config_values,
        ),
        config=replace(test_case.rules_config, select=test_case.select),
        project_dir=tmp_path,
    )

    assert len(result.findings) == 1
    finding: Finding = result.findings[0]
    assert finding.code == test_case.expected_codes[0]
    assert finding.message == (
        'int_clean model "commerce__int_clean__orders" must keep layer folder '
        '"intermediate/clean" contiguous'
    )
    assert finding.remediation == (
        'Move the model to "sql/models/commerce/intermediate/clean/mart/'
        'commerce__int_clean__orders.sql"; preserve subject folders outside '
        '"intermediate/clean". Use rules.layout with SQBRPROJECT201-204 to enforce '
        "domain and subdomain ordering."
    )
