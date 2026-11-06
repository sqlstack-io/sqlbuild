"""E2E coverage of SQLCLEAN038 inferring that one side of a CROSS JOIN holds at most one row."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.rules._test_types import OneRowCrossJoinCase
from tests.e2e.src.sqlbuild.cli.commands.main.rules.helpers import (
    diagnostic_messages,
    run_compile_cli,
    write_one_row_cross_join_project,
)

_PROJECT_TOML: str = 'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQLCLEAN038"]\n'
_SEED_YAML: str = (
    "seeds:\n"
    "  - name: order_policy\n"
    "    description: Order limits applied to every order.\n"
    "    columns:\n"
    "      - name: max_amount\n"
    "        type: INTEGER\n"
)
_RAW_ORDERS_SQL: str = "MODEL (description 'Raw orders.');\nSELECT 1 AS order_id, 3 AS amount\n"
_HEADER: str = "MODEL (description 'Orders checked against limits.');\n"
_DIRECT_SQL: str = (
    _HEADER + "WITH orders AS (\n"
    '  SELECT o.order_id, o.amount FROM __ref("raw_orders") AS o\n'
    "),\n"
    "final AS (\n"
    "  SELECT o.order_id, p.max_amount\n"
    "  FROM orders AS o\n"
    '  CROSS JOIN __seed("order_policy") AS p\n'
    ")\n"
    "SELECT f.order_id, f.max_amount FROM final AS f\n"
)
_CARRIED_SQL: str = (
    _HEADER + "WITH orders AS (\n"
    '  SELECT o.order_id, o.amount FROM __ref("raw_orders") AS o\n'
    "),\n"
    "policy AS (\n"
    '  SELECT p.max_amount FROM __seed("order_policy") AS p WHERE p.max_amount > 0\n'
    "),\n"
    "final AS (\n"
    "  SELECT o.order_id, p.max_amount FROM orders AS o CROSS JOIN policy AS p\n"
    ")\n"
    "SELECT f.order_id, f.max_amount FROM final AS f\n"
)
_AGGREGATE_SQL: str = (
    _HEADER + "WITH orders AS (\n"
    '  SELECT o.order_id, o.amount FROM __ref("raw_orders") AS o\n'
    "),\n"
    "totals AS (\n"
    "  SELECT SUM(o.amount) AS total_amount FROM orders AS o\n"
    "),\n"
    "final AS (\n"
    "  SELECT o.order_id, t.total_amount FROM orders AS o CROSS JOIN totals AS t\n"
    ")\n"
    "SELECT f.order_id, f.total_amount FROM final AS f\n"
)
_LIMIT_SQL: str = (
    _HEADER + "WITH orders AS (\n"
    '  SELECT o.order_id, o.amount FROM __ref("raw_orders") AS o\n'
    "),\n"
    "largest AS (\n"
    "  SELECT o.amount AS largest_amount FROM orders AS o\n"
    "  ORDER BY o.amount DESC, o.order_id ASC\n"
    "  LIMIT 1\n"
    "),\n"
    "final AS (\n"
    "  SELECT o.order_id, l.largest_amount FROM orders AS o CROSS JOIN largest AS l\n"
    ")\n"
    "SELECT f.order_id, f.largest_amount FROM final AS f\n"
)
_MULTIPLYING: str = (
    "CROSS JOIN with `order_policy` in CTE `final` creates an explicit cartesian product"
)


_SEED_YAML_FILE: tuple[str, str] = ("seeds/order_policy.yml", _SEED_YAML)
_RAW_ORDERS_FILE: tuple[str, str] = ("models/raw_orders.sql", _RAW_ORDERS_SQL)


@pytest.mark.parametrize(
    "test_case",
    [
        OneRowCrossJoinCase(
            description="one-row seed joined directly",
            seed_rows=("100",),
            model_sql=_DIRECT_SQL,
            expected_messages=(),
            expected_exit=0,
        ),
        OneRowCrossJoinCase(
            description="one-row seed carried through a filtering CTE",
            seed_rows=("100",),
            model_sql=_CARRIED_SQL,
            expected_messages=(),
            expected_exit=0,
        ),
        OneRowCrossJoinCase(
            description="aggregate without GROUP BY",
            seed_rows=("100", "200"),
            model_sql=_AGGREGATE_SQL,
            expected_messages=(),
            expected_exit=0,
        ),
        OneRowCrossJoinCase(
            description="LIMIT 1",
            seed_rows=("100", "200"),
            model_sql=_LIMIT_SQL,
            expected_messages=(),
            expected_exit=0,
        ),
        OneRowCrossJoinCase(
            description="two-row seed multiplies rows",
            seed_rows=("100", "200"),
            model_sql=_DIRECT_SQL,
            expected_messages=(_MULTIPLYING,),
            expected_exit=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cross_join_when_compiling_then_only_multiplying_joins_are_reported(
    test_case: OneRowCrossJoinCase,
    tmp_path: Path,
) -> None:
    _ = write_one_row_cross_join_project(
        project_dir=tmp_path,
        toml=_PROJECT_TOML,
        seed_rows=test_case.seed_rows,
        files=(
            _SEED_YAML_FILE,
            _RAW_ORDERS_FILE,
            ("models/checked_orders.sql", test_case.model_sql),
        ),
    )

    result: subprocess.CompletedProcess[str] = run_compile_cli(tmp_path)

    assert diagnostic_messages(result, "SQLCLEAN038") == test_case.expected_messages, (
        result.stdout + result.stderr
    )
    assert result.returncode == test_case.expected_exit, result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        OneRowCrossJoinCase(
            description="adding a seed row brings the finding back",
            seed_rows=("100",),
            model_sql=_DIRECT_SQL,
            expected_messages=(_MULTIPLYING,),
            expected_exit=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_one_row_seed_when_a_row_is_added_then_the_cached_compile_reports_the_join(
    test_case: OneRowCrossJoinCase,
    tmp_path: Path,
) -> None:
    seed: Path = write_one_row_cross_join_project(
        project_dir=tmp_path,
        toml=_PROJECT_TOML,
        seed_rows=test_case.seed_rows,
        files=(
            _SEED_YAML_FILE,
            _RAW_ORDERS_FILE,
            ("models/checked_orders.sql", test_case.model_sql),
        ),
    )
    first: subprocess.CompletedProcess[str] = run_compile_cli(tmp_path)
    seed.write_text(seed.read_text(encoding="utf-8") + "200\n", encoding="utf-8")

    second: subprocess.CompletedProcess[str] = run_compile_cli(tmp_path)

    assert first.returncode == 0, first.stdout + first.stderr
    assert diagnostic_messages(first, "SQLCLEAN038") == ()
    assert diagnostic_messages(second, "SQLCLEAN038") == test_case.expected_messages, (
        second.stdout + second.stderr
    )
    assert second.returncode == test_case.expected_exit


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
