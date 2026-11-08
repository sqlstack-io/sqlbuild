"""E2E coverage of local replay of Snowflake captures converted by SQLQuack's default policy."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.scenario._test_types import (
    SnowflakeLocalConversionTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    SNOWFLAKE_ORDER_IDS_FIXTURE_SQL,
    build_offline_snowflake_project_toml,
    write_snowflake_order_ids_snapshot,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_SOURCES_YML: str = (
    "sources:\n  - name: raw_orders\n    description: Raw orders.\n"
    "    schema: main\n    table: raw_orders\n    columns:\n"
    '      - {name: order_id, type: "NUMBER(38,0)"}\n'
)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeLocalConversionTestCase(
            description="a model reading another converted model replays locally",
            scenario_name="order_sizes__converted",
            model_files=(
                (
                    "models/order_flags.sql",
                    'MODEL (description "Order sizes.");\n'
                    "SELECT IFNULL(order_id, 0) AS order_id,\n"
                    "  IFF(order_id > 0, 'big', 'small') AS order_size\n"
                    'FROM __source("raw_orders")\n',
                ),
                (
                    "models/order_sizes.sql",
                    'MODEL (description "Big orders.");\n'
                    "SELECT order_id, order_size\n"
                    'FROM __ref("order_flags")\n'
                    "WHERE order_size = 'big'\n",
                ),
            ),
            expected_sql="__expected__order_sizes AS (SELECT 1 AS order_id, 'big' AS order_size)",
            expected_exit_code=0,
            expected_stdout_fragments=("PASS=1  FAIL=0  ERROR=0  SKIP=0  TOTAL=1",),
        ),
        SnowflakeLocalConversionTestCase(
            description="a model needing a divergence SQLQuack's default refuses is an error",
            scenario_name="order_sizes__refused",
            model_files=(
                (
                    "models/order_sizes.sql",
                    'MODEL (description "Orders with the conversion time.");\n'
                    "SELECT order_id,\n"
                    "  CONVERT_TIMEZONE('UTC', CURRENT_TIMESTAMP()) AS converted_at\n"
                    'FROM __source("raw_orders")\n',
                ),
            ),
            expected_sql=(
                "__expected__order_sizes AS (\n"
                "  SELECT 1 AS order_id, CURRENT_TIMESTAMP() AS converted_at\n"
                ")"
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "error[X607]",
                "order_sizes",
                "SQ0008",
                "session_timezone_utc",
                "PASS=0  FAIL=0  ERROR=1  SKIP=0  TOTAL=1",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_capture_when_replaying_locally_then_sqlquack_converts_or_refuses(
    test_case: SnowflakeLocalConversionTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snowflake_conversion",
        repo_files={
            "sqlbuild_project.toml": build_offline_snowflake_project_toml(),
            "sources/raw.yml": _SOURCES_YML,
            **dict(test_case.model_files),
            f"tests/scenarios/{test_case.scenario_name}.sql": (
                "SCENARIO (description 'A Snowflake capture replays locally.');\n"
                "WITH\n"
                f"__source__raw_orders AS ({SNOWFLAKE_ORDER_IDS_FIXTURE_SQL}),\n"
                f"{test_case.expected_sql}\n"
                "SELECT 1\n"
            ),
        },
    )
    write_snowflake_order_ids_snapshot(
        project_dir=project_dir, scenario_name=test_case.scenario_name
    )

    replayed: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "test", "--local", test_case.scenario_name),
        project_dir=project_dir,
    )

    assert replayed.returncode == test_case.expected_exit_code, replayed.stdout + replayed.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in replayed.stdout, replayed.stdout


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
