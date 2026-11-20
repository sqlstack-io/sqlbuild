"""E2E coverage of dynamic models: which adapters and headers compile, and the errors they get."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    DynamicCompileE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT_TOML: dict[str, str] = {
    "snowflake": (
        'name = "shop"\nadapter = "snowflake"\n\n[connection]\naccount = "example"\n'
        'user = "builder"\n\n[defaults]\ndatabase = "ANALYTICS"\nschema = "MARTS"\n\n'
        "[scopes]\nenforce_placement = false\n"
    ),
    "duckdb": (
        'name = "shop"\nadapter = "duckdb"\n\n[connection]\npath = "shop.duckdb"\n\n'
        "[scopes]\nenforce_placement = false\n"
    ),
}
_ORDERS: str = (
    "MODEL (description 'Orders.', materialized table);\nSELECT 1 AS order_id, 10 AS amount\n"
)
_CLEAN_ORDERS: str = (
    "MODEL (description 'Orders with an amount.', materialized dynamic);\n"
    'SELECT order_id, amount FROM __ref("orders") WHERE amount > 0\n'
)
_DAILY_TOTALS_QUERY: str = 'SELECT SUM(amount) AS total FROM __ref("clean_orders")\n'


@pytest.mark.parametrize(
    "test_case",
    [
        DynamicCompileE2ETestCase(
            description="a chain refreshed by the lag of its last model compiles on Snowflake",
            adapter="snowflake",
            daily_totals_header="materialized dynamic, refresh (lag 1h), refresh_mode full",
            expected_exit_code=0,
            expected_fragments=("Project compiled  3 models",),
        ),
        DynamicCompileE2ETestCase(
            description="dynamic models are refused on DuckDB",
            adapter="duckdb",
            daily_totals_header="materialized dynamic, refresh (lag 1h)",
            expected_exit_code=1,
            expected_fragments=(
                "[A013]",
                "materialized dynamic is not supported on the duckdb adapter; "
                "adapters that support it: snowflake",
            ),
        ),
        DynamicCompileE2ETestCase(
            description="materialized_view on Snowflake points to dynamic",
            adapter="snowflake",
            daily_totals_header="materialized materialized_view",
            expected_exit_code=1,
            expected_fragments=(
                "[A014]",
                "Snowflake has no materialized_view materialization; use materialized "
                "dynamic, which builds a dynamic table",
            ),
        ),
        DynamicCompileE2ETestCase(
            description="a lag below Snowflake's minimum",
            adapter="snowflake",
            daily_totals_header="materialized dynamic, refresh (lag 30s)",
            expected_exit_code=1,
            expected_fragments=(
                "[A015]",
                "refresh lag on Snowflake must be at least 1m; got '30s'",
            ),
        ),
        DynamicCompileE2ETestCase(
            description="a last dynamic model with no refresh would never refresh",
            adapter="snowflake",
            daily_totals_header="materialized dynamic",
            expected_exit_code=1,
            expected_fragments=(
                "[A016]",
                "dynamic model 'daily_totals' has no refresh and no dynamic model reads "
                "it, so Snowflake would never refresh it; add refresh (lag 1h)",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dynamic_models_when_compiling_then_adapter_rules_apply(
    test_case: DynamicCompileE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_TOML[test_case.adapter],
            "models/orders.sql": _ORDERS,
            "models/clean_orders.sql": _CLEAN_ORDERS,
            "models/daily_totals.sql": (
                f"MODEL (description 'Daily totals.', {test_case.daily_totals_header});\n"
                f"{_DAILY_TOTALS_QUERY}"
            ),
        },
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    output: str = compiled.stdout + compiled.stderr
    assert compiled.returncode == test_case.expected_exit_code, output
    assert all(map(output.__contains__, test_case.expected_fragments)), output
