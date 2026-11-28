"""Python macros translated from dbt render Jinja that calls the sibling macros passed to it."""

from __future__ import annotations

from pathlib import Path

import duckdb
import pytest

from sqlbuild._native import run_cli
from tests.integration.src.sqlbuild.integrations.dbt._test_types import DbtMacroBuildTestCase

_PROJECT_MACROS: str = '''from sqlbuild.dbt import jinja as _jinja, override as _override


@_override(replaces="dbt.macro shop.cents_to_dollars")
def cents_to_dollars(column_name, scale=2):
    return _jinja(r"""round({{ column_name }} / 100, {{ scale }})""", column_name=column_name, scale=scale)
'''
_MARTS_MACROS: str = '''from sqlbuild.dbt import jinja as _jinja, override as _override


@_override(replaces="dbt.macro dbt.concat")
def concat(fields):
    return _jinja(r"""{{ return(default__concat(fields)) }}""", fields=fields, default__concat=default__concat)


@_override(replaces="dbt.macro shop.customer_label", reason="converted")
def customer_label(name_column, tier_column):
    return _jinja(r"""{{ concat([name_column, "' ('", tier_column, "')'"]) }}""", name_column=name_column, tier_column=tier_column, concat=concat)


@_override(replaces="dbt.macro dbt.default__concat")
def default__concat(fields):
    return _jinja(r"""{{ fields|join(' || ') }}""", fields=fields)
'''
_SHARED_CONCAT: str = '''from sqlbuild.dbt import jinja as _jinja, override as _override


@_override(replaces="dbt.macro dbt.concat")
def concat(fields):
    return _jinja(r"""{% for f in fields %}{% if loop.first %}{% do return(default__concat(fields)) %}{% break %}{% endif %}{% endfor %}""", fields=fields, default__concat=default__concat)


@_override(replaces="dbt.macro dbt.default__concat")
def default__concat(fields):
    return _jinja(r"""{{ fields|join(' || ') }}""", fields=fields)
'''
_MARTS_LABEL: str = '''from sqlbuild.dbt import jinja as _jinja, override as _override
from sql.macros.python.dbt_macros import concat


@_override(replaces="dbt.macro shop.customer_label")
def customer_label(name_column, tier_column):
    return _jinja(r"""{{ concat([name_column, "' ('", tier_column, "')'"]) }}""", name_column=name_column, tier_column=tier_column, concat=concat)
'''


@pytest.mark.parametrize(
    "test_case",
    [
        DbtMacroBuildTestCase(
            description="a label macro calls concat, which returns its dispatched default",
            files={
                "sql/macros/python/dbt_macros.py": _PROJECT_MACROS,
                "sql/models/marts/_sqlbuild/_macros/python/dbt_macros.py": _MARTS_MACROS,
                "sql/models/staging/stg_orders.sql": (
                    "MODEL (description 'Orders.');\n"
                    "WITH raw_orders AS (\n"
                    "  SELECT 1 AS order_id, 1 AS customer_id, 1000 AS amount_cents\n"
                    "  UNION ALL SELECT 2, 2, 2500\n"
                    "  UNION ALL SELECT 3, 1, 700\n"
                    ")\n"
                    "SELECT order_id, customer_id, amount_cents,\n"
                    '  @py.cents_to_dollars("amount_cents", scale=1) AS amount_dollars\n'
                    "FROM raw_orders\n"
                ),
                "sql/models/marts/customer_totals.sql": (
                    "MODEL (description 'Customer totals.', materialized table);\n"
                    "WITH customers AS (\n"
                    "  SELECT 1 AS customer_id, 'Ada' AS customer_name, 'gold' AS tier\n"
                    "  UNION ALL SELECT 2, 'Bo', 'silver'\n"
                    ")\n"
                    "SELECT c.customer_id,\n"
                    '  @py.customer_label("c.customer_name", "c.tier") AS label,\n'
                    "  count(o.order_id) AS order_count,\n"
                    '  @py.cents_to_dollars("sum(o.amount_cents)") AS amount_dollars\n'
                    "FROM customers AS c\n"
                    'LEFT JOIN __ref("stg_orders") AS o ON o.customer_id = c.customer_id\n'
                    "GROUP BY ALL\n"
                ),
            },
            query=(
                "SELECT customer_id, label, order_count, amount_dollars"
                " FROM main.customer_totals ORDER BY customer_id"
            ),
            expected_rows=((1, "Ada (gold)", 2, 17.0), (2, "Bo (silver)", 1, 25.0)),
        ),
        DbtMacroBuildTestCase(
            description="a label macro passes on an imported concat that returns with do",
            files={
                "sql/macros/python/dbt_macros.py": _SHARED_CONCAT,
                "sql/models/marts/_sqlbuild/_macros/python/dbt_macros.py": _MARTS_LABEL,
                "sql/models/staging/stg_customers.sql": (
                    "MODEL (description 'Customers.');\n"
                    "WITH raw_customers AS (\n"
                    "  SELECT 1 AS customer_id, 'Ada' AS customer_name, 'gold' AS tier\n"
                    "  UNION ALL SELECT 2, 'Bo', 'silver'\n"
                    ")\n"
                    "SELECT customer_id, customer_name, tier,\n"
                    '  @py.concat(["customer_name", "tier"]) AS name_tier\n'
                    "FROM raw_customers\n"
                ),
                "sql/models/marts/customer_labels.sql": (
                    "MODEL (description 'Customer labels.', materialized table);\n"
                    "SELECT c.customer_id, c.name_tier,\n"
                    '  @py.customer_label("c.customer_name", "c.tier") AS label\n'
                    'FROM __ref("stg_customers") AS c\n'
                ),
            },
            query=(
                "SELECT customer_id, name_tier, label FROM main.customer_labels"
                " ORDER BY customer_id"
            ),
            expected_rows=((1, "Adagold", "Ada (gold)"), (2, "Bosilver", "Bo (silver)")),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dbt_jinja_macros_when_building_then_passed_sibling_macros_render_in_place(
    test_case: DbtMacroBuildTestCase, tmp_path: Path
) -> None:
    files: dict[str, str] = {
        "sqlbuild_project.toml": (
            'name = "shop"\nadapter = "duckdb"\n[connection]\ndatabase = "shop.duckdb"\n'
        ),
    } | test_case.files
    for path, text in files.items():
        (tmp_path / path).parent.mkdir(parents=True, exist_ok=True)
        _ = (tmp_path / path).write_text(text)

    assert run_cli(["--project-dir", str(tmp_path), "build"]) == 0
    connection: duckdb.DuckDBPyConnection = duckdb.connect(
        str(tmp_path / "shop.duckdb"), read_only=True
    )
    try:
        rows: list[tuple[object, ...]] = connection.execute(test_case.query).fetchall()
    finally:
        connection.close()
    assert tuple(rows) == test_case.expected_rows
