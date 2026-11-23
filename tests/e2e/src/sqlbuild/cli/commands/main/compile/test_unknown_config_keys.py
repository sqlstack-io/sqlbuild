"""E2E coverage for rejecting unknown MODEL header, model config layer, and source keys and values."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    ModelHeaderKeyCompileCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT_TOML: str = 'name = "orders"\nadapter = "duckdb"\n'


@pytest.mark.parametrize(
    "test_case",
    [
        ModelHeaderKeyCompileCase(
            description="misspelled MODEL key",
            repo_files={
                "sqlbuild_project.toml": _PROJECT_TOML,
                "sql/models/stg_orders.sql": "MODEL (description 'Test model stg_orders.', materialized view);\n\nSELECT 1 AS order_id\n",
                "sql/models/fct_orders.sql": (
                    'MODEL (description "Test model fct_orders.",\n  materialized table,\n  descripton "Orders fact.",\n);\n\n'
                    'SELECT order_id FROM __ref("stg_orders")\n'
                ),
            },
            expected_fragments=(
                "error[D002]",
                "sql/models/fct_orders.sql:3' has unsupported keys: descripton",
                "did you mean 'description'?",
            ),
        ),
        ModelHeaderKeyCompileCase(
            description="misspelled path default key",
            repo_files={
                "sqlbuild_project.toml": (
                    _PROJECT_TOML + '\n[path_defaults.staging]\nmaterialised = "view"\n'
                ),
                "sql/models/staging/stg_orders.sql": "MODEL (description 'Test model stg_orders.');\n\nSELECT 1 AS order_id\n",
            },
            expected_fragments=(
                "error[D001]",
                "path_defaults['staging'] contains unknown key(s): materialised",
                "did you mean 'materialized'?",
            ),
        ),
        ModelHeaderKeyCompileCase(
            description="misspelled source column key",
            repo_files={
                "sqlbuild_project.toml": _PROJECT_TOML,
                "sql/sources/raw.yml": (
                    "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n    expression: SELECT 1 AS order_id\n"
                    "    columns:\n      - name: order_id\n        tpye: INTEGER\n"
                ),
                "sql/models/stg_orders.sql": (
                    'MODEL (description "Test model stg_orders.");\n\nSELECT order_id FROM __source("raw_orders")\n'
                ),
            },
            expected_fragments=(
                "error[D006]",
                "source column has unknown keys: tpye",
                "did you mean 'type'?",
            ),
        ),
        ModelHeaderKeyCompileCase(
            description="unknown on_schema_change in MODEL header",
            repo_files={
                "sqlbuild_project.toml": _PROJECT_TOML,
                "sql/models/fct_orders.sql": (
                    "MODEL (description 'Test model fct_orders.',\n  materialized incremental,\n  incremental_strategy append,\n"
                    "  on_schema_change append_columns,\n);\n\nSELECT 1 AS order_id\n"
                ),
            },
            expected_fragments=(
                "error[P001]",
                "unknown on_schema_change 'append_columns'; valid values: append_new_columns, "
                "fail, ignore, sync_all_columns",
                "on_schema_change append_new_columns,",
            ),
        ),
        ModelHeaderKeyCompileCase(
            description="unknown replay_on_change in project defaults",
            repo_files={
                "sqlbuild_project.toml": (
                    _PROJECT_TOML + '\n[defaults]\nreplay_on_change = "everything"\n'
                ),
                "sql/models/fct_orders.sql": (
                    "MODEL (description 'Test model fct_orders.', materialized incremental, incremental_strategy append);\n\n"
                    "SELECT 1 AS order_id\n"
                ),
            },
            expected_fragments=(
                "error[D001]",
                "[defaults] unknown replay_on_change 'everything'; valid values: forward, full, "
                "bounded-<duration>",
                'replay_on_change = "bounded-14d"',
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unknown_model_config_key_when_compiling_then_fails_naming_key_and_nearest_key(
    test_case: ModelHeaderKeyCompileCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="orders", repo_files=test_case.repo_files
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode != 0, output
    assert all(fragment in output for fragment in test_case.expected_fragments), output


@pytest.mark.parametrize(
    "test_case",
    [
        ModelHeaderKeyCompileCase(
            description="custom materialization values live under config",
            repo_files={
                "sqlbuild_project.toml": _PROJECT_TOML,
                "python/materializations/order_snapshot.py": (
                    "from sqlbuild.executor.custom.models import (\n"
                    "    MaterializationContext,\n    MaterializationResult,\n)\n\n\n"
                    "def materialize(ctx: MaterializationContext) -> MaterializationResult:\n"
                    "    return MaterializationResult(success=True)\n"
                ),
                "sql/models/order_snapshot.sql": (
                    "MODEL (description 'Test model order_snapshot.',\n  materialized order_snapshot,\n"
                    "  config (retention_days 7, partition_column order_id),\n);\n\n"
                    "SELECT 1 AS order_id\n"
                ),
            },
            expected_fragments=("Project compiled  1 model",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_custom_materialization_config_block_when_compiling_then_arbitrary_keys_pass(
    test_case: ModelHeaderKeyCompileCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="orders", repo_files=test_case.repo_files
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == 0, output
    assert all(fragment in output for fragment in test_case.expected_fragments), output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
