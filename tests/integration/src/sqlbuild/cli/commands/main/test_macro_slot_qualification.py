"""Real CLI coverage for how SQLCLEAN028 judges the qualification of SQL macro slot reads."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    MacroSlotQualificationTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import refund_rates_project

_PROJECT: str = (
    'name = "shop"\n'
    'adapter = "duckdb"\n'
    'default_target = "dev"\n\n'
    "[connections.local]\n"
    'database = "shop.duckdb"\n\n'
    "[targets.dev]\n"
    'connection = "local"\n'
    'schema = "dev"\n\n'
    "[rules]\n"
    'select = ["SQLCLEAN028"]\n'
)


@pytest.mark.parametrize(
    "test_case",
    [
        MacroSlotQualificationTestCase(
            description="a first step written in plain slot reads passes",
            files=refund_rates_project(
                "  SELECT\n"
                "    @weekly.keys,\n"
                "    @weekly.order_count AS orders,\n"
                "    @weekly.refund_count AS refunds\n"
                "  FROM @weekly"
            ),
            expected_exit_code=0,
            expected_fragments=("Project compiled",),
            absent_fragments=("SQLCLEAN028",),
        ),
        MacroSlotQualificationTestCase(
            description="aliased slot reads beside a plain group read fail once, at the macro",
            files=refund_rates_project(
                "  SELECT\n"
                "    @weekly.keys,\n"
                "    s.@weekly.order_count AS orders,\n"
                "    s.@weekly.refund_count AS refunds\n"
                "  FROM @weekly AS s"
            ),
            expected_exit_code=1,
            expected_fragments=(
                "--> models/_macros/sql/weekly_rates.sql:8:5",
                "to qualify it, read through an alias: `FROM @rel AS s`, then `s.@rel.column`",
            ),
            absent_fragments=("--> models/refund_rates.sql",),
        ),
        MacroSlotQualificationTestCase(
            description="an expression macro's plain reads pass beside bare caller columns",
            files={
                "models/orders.sql": (
                    'MODEL (\n  description "One row per order",\n  materialized table,\n);\n\n'
                    "SELECT 1 AS order_id, CAST(10 AS DOUBLE) AS amount, "
                    "CAST(1 AS DOUBLE) AS discount\n"
                ),
                "models/_macros/sql/net_amount.sql": (
                    'MACRO (\n  description "Order amount after discount",\n'
                    "  arguments (orders RELATION (amount DOUBLE, discount DOUBLE)),\n);\n\n"
                    "@orders.amount - @orders.discount\n"
                ),
                "models/order_totals.sql": (
                    'MODEL (\n  description "Net amount per order",\n  materialized table,\n);\n\n'
                    "SELECT order_id, @sql.net_amount(orders) AS net\n"
                    'FROM __ref("orders") AS orders\n'
                ),
            },
            expected_exit_code=0,
            expected_fragments=("Project compiled",),
            absent_fragments=("SQLCLEAN028",),
        ),
        MacroSlotQualificationTestCase(
            description="an expression macro's plain reads pass beside hand-qualified columns",
            files={
                "models/orders.sql": (
                    'MODEL (\n  description "One row per order",\n  materialized table,\n);\n\n'
                    "SELECT 1 AS order_id, CAST(10 AS DOUBLE) AS amount, "
                    "CAST(1 AS DOUBLE) AS discount\n"
                ),
                "models/_macros/sql/net_amount.sql": (
                    'MACRO (\n  description "Order amount after discount",\n'
                    "  arguments (orders RELATION (amount DOUBLE, discount DOUBLE)),\n);\n\n"
                    "@orders.amount - @orders.discount\n"
                ),
                "models/order_totals.sql": (
                    'MODEL (\n  description "Net amount per order",\n  materialized table,\n);\n\n'
                    "SELECT o.order_id, @sql.net_amount(o) AS net\n"
                    'FROM __ref("orders") AS o\n'
                ),
            },
            expected_exit_code=0,
            expected_fragments=("Project compiled",),
            absent_fragments=("SQLCLEAN028",),
        ),
        MacroSlotQualificationTestCase(
            description="an expression macro's bare column beside a hand-qualified column fails",
            files={
                "models/orders.sql": (
                    'MODEL (\n  description "One row per order",\n  materialized table,\n);\n\n'
                    "SELECT 1 AS order_id, 'open' AS status\n"
                ),
                "models/_macros/sql/status_value.sql": (
                    'MACRO (\n  description "The order status",\n  returns VARCHAR,\n);\n\nstatus\n'
                ),
                "models/open_orders.sql": (
                    'MODEL (\n  description "Open orders",\n  materialized table,\n);\n\n'
                    'SELECT o.order_id, @sql.status_value() AS s FROM __ref("orders") AS o\n'
                ),
            },
            expected_exit_code=1,
            expected_fragments=("error[SQLCLEAN028]", "--> models/open_orders.sql"),
            absent_fragments=(),
        ),
        MacroSlotQualificationTestCase(
            description="a column-list macro's bare output beside a hand-qualified column fails",
            files={
                "models/orders.sql": (
                    'MODEL (\n  description "One row per order",\n  materialized table,\n);\n\n'
                    "SELECT 1 AS order_id, 'open' AS status\n"
                ),
                "models/_macros/sql/order_status.sql": (
                    'MACRO (\n  description "The order status column",\n);\n\nSELECT status\n'
                ),
                "models/open_orders.sql": (
                    'MODEL (\n  description "Open orders",\n  materialized table,\n);\n\n'
                    'SELECT o.order_id, @sql.order_status() FROM __ref("orders") AS o\n'
                ),
            },
            expected_exit_code=1,
            expected_fragments=("error[SQLCLEAN028]", "--> models/open_orders.sql"),
            absent_fragments=(),
        ),
        MacroSlotQualificationTestCase(
            description="a caller that mixes styles itself still fails",
            files={
                "models/orders.sql": (
                    'MODEL (\n  description "One row per order",\n  materialized table,\n);\n\n'
                    "SELECT 1 AS order_id, 'open' AS status\n"
                ),
                "models/open_orders.sql": (
                    'MODEL (\n  description "Open orders",\n  materialized table,\n);\n\n'
                    'SELECT o.order_id, status FROM __ref("orders") AS o\n'
                ),
            },
            expected_exit_code=1,
            expected_fragments=("error[SQLCLEAN028]", "--> models/open_orders.sql"),
            absent_fragments=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_macro_slot_reads_when_compiling_then_qualification_is_judged_as_written(
    test_case: MacroSlotQualificationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    files: dict[str, str] = {"sqlbuild_project.toml": _PROJECT, **test_case.files}
    for relative, contents in files.items():
        path: Path = tmp_path / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")

    exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), "compile"])

    output: str = "".join(capsys.readouterr())
    assert exit_code == test_case.expected_exit_code, output
    assert [fragment in output for fragment in test_case.expected_fragments] == [
        True for _ in test_case.expected_fragments
    ], output
    assert [fragment in output for fragment in test_case.absent_fragments] == [
        False for _ in test_case.absent_fragments
    ], output
