"""E2E coverage: a warehouse error in a tested model's output fails the unit test."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import (
    OutputEvaluationErrorE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.test.helpers import order_totals_test_files
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    run_sqb,
)

_CONVERSION_ERROR: str = "Numeric value 'cluster::r1' is not recognized"
# Like a warehouse that prunes unread columns: the stored output computes every column and
# fails on the fixture, while an inline comparison of the expected columns alone succeeds.
_PRUNING_WAREHOUSE: str = (
    "from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter\n\n\n"
    "class PruningDuckDbAdapter(DuckDbAdapter):\n"
    '    adapter_name = "pruning_duckdb"\n'
    "    failing_outputs = set()\n\n"
    "    def create_sql_test_output(self, *, connection, sql, name):\n"
    "        self.failing_outputs.add(sql)\n"
    f'        raise RuntimeError("{_CONVERSION_ERROR}")\n\n'
    "    def execute(self, *, connection, sql):\n"
    "        if sql in self.failing_outputs:\n"
    f'            raise RuntimeError("{_CONVERSION_ERROR}")\n'
    "        return super().execute(connection=connection, sql=sql)\n"
)
# A session that cannot keep test outputs at all, while every query runs.
_NO_TEMPORARY_TABLES: str = (
    "from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter\n\n\n"
    "class PruningDuckDbAdapter(DuckDbAdapter):\n"
    '    adapter_name = "pruning_duckdb"\n\n'
    "    def create_sql_test_output(self, *, connection, sql, name):\n"
    '        raise PermissionError("temporary tables are disabled on this session")\n'
)
# A session that keeps one test output and refuses every later one.
_ONE_TEMPORARY_TABLE: str = (
    "from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter\n\n\n"
    "class PruningDuckDbAdapter(DuckDbAdapter):\n"
    '    adapter_name = "pruning_duckdb"\n'
    "    stored = []\n\n"
    "    def create_sql_test_output(self, *, connection, sql, name):\n"
    "        self.stored.append(name)\n"
    "        if len(self.stored) > 1:\n"
    '            raise PermissionError("no more temporary tables in this session")\n'
    "        return super().create_sql_test_output(connection=connection, sql=sql, name=name)\n"
)


_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "pruning_duckdb"\n\n'
        '[connection]\ndatabase = "orders.duckdb"\n\n'
        '[defaults]\nmaterialized = "table"\n'
    ),
    "sql/sources/raw_orders.yml": (
        "sources:\n"
        "  - name: raw_orders\n    description: Test source raw_orders.\n"
        "    schema: main\n"
        "    table: raw_orders\n"
        "    columns:\n"
        "      - name: order_id\n        type: INTEGER\n"
        "      - name: code\n        type: VARCHAR\n"
    ),
    "sql/models/orders.sql": (
        "MODEL (description 'Test model orders.');\n\n"
        'SELECT order_id, TRY_CAST(code AS INTEGER) AS code_number FROM __source("raw_orders")\n'
    ),
    "tests/unit/test_orders.sql": (
        'TEST (name "orders_ids");\n\nWITH\n'
        "__source__raw_orders AS (\n  SELECT 1 AS order_id, 'cluster::r1' AS code\n),\n"
        "__expected__orders AS (\n  SELECT 1 AS order_id\n)\nSELECT 1\n"
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        OutputEvaluationErrorE2ETestCase(
            description="an output the warehouse cannot compute errors the test",
            adapter_source=_PRUNING_WAREHOUSE,
            extra_files={},
            expected_exit_code=1,
            expected_fragments=(
                "test 'orders_ids' encountered an execution error while running 'orders'",
                _CONVERSION_ERROR,
                "PASS=0  FAIL=1",
            ),
            expected_json_fragments=('"error"', _CONVERSION_ERROR),
            unexpected_fragments=("PASS=1  FAIL=0",),
        ),
        OutputEvaluationErrorE2ETestCase(
            description="a session that cannot store outputs still compares inline",
            adapter_source=_NO_TEMPORARY_TABLES,
            extra_files={},
            expected_exit_code=0,
            expected_fragments=("PASS=1  FAIL=0",),
            expected_json_fragments=('"pass"',),
            unexpected_fragments=("encountered an execution error",),
        ),
        OutputEvaluationErrorE2ETestCase(
            description="a computable later output that cannot be stored compares inline",
            adapter_source=_ONE_TEMPORARY_TABLE,
            extra_files=order_totals_test_files("code_number + 1"),
            expected_exit_code=0,
            expected_fragments=("PASS=1  FAIL=0",),
            expected_json_fragments=('"pass"',),
            unexpected_fragments=("encountered an execution error", "does not exist"),
        ),
        OutputEvaluationErrorE2ETestCase(
            description="a later output that fails on its own reports the warehouse error",
            adapter_source=_ONE_TEMPORARY_TABLE,
            extra_files=order_totals_test_files("CAST('cluster::r1' AS INTEGER)"),
            expected_exit_code=1,
            expected_fragments=("Could not convert string 'cluster::r1'", "PASS=0  FAIL=1"),
            expected_json_fragments=('"error"', "Could not convert string"),
            unexpected_fragments=("does not exist",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_output_store_failure_when_testing_then_only_storage_problems_fall_back(
    test_case: OutputEvaluationErrorE2ETestCase, tmp_path: Path
) -> None:
    """An inline comparison of some columns must not hide an error computing the others."""

    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files={
            **_PROJECT_FILES,
            "python/adapters/pruning_duckdb.py": test_case.adapter_source,
            **test_case.extra_files,
        },
    )
    config_path: Path = project_dir / "sqlbuild_project.toml"
    _ = config_path.write_text(
        config_path.read_text(encoding="utf-8").replace(
            '"orders.duckdb"', f'"{(project_dir / "orders.duckdb").as_posix()}"'
        ),
        encoding="utf-8",
    )
    execute_duckdb(
        db_path=project_dir / "orders.duckdb",
        sql="CREATE TABLE raw_orders AS SELECT 1 AS order_id, '1' AS code",
    )

    tested: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test", "--run-on", "warehouse"), project_dir=project_dir
    )
    as_json: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test", "--run-on", "warehouse", "--json"), project_dir=project_dir
    )
    payload: dict[str, Any] = json.loads(as_json.stdout)

    output: str = tested.stdout + tested.stderr
    assert tested.returncode == test_case.expected_exit_code, output
    assert as_json.returncode == test_case.expected_exit_code, as_json.stdout
    assert all(fragment in output for fragment in test_case.expected_fragments), output
    assert not any(fragment in output for fragment in test_case.unexpected_fragments), output
    assert all(fragment in json.dumps(payload) for fragment in test_case.expected_json_fragments), (
        as_json.stdout
    )
