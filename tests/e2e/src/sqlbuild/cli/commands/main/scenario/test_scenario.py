"""E2E tests for sqb scenario test command."""

from __future__ import annotations

import subprocess
from collections import defaultdict
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.scenario._test_types import (
    ScenarioCliE2ETestCase,
    ScenarioLocalCliE2ETestCase,
    ScenarioLocalCommittedSnapshotE2ETestCase,
    ScenarioLocalRetainE2ETestCase,
    ScenarioLocalRuntimeArtifactTestCase,
    ScenarioLocalSnapshotSyncE2ETestCase,
    ScenarioPartialFixtureE2ETestCase,
    ScenarioPythonHooksCliE2ETestCase,
    ScenarioRuntimeArtifactTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    assert_local_duckdb_state,
    assert_runtime_artifact_contains,
    build_partial_fixture_scenario_project_files,
    build_scenario_dependency_chain_project_files,
    build_scenario_project_files,
    build_scenario_python_hooks_project_files,
    list_scenario_relation_names,
    maybe_capture_scenario_snapshot,
    maybe_corrupt_scenario_snapshot_dialect,
    maybe_corrupt_scenario_snapshot_jsonl,
    maybe_write_stale_order_totals_scenario,
    scenario_relation_name_by_suffix,
    scenario_relation_row_count,
    write_committed_order_totals_pass_snapshot,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    prepare_waffle_shop,
    query_duckdb,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioCliE2ETestCase(
            description="unclosed scenario comment names the scenario",
            command=("--no-color", "scenario", "test"),
            expected_exit_code=1,
            expected_stderr_fragments=("SQL scenario contains an unclosed block comment",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_unclosed_scenario_comment_when_running_cli_then_error_names_scenario(
    test_case: ScenarioCliE2ETestCase,
    tmp_path: Path,
) -> None:
    repo_files: dict[str, str] = build_scenario_project_files()
    repo_files["tests/scenarios/order_totals_pass.sql"] = (
        "SCENARIO (description 'Test scenario order_totals_pass.');\nWITH /* unfinished"
    )
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="scenario_comment", repo_files=repo_files
    )
    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )

    assert result.returncode == test_case.expected_exit_code
    for fragment in test_case.expected_stderr_fragments:
        assert fragment in result.stderr


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioPartialFixtureE2ETestCase(
            description="required typed source column with unspecified nullability gets a null",
            repo_files=build_partial_fixture_scenario_project_files(),
            expected_stdout_fragment="PASS=1  FAIL=0  TOTAL=1",
            expected_artifact_fragments=(
                'CAST(NULL AS VARCHAR) AS "status"',
                "__sqlbuild_partial_fixture",
            ),
        ),
        ScenarioPartialFixtureE2ETestCase(
            description="three-model dependency chain preserves helper fixtures and final checks",
            repo_files=build_scenario_dependency_chain_project_files(),
            expected_stdout_fragment="PASS=1  FAIL=0  TOTAL=1",
            expected_artifact_fragments=(
                'CAST(NULL AS VARCHAR) AS "status"',
                "WITH helper AS (SELECT 1 AS order_id)",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_partial_scenario_fixture_when_required_type_is_known_then_typed_null_is_used(
    test_case: ScenarioPartialFixtureE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="partial_fixture_scenario_project",
        repo_files=test_case.repo_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "test"),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    assert test_case.expected_stdout_fragment in result.stdout
    fixture_artifact: Path = (
        project_dir
        / "target"
        / "run"
        / "scenarios"
        / "partial_orders"
        / "fixtures"
        / "source__raw_orders.sql"
    )
    fixture_sql: str = fixture_artifact.read_text(encoding="utf-8")
    for fragment in test_case.expected_artifact_fragments:
        assert fragment in fixture_sql


SCENARIO_LOCAL_DUCKDB_CASE_FIXTURES: tuple[ScenarioLocalRetainE2ETestCase, ...] = (
    ScenarioLocalRetainE2ETestCase(
        description="captured snapshot keeps local DuckDB by default",
        scenario_name="order_totals_pass",
        capture_command=("--no-color", "scenario", "capture", "order_totals_pass"),
        command=("--no-color", "scenario", "test", "order_totals_pass", "--local"),
        expected_exit_code=0,
        expected_stdout_fragments=(
            "order_totals_pass",
            "Retained local DuckDB:",
            "PASS=1  FAIL=0  ERROR=0  SKIP=0  TOTAL=1",
        ),
        retained_duckdb_relative_path=Path("target/run/scenarios/order_totals_pass/local.duckdb"),
        retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
        expected_count=2,
        retained_rows_sql=('SELECT id, amount FROM "__sqb_local__source__raw_orders" ORDER BY id'),
        expected_rows=((1, 10), (2, 5)),
    ),
    ScenarioLocalRetainE2ETestCase(
        description="malformed local JSONL errors and retains local DuckDB",
        scenario_name="order_totals_pass",
        capture_command=("--no-color", "scenario", "capture", "order_totals_pass"),
        command=("--no-color", "scenario", "test", "order_totals_pass", "--local"),
        expected_exit_code=1,
        expected_stdout_fragments=(
            "order_totals_pass",
            "ERROR",
            "error[X604]:",
            "Retained local DuckDB:",
            "PASS=0  FAIL=0  ERROR=1  SKIP=0  TOTAL=1",
        ),
        retained_duckdb_relative_path=Path("target/run/scenarios/order_totals_pass/local.duckdb"),
        retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
        expected_count=0,
        corrupt_jsonl=True,
    ),
    ScenarioLocalRetainE2ETestCase(
        description="expected mismatch is local FAIL",
        scenario_name="order_totals_fail",
        capture_command=("--no-color", "scenario", "capture", "order_totals_fail"),
        command=("--no-color", "scenario", "test", "order_totals_fail", "--local"),
        expected_exit_code=1,
        expected_stdout_fragments=(
            "order_totals_fail",
            "FAIL",
            "error[X506]:",
            "expect    expected order_totals",
            "PASS=0  FAIL=1  ERROR=0  SKIP=0  TOTAL=1",
        ),
        retained_duckdb_relative_path=Path("target/run/scenarios/order_totals_fail/local.duckdb"),
        retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
        expected_count=1,
        retained_rows_sql=('SELECT id, amount FROM "__sqb_local__source__raw_orders" ORDER BY id'),
        expected_rows=((1, 10),),
    ),
    ScenarioLocalRetainE2ETestCase(
        description="local model execution error is ERROR",
        scenario_name="local_model_error",
        capture_command=("--no-color", "scenario", "capture", "local_model_error"),
        command=("--no-color", "scenario", "test", "local_model_error", "--local"),
        expected_exit_code=1,
        expected_stdout_fragments=(
            "local_model_error",
            "ERROR",
            "error[X608]:",
            "not_a_number_",
            "PASS=0  FAIL=0  ERROR=1  SKIP=0  TOTAL=1",
        ),
        retained_duckdb_relative_path=Path("target/run/scenarios/local_model_error/local.duckdb"),
        retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
        expected_count=1,
        additional_project_files=(
            (
                "sql/models/local_model_error_rows.sql",
                "MODEL (description 'Test model local_model_error_rows.', materialized table);\n\n"
                "SELECT CAST('not_a_number_' || amount AS INTEGER) AS bad_value\n"
                'FROM __source("raw_orders")\n',
            ),
            (
                "tests/scenarios/local_model_error.sql",
                "SCENARIO (description 'Test scenario local_model_error.');\n\n"
                "WITH\n"
                "__source__raw_orders AS (\n"
                "  SELECT 1 AS id, 10 AS amount\n"
                "),\n"
                "__expected__local_model_error_rows AS (\n"
                "  SELECT 10 AS bad_value\n"
                ")\n"
                "SELECT 1\n",
            ),
        ),
    ),
    ScenarioLocalRetainE2ETestCase(
        description="local SQL function scenario passes",
        scenario_name="local_sql_function_pass",
        capture_command=("--no-color", "scenario", "capture", "local_sql_function_pass"),
        command=("--no-color", "scenario", "test", "local_sql_function_pass", "--local"),
        expected_exit_code=0,
        expected_stdout_fragments=(
            "local_sql_function_pass",
            "PASS=1  FAIL=0  ERROR=0  SKIP=0  TOTAL=1",
        ),
        retained_duckdb_relative_path=Path(
            "target/run/scenarios/local_sql_function_pass/local.duckdb"
        ),
        retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
        expected_count=2,
        additional_project_files=(
            (
                "sql/functions/sql/is_large_order.sql",
                "FUNCTION (description 'Test function is_large_order.', arguments (amount INTEGER), returns BOOLEAN);\n\namount > 9\n",
            ),
            (
                "sql/models/local_sql_function_pass_rows.sql",
                "MODEL (description 'Test model local_sql_function_pass_rows.', materialized table);\n\n"
                'SELECT id, __udf("is_large_order")(amount) AS is_large_order\n'
                'FROM __source("raw_orders")\n',
            ),
            (
                "tests/scenarios/local_sql_function_pass.sql",
                "SCENARIO (description 'Test scenario local_sql_function_pass.');\n\n"
                "WITH\n"
                "__source__raw_orders AS (\n"
                "  SELECT 1 AS id, 10 AS amount\n"
                "  UNION ALL\n"
                "  SELECT 2 AS id, 5 AS amount\n"
                "),\n"
                "__expected__local_sql_function_pass_rows AS (\n"
                "  SELECT 1 AS id, TRUE AS is_large_order\n"
                "  UNION ALL\n"
                "  SELECT 2 AS id, FALSE AS is_large_order\n"
                ")\n"
                "SELECT 1\n",
            ),
        ),
    ),
    ScenarioLocalRetainE2ETestCase(
        description="local SQL function setup error is ERROR",
        scenario_name="local_sql_function_setup_error",
        capture_command=("--no-color", "scenario", "capture", "local_sql_function_setup_error"),
        command=("--no-color", "scenario", "test", "local_sql_function_setup_error", "--local"),
        expected_exit_code=1,
        expected_stdout_fragments=(
            "local_sql_function_setup_error",
            "ERROR",
            "error[X609]:",
            "local function 'bad_sql_function' failed",
            "missing_col",
            "PASS=0  FAIL=0  ERROR=1  SKIP=0  TOTAL=1",
        ),
        retained_duckdb_relative_path=Path(
            "target/run/scenarios/local_sql_function_setup_error/local.duckdb"
        ),
        retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
        expected_count=1,
        additional_project_files=(
            (
                "sql/functions/sql/bad_sql_function.sql",
                "FUNCTION (description 'Test function bad_sql_function.', arguments (amount INTEGER), returns BOOLEAN);\n\nmissing_col > 9\n",
            ),
            (
                "sql/models/local_sql_function_setup_error_rows.sql",
                "MODEL (description 'Test model local_sql_function_setup_error_rows.', materialized table);\n\n"
                'SELECT id, __udf("bad_sql_function")(amount) AS is_large_order\n'
                'FROM __source("raw_orders")\n',
            ),
            (
                "tests/scenarios/local_sql_function_setup_error.sql",
                "SCENARIO (description 'Test scenario local_sql_function_setup_error.');\n\n"
                "WITH\n"
                "__source__raw_orders AS (\n"
                "  SELECT 1 AS id, 10 AS amount\n"
                "),\n"
                "__expected__local_sql_function_setup_error_rows AS (\n"
                "  SELECT 1 AS id, TRUE AS is_large_order\n"
                ")\n"
                "SELECT 1\n",
            ),
        ),
    ),
    ScenarioLocalRetainE2ETestCase(
        description="local SQL function transpile error is ERROR",
        scenario_name="local_sql_function_transpile_error",
        capture_command=(
            "--no-color",
            "scenario",
            "capture",
            "local_sql_function_transpile_error",
        ),
        command=(
            "--no-color",
            "scenario",
            "test",
            "local_sql_function_transpile_error",
            "--local",
        ),
        expected_exit_code=1,
        expected_stdout_fragments=(
            "local_sql_function_transpile_error",
            "ERROR",
            "error[X607]:",
            "bad_transpile_function",
            "PASS=0  FAIL=0  ERROR=1  SKIP=0  TOTAL=1",
        ),
        retained_duckdb_relative_path=Path(
            "target/run/scenarios/local_sql_function_transpile_error/local.duckdb"
        ),
        retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
        expected_count=1,
        corrupt_capture_dialect=True,
        additional_project_files=(
            (
                "sql/functions/sql/bad_transpile_function.sql",
                "FUNCTION (description 'Test function bad_transpile_function.', arguments (amount INTEGER), returns BOOLEAN);\n\namount > 9\n",
            ),
            (
                "sql/models/local_sql_function_transpile_error_rows.sql",
                "MODEL (description 'Test model local_sql_function_transpile_error_rows.', materialized table);\n\n"
                'SELECT id, __udf("bad_transpile_function")(amount) AS is_large_order\n'
                'FROM __source("raw_orders")\n',
            ),
            (
                "tests/scenarios/local_sql_function_transpile_error.sql",
                "SCENARIO (description 'Test scenario local_sql_function_transpile_error.');\n\n"
                "WITH\n"
                "__source__raw_orders AS (\n"
                "  SELECT 1 AS id, 10 AS amount\n"
                "),\n"
                "__expected__local_sql_function_transpile_error_rows AS (\n"
                "  SELECT 1 AS id, TRUE AS is_large_order\n"
                ")\n"
                "SELECT 1\n",
            ),
        ),
    ),
    ScenarioLocalRetainE2ETestCase(
        description="local SQL function runtime error is ERROR",
        scenario_name="local_sql_function_runtime_error",
        capture_command=("--no-color", "scenario", "capture", "local_sql_function_runtime_error"),
        command=("--no-color", "scenario", "test", "local_sql_function_runtime_error", "--local"),
        expected_exit_code=1,
        expected_stdout_fragments=(
            "local_sql_function_runtime_error",
            "ERROR",
            "error[X608]:",
            "Could not convert string",
            "PASS=0  FAIL=0  ERROR=1  SKIP=0  TOTAL=1",
        ),
        retained_duckdb_relative_path=Path(
            "target/run/scenarios/local_sql_function_runtime_error/local.duckdb"
        ),
        retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
        expected_count=1,
        additional_project_files=(
            (
                "sql/functions/sql/sql_runtime_error.sql",
                "FUNCTION (description 'Test function sql_runtime_error.', arguments (amount INTEGER), returns INTEGER);\n\n"
                "CAST('bad' AS INTEGER)\n",
            ),
            (
                "sql/models/local_sql_function_runtime_error_rows.sql",
                "MODEL (description 'Test model local_sql_function_runtime_error_rows.', materialized table);\n\n"
                'SELECT id, __udf("sql_runtime_error")(amount) AS bad_value\n'
                'FROM __source("raw_orders")\n',
            ),
            (
                "tests/scenarios/local_sql_function_runtime_error.sql",
                "SCENARIO (description 'Test scenario local_sql_function_runtime_error.');\n\n"
                "WITH\n"
                "__source__raw_orders AS (\n"
                "  SELECT 1 AS id, 10 AS amount\n"
                "),\n"
                "__expected__local_sql_function_runtime_error_rows AS (\n"
                "  SELECT 1 AS id, 10 AS bad_value\n"
                ")\n"
                "SELECT 1\n",
            ),
        ),
    ),
    ScenarioLocalRetainE2ETestCase(
        description="local Python function scenario passes",
        scenario_name="local_python_function_pass",
        capture_command=("--no-color", "scenario", "capture", "local_python_function_pass"),
        command=("--no-color", "scenario", "test", "local_python_function_pass", "--local"),
        expected_exit_code=0,
        expected_stdout_fragments=(
            "local_python_function_pass",
            "PASS=1  FAIL=0  ERROR=0  SKIP=0  TOTAL=1",
        ),
        retained_duckdb_relative_path=Path(
            "target/run/scenarios/local_python_function_pass/local.duckdb"
        ),
        retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
        expected_count=2,
        additional_project_files=(
            (
                "sql/functions/python/is_large_order_py.py",
                "from sqlbuild.functions import udf\n\n\n"
                "@udf(\n"
                '    arguments={"amount": "INTEGER"},\n'
                '    returns="BOOLEAN",\n'
                '    runtime_version="3.11",\n'
                ")\n"
                "def main(amount: int | None) -> bool:\n"
                "    '''Test function main.'''\n    return amount is not None and amount > 9\n",
            ),
            (
                "sql/models/local_python_function_pass_rows.sql",
                "MODEL (description 'Test model local_python_function_pass_rows.', materialized table);\n\n"
                'SELECT id, __udf("is_large_order_py")(amount) AS is_large_order\n'
                'FROM __source("raw_orders")\n',
            ),
            (
                "tests/scenarios/local_python_function_pass.sql",
                "SCENARIO (description 'Test scenario local_python_function_pass.');\n\n"
                "WITH\n"
                "__source__raw_orders AS (\n"
                "  SELECT 1 AS id, 10 AS amount\n"
                "  UNION ALL\n"
                "  SELECT 2 AS id, 5 AS amount\n"
                "),\n"
                "__expected__local_python_function_pass_rows AS (\n"
                "  SELECT 1 AS id, TRUE AS is_large_order\n"
                "  UNION ALL\n"
                "  SELECT 2 AS id, FALSE AS is_large_order\n"
                ")\n"
                "SELECT 1\n",
            ),
        ),
    ),
    ScenarioLocalRetainE2ETestCase(
        description="local Python function setup error is ERROR",
        scenario_name="local_python_function_error",
        capture_command=("--no-color", "scenario", "capture", "local_python_function_error"),
        command=("--no-color", "scenario", "test", "local_python_function_error", "--local"),
        expected_exit_code=1,
        expected_stdout_fragments=(
            "local_python_function_error",
            "ERROR",
            "error[X609]:",
            "cannot set database or schema",
            "PASS=0  FAIL=0  ERROR=1  SKIP=0  TOTAL=1",
        ),
        retained_duckdb_relative_path=Path(
            "target/run/scenarios/local_python_function_error/local.duckdb"
        ),
        retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
        expected_count=1,
        additional_project_files=(
            (
                "sql/functions/python/is_large_order_py.py",
                "from sqlbuild.functions import udf\n\n\n"
                "@udf(\n"
                '    arguments={"amount": "INTEGER"},\n'
                '    returns="BOOLEAN",\n'
                '    runtime_version="3.11",\n'
                '    schema="analytics",\n'
                ")\n"
                "def main(amount: int | None) -> bool:\n"
                "    '''Test function main.'''\n    return amount is not None and amount > 9\n",
            ),
            (
                "sql/models/local_python_function_error_rows.sql",
                "MODEL (description 'Test model local_python_function_error_rows.', materialized table);\n\n"
                'SELECT id, __udf("is_large_order_py")(amount) AS is_large_order\n'
                'FROM __source("raw_orders")\n',
            ),
            (
                "tests/scenarios/local_python_function_error.sql",
                "SCENARIO (description 'Test scenario local_python_function_error.');\n\n"
                "WITH\n"
                "__source__raw_orders AS (\n"
                "  SELECT 1 AS id, 10 AS amount\n"
                "),\n"
                "__expected__local_python_function_error_rows AS (\n"
                "  SELECT 1 AS id, TRUE AS is_large_order\n"
                ")\n"
                "SELECT 1\n",
            ),
        ),
    ),
    ScenarioLocalRetainE2ETestCase(
        description="local Python function runtime error is ERROR",
        scenario_name="local_python_function_runtime_error",
        capture_command=(
            "--no-color",
            "scenario",
            "capture",
            "local_python_function_runtime_error",
        ),
        command=(
            "--no-color",
            "scenario",
            "test",
            "local_python_function_runtime_error",
            "--local",
        ),
        expected_exit_code=1,
        expected_stdout_fragments=(
            "local_python_function_runtime_error",
            "ERROR",
            "error[X608]:",
            "python udf exploded",
            "PASS=0  FAIL=0  ERROR=1  SKIP=0  TOTAL=1",
        ),
        retained_duckdb_relative_path=Path(
            "target/run/scenarios/local_python_function_runtime_error/local.duckdb"
        ),
        retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
        expected_count=1,
        additional_project_files=(
            (
                "sql/functions/python/python_runtime_error.py",
                "from sqlbuild.functions import udf\n\n\n"
                "@udf(\n"
                '    arguments={"amount": "INTEGER"},\n'
                '    returns="INTEGER",\n'
                '    runtime_version="3.11",\n'
                ")\n"
                "def main(amount: int | None) -> int:\n"
                '    """Test function main."""\n    raise ValueError("python udf exploded")\n',
            ),
            (
                "sql/models/local_python_function_runtime_error_rows.sql",
                "MODEL (description 'Test model local_python_function_runtime_error_rows.', materialized table);\n\n"
                'SELECT id, __udf("python_runtime_error")(amount) AS bad_value\n'
                'FROM __source("raw_orders")\n',
            ),
            (
                "tests/scenarios/local_python_function_runtime_error.sql",
                "SCENARIO (description 'Test scenario local_python_function_runtime_error.');\n\n"
                "WITH\n"
                "__source__raw_orders AS (\n"
                "  SELECT 1 AS id, 10 AS amount\n"
                "),\n"
                "__expected__local_python_function_runtime_error_rows AS (\n"
                "  SELECT 1 AS id, 10 AS bad_value\n"
                ")\n"
                "SELECT 1\n",
            ),
        ),
    ),
)


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioCliE2ETestCase(
            description="runs selected scenario by name and cleans up artifacts",
            command=("--no-color", "scenario", "test", "order_totals_pass"),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Execution  sqb scenario test  (target: order_totals_pass, concurrency: 1)",
                "Scenario (1 selected)",
                "order_totals_pass",
                "expect    expected order_totals",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
            expected_retained_prefix_count=0,
        ),
        ScenarioCliE2ETestCase(
            description="runs assertion scenario and reports passing assertion expectation",
            command=("--no-color", "scenario", "test", "orders_assert_pass"),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Scenario (1 selected)",
                "orders_assert_pass",
                "expect    assertion no_negative_orders",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
            expected_retained_prefix_count=0,
        ),
        ScenarioCliE2ETestCase(
            description="runs selected scenario by path and cleans up artifacts",
            command=(
                "--no-color",
                "scenario",
                "test",
                "tests/scenarios/nested/orders_assert_pass.sql",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Scenario (1 selected)",
                "orders_assert_pass",
                "expect    assertion no_negative_orders",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
            expected_retained_prefix_count=0,
        ),
        ScenarioCliE2ETestCase(
            description="runs multiple selected scenarios by name and path",
            command=(
                "--no-color",
                "scenario",
                "test",
                "order_totals_pass",
                "tests/scenarios/nested/orders_assert_pass.sql",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Scenario (2 selected)",
                "order_totals_pass",
                "orders_assert_pass",
                "expect    expected order_totals",
                "expect    assertion no_negative_orders",
                "PASS=2  FAIL=0  TOTAL=2",
            ),
            expected_retained_prefix_count=0,
        ),
        ScenarioCliE2ETestCase(
            description="runs scenarios under project relative folder selector",
            command=("--no-color", "scenario", "test", "tests/scenarios/nested"),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Scenario (1 selected)",
                "orders_assert_pass",
                "expect    assertion no_negative_orders",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
            expected_retained_prefix_count=0,
        ),
        ScenarioCliE2ETestCase(
            description="deduplicates selected scenarios from name and folder selectors",
            command=("--no-color", "scenario", "test", "orders_assert_pass", "nested"),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Scenario (1 selected)",
                "orders_assert_pass",
                "expect    assertion no_negative_orders",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
            expected_retained_prefix_count=0,
        ),
        ScenarioCliE2ETestCase(
            description="retains artifacts and prints relation map",
            command=("--no-color", "scenario", "test", "order_totals_pass", "--retain"),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Scenario (1 selected)",
                "order_totals_pass",
                "Retained relations:",
                "expect    expected order_totals",
                "source raw_orders -> __sqb_",
                "model  orders -> __sqb_",
                "model  order_totals -> __sqb_",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
            expected_retained_prefix_count=3,
        ),
        ScenarioCliE2ETestCase(
            description="failed scenario suggests retain after cleanup",
            command=("--no-color", "scenario", "test", "order_totals_fail"),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "Scenario (1 selected)",
                "order_totals_fail",
                "error[X506]: scenario 'order_totals_fail' expected comparison for model "
                "'order_totals' failed: actual=1 expected=1 mismatched=1",
                "= help: Compare the expected CTE with the retained scenario model relation.",
                "expect    expected order_totals",
                "expected order_totals:",
                "Rerun with --retain to inspect scenario-owned artifacts.",
                "PASS=0  FAIL=1  TOTAL=1",
            ),
            expected_retained_prefix_count=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_scenario_project_when_running_scenario_test_then_cli_behaves_as_expected(
    test_case: ScenarioCliE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_project",
        repo_files=build_scenario_project_files(),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    expected_stdout_fragment: str
    for expected_stdout_fragment in test_case.expected_stdout_fragments:
        assert expected_stdout_fragment in result.stdout
    assert test_case.expected_retained_prefix_count is not None
    retained_names: tuple[str, ...] = list_scenario_relation_names(
        db_path=project_dir / "scenario_demo.duckdb"
    )
    assert len(retained_names) == test_case.expected_retained_prefix_count


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioPythonHooksCliE2ETestCase(
            description="scenario CLI executes discovered Python model hook",
            command=("--no-color", "scenario", "test", "order_totals_pass", "--retain"),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Scenario (1 selected)",
                "order_totals_pass",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
            expected_hook_log_rows=(("orders", "post_hooks"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_scenario_project_with_python_hooks_when_running_cli_then_hooks_execute(
    test_case: ScenarioPythonHooksCliE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_python_hooks_project",
        repo_files=build_scenario_python_hooks_project_files(),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    expected_fragment: str
    for expected_fragment in test_case.expected_stdout_fragments:
        assert expected_fragment in result.stdout
    assert query_duckdb(
        db_path=project_dir / "scenario_demo.duckdb",
        sql="SELECT model_name, phase FROM main.scenario_hook_log",
    ) == list(test_case.expected_hook_log_rows)


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioLocalCliE2ETestCase(
            description="missing snapshot skips by default",
            command=("--no-color", "scenario", "test", "order_totals_pass", "--local"),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "order_totals_pass",
                "SKIP",
                "error[X601]:",
                "PASS=0  FAIL=0  ERROR=0  SKIP=1  TOTAL=1",
            ),
        ),
        ScenarioLocalCliE2ETestCase(
            description="missing snapshot errors with strict",
            command=(
                "--no-color",
                "scenario",
                "test",
                "order_totals_pass",
                "--local",
                "--strict",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "order_totals_pass",
                "ERROR",
                "error[X601]:",
                "PASS=0  FAIL=0  ERROR=1  SKIP=0  TOTAL=1",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_missing_snapshot_when_running_local_scenario_then_reports_expected_status(
    test_case: ScenarioLocalCliE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_project",
        repo_files=build_scenario_project_files(),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    expected_fragment: str
    for expected_fragment in test_case.expected_stdout_fragments:
        assert expected_fragment in result.stdout


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioLocalRetainE2ETestCase(
            description="captured snapshot keeps local DuckDB by default",
            scenario_name="order_totals_pass",
            capture_command=("--no-color", "scenario", "capture", "order_totals_pass"),
            command=("--no-color", "scenario", "test", "order_totals_pass", "--local"),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "order_totals_pass",
                "Retained local DuckDB:",
                "PASS=1  FAIL=0  ERROR=0  SKIP=0  TOTAL=1",
            ),
            retained_duckdb_relative_path=Path(
                "target/run/scenarios/order_totals_pass/local.duckdb"
            ),
            retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
            expected_count=2,
            retained_rows_sql=(
                'SELECT id, amount FROM "__sqb_local__source__raw_orders" ORDER BY id'
            ),
            expected_rows=((1, 10), (2, 5)),
        ),
        ScenarioLocalRetainE2ETestCase(
            description="malformed local JSONL errors and retains local DuckDB",
            scenario_name="order_totals_pass",
            capture_command=("--no-color", "scenario", "capture", "order_totals_pass"),
            command=("--no-color", "scenario", "test", "order_totals_pass", "--local"),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "order_totals_pass",
                "ERROR",
                "error[X604]:",
                "Retained local DuckDB:",
                "PASS=0  FAIL=0  ERROR=1  SKIP=0  TOTAL=1",
            ),
            retained_duckdb_relative_path=Path(
                "target/run/scenarios/order_totals_pass/local.duckdb"
            ),
            retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
            expected_count=0,
            corrupt_jsonl=True,
        ),
        ScenarioLocalRetainE2ETestCase(
            description="expected mismatch is local FAIL",
            scenario_name="order_totals_fail",
            capture_command=("--no-color", "scenario", "capture", "order_totals_fail"),
            command=("--no-color", "scenario", "test", "order_totals_fail", "--local"),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "order_totals_fail",
                "FAIL",
                "error[X506]:",
                "expect    expected order_totals",
                "PASS=0  FAIL=1  ERROR=0  SKIP=0  TOTAL=1",
            ),
            retained_duckdb_relative_path=Path(
                "target/run/scenarios/order_totals_fail/local.duckdb"
            ),
            retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
            expected_count=1,
            retained_rows_sql=(
                'SELECT id, amount FROM "__sqb_local__source__raw_orders" ORDER BY id'
            ),
            expected_rows=((1, 10),),
        ),
        ScenarioLocalRetainE2ETestCase(
            description="local model execution error is ERROR",
            scenario_name="local_model_error",
            capture_command=("--no-color", "scenario", "capture", "local_model_error"),
            command=("--no-color", "scenario", "test", "local_model_error", "--local"),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "local_model_error",
                "ERROR",
                "error[X608]:",
                "not_a_number_",
                "PASS=0  FAIL=0  ERROR=1  SKIP=0  TOTAL=1",
            ),
            retained_duckdb_relative_path=Path(
                "target/run/scenarios/local_model_error/local.duckdb"
            ),
            retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
            expected_count=1,
            additional_project_files=(
                (
                    "sql/models/local_model_error_rows.sql",
                    "MODEL (description 'Test model local_model_error_rows.', materialized table);\n\n"
                    "SELECT CAST('not_a_number_' || amount AS INTEGER) AS bad_value\n"
                    'FROM __source("raw_orders")\n',
                ),
                (
                    "tests/scenarios/local_model_error.sql",
                    "SCENARIO (description 'Test scenario local_model_error.');\n\n"
                    "WITH\n"
                    "__source__raw_orders AS (\n"
                    "  SELECT 1 AS id, 10 AS amount\n"
                    "),\n"
                    "__expected__local_model_error_rows AS (\n"
                    "  SELECT 10 AS bad_value\n"
                    ")\n"
                    "SELECT 1\n",
                ),
            ),
        ),
        ScenarioLocalRetainE2ETestCase(
            description="local SQL function scenario passes",
            scenario_name="local_sql_function_pass",
            capture_command=("--no-color", "scenario", "capture", "local_sql_function_pass"),
            command=("--no-color", "scenario", "test", "local_sql_function_pass", "--local"),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "local_sql_function_pass",
                "PASS=1  FAIL=0  ERROR=0  SKIP=0  TOTAL=1",
            ),
            retained_duckdb_relative_path=Path(
                "target/run/scenarios/local_sql_function_pass/local.duckdb"
            ),
            retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
            expected_count=2,
            additional_project_files=(
                (
                    "sql/functions/sql/is_large_order.sql",
                    "FUNCTION (description 'Test function is_large_order.', arguments (amount INTEGER), returns BOOLEAN);\n\namount > 9\n",
                ),
                (
                    "sql/models/local_sql_function_pass_rows.sql",
                    "MODEL (description 'Test model local_sql_function_pass_rows.', materialized table);\n\n"
                    'SELECT id, __udf("is_large_order")(amount) AS is_large_order\n'
                    'FROM __source("raw_orders")\n',
                ),
                (
                    "tests/scenarios/local_sql_function_pass.sql",
                    "SCENARIO (description 'Test scenario local_sql_function_pass.');\n\n"
                    "WITH\n"
                    "__source__raw_orders AS (\n"
                    "  SELECT 1 AS id, 10 AS amount\n"
                    "  UNION ALL\n"
                    "  SELECT 2 AS id, 5 AS amount\n"
                    "),\n"
                    "__expected__local_sql_function_pass_rows AS (\n"
                    "  SELECT 1 AS id, TRUE AS is_large_order\n"
                    "  UNION ALL\n"
                    "  SELECT 2 AS id, FALSE AS is_large_order\n"
                    ")\n"
                    "SELECT 1\n",
                ),
            ),
        ),
        ScenarioLocalRetainE2ETestCase(
            description="local SQL function setup error is ERROR",
            scenario_name="local_sql_function_setup_error",
            capture_command=("--no-color", "scenario", "capture", "local_sql_function_setup_error"),
            command=("--no-color", "scenario", "test", "local_sql_function_setup_error", "--local"),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "local_sql_function_setup_error",
                "ERROR",
                "error[X609]:",
                "local function 'bad_sql_function' failed",
                "missing_col",
                "PASS=0  FAIL=0  ERROR=1  SKIP=0  TOTAL=1",
            ),
            retained_duckdb_relative_path=Path(
                "target/run/scenarios/local_sql_function_setup_error/local.duckdb"
            ),
            retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
            expected_count=1,
            additional_project_files=(
                (
                    "sql/functions/sql/bad_sql_function.sql",
                    "FUNCTION (description 'Test function bad_sql_function.', arguments (amount INTEGER), returns BOOLEAN);\n\nmissing_col > 9\n",
                ),
                (
                    "sql/models/local_sql_function_setup_error_rows.sql",
                    "MODEL (description 'Test model local_sql_function_setup_error_rows.', materialized table);\n\n"
                    'SELECT id, __udf("bad_sql_function")(amount) AS is_large_order\n'
                    'FROM __source("raw_orders")\n',
                ),
                (
                    "tests/scenarios/local_sql_function_setup_error.sql",
                    "SCENARIO (description 'Test scenario local_sql_function_setup_error.');\n\n"
                    "WITH\n"
                    "__source__raw_orders AS (\n"
                    "  SELECT 1 AS id, 10 AS amount\n"
                    "),\n"
                    "__expected__local_sql_function_setup_error_rows AS (\n"
                    "  SELECT 1 AS id, TRUE AS is_large_order\n"
                    ")\n"
                    "SELECT 1\n",
                ),
            ),
        ),
        ScenarioLocalRetainE2ETestCase(
            description="local SQL function transpile error is ERROR",
            scenario_name="local_sql_function_transpile_error",
            capture_command=(
                "--no-color",
                "scenario",
                "capture",
                "local_sql_function_transpile_error",
            ),
            command=(
                "--no-color",
                "scenario",
                "test",
                "local_sql_function_transpile_error",
                "--local",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "local_sql_function_transpile_error",
                "ERROR",
                "error[X607]:",
                "bad_transpile_function",
                "PASS=0  FAIL=0  ERROR=1  SKIP=0  TOTAL=1",
            ),
            retained_duckdb_relative_path=Path(
                "target/run/scenarios/local_sql_function_transpile_error/local.duckdb"
            ),
            retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
            expected_count=1,
            corrupt_capture_dialect=True,
            additional_project_files=(
                (
                    "sql/functions/sql/bad_transpile_function.sql",
                    "FUNCTION (description 'Test function bad_transpile_function.', arguments (amount INTEGER), returns BOOLEAN);\n\namount > 9\n",
                ),
                (
                    "sql/models/local_sql_function_transpile_error_rows.sql",
                    "MODEL (description 'Test model local_sql_function_transpile_error_rows.', materialized table);\n\n"
                    'SELECT id, __udf("bad_transpile_function")(amount) AS is_large_order\n'
                    'FROM __source("raw_orders")\n',
                ),
                (
                    "tests/scenarios/local_sql_function_transpile_error.sql",
                    "SCENARIO (description 'Test scenario local_sql_function_transpile_error.');\n\n"
                    "WITH\n"
                    "__source__raw_orders AS (\n"
                    "  SELECT 1 AS id, 10 AS amount\n"
                    "),\n"
                    "__expected__local_sql_function_transpile_error_rows AS (\n"
                    "  SELECT 1 AS id, TRUE AS is_large_order\n"
                    ")\n"
                    "SELECT 1\n",
                ),
            ),
        ),
        ScenarioLocalRetainE2ETestCase(
            description="local SQL function runtime error is ERROR",
            scenario_name="local_sql_function_runtime_error",
            capture_command=(
                "--no-color",
                "scenario",
                "capture",
                "local_sql_function_runtime_error",
            ),
            command=(
                "--no-color",
                "scenario",
                "test",
                "local_sql_function_runtime_error",
                "--local",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "local_sql_function_runtime_error",
                "ERROR",
                "error[X608]:",
                "Could not convert string",
                "PASS=0  FAIL=0  ERROR=1  SKIP=0  TOTAL=1",
            ),
            retained_duckdb_relative_path=Path(
                "target/run/scenarios/local_sql_function_runtime_error/local.duckdb"
            ),
            retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
            expected_count=1,
            additional_project_files=(
                (
                    "sql/functions/sql/sql_runtime_error.sql",
                    "FUNCTION (description 'Test function sql_runtime_error.', arguments (amount INTEGER), returns INTEGER);\n\n"
                    "CAST('bad' AS INTEGER)\n",
                ),
                (
                    "sql/models/local_sql_function_runtime_error_rows.sql",
                    "MODEL (description 'Test model local_sql_function_runtime_error_rows.', materialized table);\n\n"
                    'SELECT id, __udf("sql_runtime_error")(amount) AS bad_value\n'
                    'FROM __source("raw_orders")\n',
                ),
                (
                    "tests/scenarios/local_sql_function_runtime_error.sql",
                    "SCENARIO (description 'Test scenario local_sql_function_runtime_error.');\n\n"
                    "WITH\n"
                    "__source__raw_orders AS (\n"
                    "  SELECT 1 AS id, 10 AS amount\n"
                    "),\n"
                    "__expected__local_sql_function_runtime_error_rows AS (\n"
                    "  SELECT 1 AS id, 10 AS bad_value\n"
                    ")\n"
                    "SELECT 1\n",
                ),
            ),
        ),
        ScenarioLocalRetainE2ETestCase(
            description="local Python function scenario passes",
            scenario_name="local_python_function_pass",
            capture_command=("--no-color", "scenario", "capture", "local_python_function_pass"),
            command=("--no-color", "scenario", "test", "local_python_function_pass", "--local"),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "local_python_function_pass",
                "PASS=1  FAIL=0  ERROR=0  SKIP=0  TOTAL=1",
            ),
            retained_duckdb_relative_path=Path(
                "target/run/scenarios/local_python_function_pass/local.duckdb"
            ),
            retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
            expected_count=2,
            additional_project_files=(
                (
                    "sql/functions/python/is_large_order_py.py",
                    "from sqlbuild.functions import udf\n\n\n"
                    "@udf(\n"
                    '    arguments={"amount": "INTEGER"},\n'
                    '    returns="BOOLEAN",\n'
                    '    runtime_version="3.11",\n'
                    ")\n"
                    "def main(amount: int | None) -> bool:\n"
                    "    '''Test function main.'''\n    return amount is not None and amount > 9\n",
                ),
                (
                    "sql/models/local_python_function_pass_rows.sql",
                    "MODEL (description 'Test model local_python_function_pass_rows.', materialized table);\n\n"
                    'SELECT id, __udf("is_large_order_py")(amount) AS is_large_order\n'
                    'FROM __source("raw_orders")\n',
                ),
                (
                    "tests/scenarios/local_python_function_pass.sql",
                    "SCENARIO (description 'Test scenario local_python_function_pass.');\n\n"
                    "WITH\n"
                    "__source__raw_orders AS (\n"
                    "  SELECT 1 AS id, 10 AS amount\n"
                    "  UNION ALL\n"
                    "  SELECT 2 AS id, 5 AS amount\n"
                    "),\n"
                    "__expected__local_python_function_pass_rows AS (\n"
                    "  SELECT 1 AS id, TRUE AS is_large_order\n"
                    "  UNION ALL\n"
                    "  SELECT 2 AS id, FALSE AS is_large_order\n"
                    ")\n"
                    "SELECT 1\n",
                ),
            ),
        ),
        ScenarioLocalRetainE2ETestCase(
            description="local Python function setup error is ERROR",
            scenario_name="local_python_function_error",
            capture_command=("--no-color", "scenario", "capture", "local_python_function_error"),
            command=("--no-color", "scenario", "test", "local_python_function_error", "--local"),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "local_python_function_error",
                "ERROR",
                "error[X609]:",
                "cannot set database or schema",
                "PASS=0  FAIL=0  ERROR=1  SKIP=0  TOTAL=1",
            ),
            retained_duckdb_relative_path=Path(
                "target/run/scenarios/local_python_function_error/local.duckdb"
            ),
            retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
            expected_count=1,
            additional_project_files=(
                (
                    "sql/functions/python/is_large_order_py.py",
                    "from sqlbuild.functions import udf\n\n\n"
                    "@udf(\n"
                    '    arguments={"amount": "INTEGER"},\n'
                    '    returns="BOOLEAN",\n'
                    '    runtime_version="3.11",\n'
                    '    schema="analytics",\n'
                    ")\n"
                    "def main(amount: int | None) -> bool:\n"
                    "    '''Test function main.'''\n    return amount is not None and amount > 9\n",
                ),
                (
                    "sql/models/local_python_function_error_rows.sql",
                    "MODEL (description 'Test model local_python_function_error_rows.', materialized table);\n\n"
                    'SELECT id, __udf("is_large_order_py")(amount) AS is_large_order\n'
                    'FROM __source("raw_orders")\n',
                ),
                (
                    "tests/scenarios/local_python_function_error.sql",
                    "SCENARIO (description 'Test scenario local_python_function_error.');\n\n"
                    "WITH\n"
                    "__source__raw_orders AS (\n"
                    "  SELECT 1 AS id, 10 AS amount\n"
                    "),\n"
                    "__expected__local_python_function_error_rows AS (\n"
                    "  SELECT 1 AS id, TRUE AS is_large_order\n"
                    ")\n"
                    "SELECT 1\n",
                ),
            ),
        ),
        ScenarioLocalRetainE2ETestCase(
            description="local Python function runtime error is ERROR",
            scenario_name="local_python_function_runtime_error",
            capture_command=(
                "--no-color",
                "scenario",
                "capture",
                "local_python_function_runtime_error",
            ),
            command=(
                "--no-color",
                "scenario",
                "test",
                "local_python_function_runtime_error",
                "--local",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "local_python_function_runtime_error",
                "ERROR",
                "error[X608]:",
                "python udf exploded",
                "PASS=0  FAIL=0  ERROR=1  SKIP=0  TOTAL=1",
            ),
            retained_duckdb_relative_path=Path(
                "target/run/scenarios/local_python_function_runtime_error/local.duckdb"
            ),
            retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
            expected_count=1,
            additional_project_files=(
                (
                    "sql/functions/python/python_runtime_error.py",
                    "from sqlbuild.functions import udf\n\n\n"
                    "@udf(\n"
                    '    arguments={"amount": "INTEGER"},\n'
                    '    returns="INTEGER",\n'
                    '    runtime_version="3.11",\n'
                    ")\n"
                    "def main(amount: int | None) -> int:\n"
                    '    """Test function main."""\n    raise ValueError("python udf exploded")\n',
                ),
                (
                    "sql/models/local_python_function_runtime_error_rows.sql",
                    "MODEL (description 'Test model local_python_function_runtime_error_rows.', materialized table);\n\n"
                    'SELECT id, __udf("python_runtime_error")(amount) AS bad_value\n'
                    'FROM __source("raw_orders")\n',
                ),
                (
                    "tests/scenarios/local_python_function_runtime_error.sql",
                    "SCENARIO (description 'Test scenario local_python_function_runtime_error.');\n\n"
                    "WITH\n"
                    "__source__raw_orders AS (\n"
                    "  SELECT 1 AS id, 10 AS amount\n"
                    "),\n"
                    "__expected__local_python_function_runtime_error_rows AS (\n"
                    "  SELECT 1 AS id, 10 AS bad_value\n"
                    ")\n"
                    "SELECT 1\n",
                ),
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_captured_snapshot_when_running_local_scenario_then_manages_local_duckdb(
    test_case: ScenarioLocalRetainE2ETestCase,
    tmp_path: Path,
) -> None:
    project_files: dict[str, str] = build_scenario_project_files()
    project_files.update(dict(test_case.additional_project_files))
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_project",
        repo_files=project_files,
    )
    capture_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.capture_command,
        project_dir=project_dir,
    )
    assert capture_result.returncode == 0, capture_result.stdout + capture_result.stderr
    maybe_corrupt_scenario_snapshot_jsonl(
        project_dir=project_dir,
        scenario_name=test_case.scenario_name,
        enabled=test_case.corrupt_jsonl,
    )
    maybe_corrupt_scenario_snapshot_dialect(
        project_dir=project_dir,
        scenario_name=test_case.scenario_name,
        enabled=test_case.corrupt_capture_dialect,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    local_duckdb_path: Path = project_dir / test_case.retained_duckdb_relative_path
    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    expected_fragment: str
    for expected_fragment in test_case.expected_stdout_fragments:
        assert expected_fragment in result.stdout
    assert_local_duckdb_state(
        db_path=local_duckdb_path,
        stdout=result.stdout,
        expected_exists=test_case.expected_duckdb_exists,
        query_when_exists=test_case.expected_duckdb_exists and not test_case.corrupt_jsonl,
        count_sql=test_case.retained_count_sql,
        expected_count=test_case.expected_count,
        rows_sql=test_case.retained_rows_sql,
        expected_rows=test_case.expected_rows,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioLocalCommittedSnapshotE2ETestCase(
            description="replays committed local snapshot without capture",
            scenario_name="order_totals_pass",
            command=("--no-color", "scenario", "test", "order_totals_pass", "--local"),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "order_totals_pass",
                "PASS=1  FAIL=0  ERROR=0  SKIP=0  TOTAL=1",
            ),
            unexpected_stdout_fragments=("Snapshot Capture",),
            retained_duckdb_relative_path=Path(
                "target/run/scenarios/order_totals_pass/local.duckdb"
            ),
            retained_count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
            expected_count=2,
            retained_rows_sql=(
                'SELECT id, amount FROM "__sqb_local__source__raw_orders" ORDER BY id'
            ),
            expected_rows=((1, 10), (2, 5)),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_committed_snapshot_when_running_local_scenario_then_replays_without_capture(
    test_case: ScenarioLocalCommittedSnapshotE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_project",
        repo_files=build_scenario_project_files(),
    )
    write_committed_order_totals_pass_snapshot(project_dir=project_dir)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    local_duckdb_path: Path = project_dir / test_case.retained_duckdb_relative_path
    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    expected_fragment: str
    for expected_fragment in test_case.expected_stdout_fragments:
        assert expected_fragment in result.stdout
    unexpected_fragment: str
    for unexpected_fragment in test_case.unexpected_stdout_fragments:
        assert unexpected_fragment not in result.stdout
    assert_local_duckdb_state(
        db_path=local_duckdb_path,
        stdout=result.stdout,
        expected_exists=True,
        query_when_exists=True,
        count_sql=test_case.retained_count_sql,
        expected_count=test_case.expected_count,
        rows_sql=test_case.retained_rows_sql,
        expected_rows=test_case.expected_rows,
    )


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioLocalSnapshotSyncE2ETestCase(
            description="sync captures missing snapshot before local replay",
            scenario_name="order_totals_pass",
            command=(
                "--no-color",
                "scenario",
                "test",
                "order_totals_pass",
                "--local",
                "--sync-snapshots",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Snapshot Sync (1 selected)",
                "CAPTURED  1 relation, 2 rows",
                "source   raw_orders",
                "2 rows, 41 B",
                "snapshot tests/_scenario_snapshots/order_totals_pass/scenario.json",
                "SYNC_PASS=1  SYNC_FAIL=0",
                "order_totals_pass",
                "PASS=1  FAIL=0  ERROR=0  SKIP=0  TOTAL=1",
            ),
        ),
        ScenarioLocalSnapshotSyncE2ETestCase(
            description="sync reuses fresh snapshot",
            scenario_name="order_totals_pass",
            command=(
                "--no-color",
                "scenario",
                "test",
                "order_totals_pass",
                "--local",
                "--sync-snapshots",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "Snapshots are fresh.",
                "order_totals_pass",
                "ERROR",
                "error[X604]:",
                "PASS=0  FAIL=0  ERROR=1  SKIP=0  TOTAL=1",
            ),
            initial_capture=True,
            corrupt_jsonl=True,
            query_when_exists=False,
        ),
        ScenarioLocalSnapshotSyncE2ETestCase(
            description="refresh recaptures fresh snapshot before local replay",
            scenario_name="order_totals_pass",
            command=(
                "--no-color",
                "scenario",
                "test",
                "order_totals_pass",
                "--local",
                "--refresh",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Snapshot Refresh (1 selected)",
                "REFRESHED  1 relation, 2 rows",
                "source   raw_orders",
                "2 rows, 41 B",
                "snapshot tests/_scenario_snapshots/order_totals_pass/scenario.json",
                "REFRESH_PASS=1  REFRESH_FAIL=0",
                "order_totals_pass",
                "PASS=1  FAIL=0  ERROR=0  SKIP=0  TOTAL=1",
            ),
            initial_capture=True,
            corrupt_jsonl=True,
        ),
        ScenarioLocalSnapshotSyncE2ETestCase(
            description="sync recaptures stale snapshot before local replay",
            scenario_name="order_totals_pass",
            command=(
                "--no-color",
                "scenario",
                "test",
                "order_totals_pass",
                "--local",
                "--sync-snapshots",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Snapshot Sync (1 selected)",
                "CAPTURED  1 relation, 3 rows",
                "source   raw_orders",
                "3 rows",
                "snapshot tests/_scenario_snapshots/order_totals_pass/scenario.json",
                "SYNC_PASS=1  SYNC_FAIL=0",
                "order_totals_pass",
                "PASS=1  FAIL=0  ERROR=0  SKIP=0  TOTAL=1",
            ),
            initial_capture=True,
            update_scenario_after_capture=True,
            expected_count=3,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_local_snapshot_sync_when_running_local_scenario_then_captures_expected_snapshots(
    test_case: ScenarioLocalSnapshotSyncE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_project",
        repo_files=build_scenario_project_files(),
    )
    maybe_capture_scenario_snapshot(
        project_dir=project_dir,
        scenario_name=test_case.scenario_name,
        enabled=test_case.initial_capture,
    )
    maybe_corrupt_scenario_snapshot_jsonl(
        project_dir=project_dir,
        scenario_name=test_case.scenario_name,
        enabled=test_case.corrupt_jsonl,
    )
    maybe_write_stale_order_totals_scenario(
        project_dir=project_dir,
        enabled=test_case.update_scenario_after_capture,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    local_duckdb_path: Path = (
        project_dir / "target" / "run" / "scenarios" / test_case.scenario_name / "local.duckdb"
    )
    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    expected_fragment: str
    for expected_fragment in test_case.expected_stdout_fragments:
        assert expected_fragment in result.stdout
    assert_local_duckdb_state(
        db_path=local_duckdb_path,
        stdout=result.stdout,
        expected_exists=test_case.expected_duckdb_exists,
        query_when_exists=test_case.query_when_exists,
        count_sql='SELECT COUNT(*) FROM "__sqb_local__source__raw_orders"',
        expected_count=test_case.expected_count,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioCliE2ETestCase(
            description="runs all discovered scenarios",
            command=("--no-color", "scenario", "test"),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "Scenario (3 selected)",
                "order_totals_pass",
                "orders_assert_pass",
                "order_totals_fail",
                "expect    expected order_totals",
                "expect    assertion no_negative_orders",
                "PASS=2  FAIL=1  TOTAL=3",
            ),
            expected_retained_prefix_count=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_multiple_scenarios_when_running_without_selector_then_runs_all_scenarios(
    test_case: ScenarioCliE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_project",
        repo_files=build_scenario_project_files(),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    expected_stdout_fragment: str
    for expected_stdout_fragment in test_case.expected_stdout_fragments:
        assert expected_stdout_fragment in result.stdout
    retained_names: tuple[str, ...] = list_scenario_relation_names(
        db_path=project_dir / "scenario_demo.duckdb"
    )
    assert len(retained_names) == test_case.expected_retained_prefix_count


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioCliE2ETestCase(
            description="multiple selected scenarios retain materialized artifacts",
            command=(
                "--no-color",
                "scenario",
                "test",
                "order_totals_pass",
                "tests/scenarios/nested/orders_assert_pass.sql",
                "--retain",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Scenario (2 selected)",
                "order_totals_pass",
                "orders_assert_pass",
                "Retained relations:",
                "source raw_orders -> __sqb_",
                "model  orders -> __sqb_",
                "model  order_totals -> __sqb_",
                "PASS=2  FAIL=0  TOTAL=2",
            ),
            expected_retained_prefix_count=5,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_multiple_selected_scenarios_when_running_with_retain_then_materializes_each_scenario(
    test_case: ScenarioCliE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_project",
        repo_files=build_scenario_project_files(),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    expected_stdout_fragment: str
    for expected_stdout_fragment in test_case.expected_stdout_fragments:
        assert expected_stdout_fragment in result.stdout
    retained_names: tuple[str, ...] = list_scenario_relation_names(
        db_path=project_dir / "scenario_demo.duckdb"
    )
    assert len(retained_names) == test_case.expected_retained_prefix_count
    retained_names_by_suffix: defaultdict[str, list[str]] = defaultdict(list)
    for name in retained_names:
        retained_names_by_suffix[name[name.index("__", 2) :]].append(name)
    retained_source_names: tuple[str, ...] = tuple(retained_names_by_suffix["__source__raw_orders"])
    retained_orders_model_names: tuple[str, ...] = tuple(
        retained_names_by_suffix["__model__orders"]
    )
    retained_order_totals_names: tuple[str, ...] = tuple(
        retained_names_by_suffix["__model__order_totals"]
    )

    assert len(retained_source_names) == 2
    assert len(retained_orders_model_names) == 2
    assert len(retained_order_totals_names) == 1
    assert sorted(
        scenario_relation_row_count(
            db_path=project_dir / "scenario_demo.duckdb",
            relation_name=relation_name,
        )
        for relation_name in retained_source_names
    ) == [1, 2]
    assert sorted(
        scenario_relation_row_count(
            db_path=project_dir / "scenario_demo.duckdb",
            relation_name=relation_name,
        )
        for relation_name in retained_orders_model_names
    ) == [1, 2]
    assert (
        scenario_relation_row_count(
            db_path=project_dir / "scenario_demo.duckdb",
            relation_name=retained_order_totals_names[0],
        )
        == 1
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioCliE2ETestCase(
            description="unknown selector fails clearly",
            command=("--no-color", "scenario", "test", "missing_scenario"),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[C453]: Unknown scenario selector 'missing_scenario'",
            ),
            disabled_setting="sql_analysis",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unknown_scenario_selector_when_running_scenario_test_then_fails_clearly(
    test_case: ScenarioCliE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_project",
        repo_files=build_scenario_project_files(),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    expected_stderr_fragment: str
    for expected_stderr_fragment in test_case.expected_stderr_fragments:
        assert expected_stderr_fragment in result.stderr


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioCliE2ETestCase(
            description="local replay rejects disabled sql_analysis",
            command=(
                "--no-color",
                "scenario",
                "test",
                "order_totals_pass",
                "--local",
            ),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[C455]: scenario test --local requires SQL analysis; "
                "sqlbuild_project.toml sets [settings] sql_analysis = false",
                "= help: to run local scenario replay, snapshot sync, or snapshot refresh, "
                "set this in sqlbuild_project.toml:\n"
                "            [settings]\n"
                "            sql_analysis = true",
            ),
            disabled_setting="sql_analysis",
        ),
        ScenarioCliE2ETestCase(
            description="local snapshot sync rejects disabled SQL analysis",
            command=(
                "--no-color",
                "scenario",
                "test",
                "order_totals_pass",
                "--local",
                "--sync-snapshots",
            ),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[C455]: scenario test --local requires SQL analysis; "
                "sqlbuild_project.toml sets [settings] sql_analysis = false",
                "= help: to run local scenario replay, snapshot sync, or snapshot refresh, "
                "set this in sqlbuild_project.toml:\n"
                "            [settings]\n"
                "            sql_analysis = true",
            ),
            disabled_setting="sql_validation",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_local_scenario_command_when_sql_validation_disabled_then_fails_clearly(
    test_case: ScenarioCliE2ETestCase,
    tmp_path: Path,
) -> None:
    repo_files: dict[str, str] = build_scenario_project_files()
    disabled_setting: str | None = test_case.disabled_setting
    assert disabled_setting is not None
    repo_files["sqlbuild_project.toml"] += f"\n[settings]\n{disabled_setting} = false\n"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_project",
        repo_files=repo_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    expected_stderr_fragment: str
    for expected_stderr_fragment in test_case.expected_stderr_fragments:
        assert expected_stderr_fragment in result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioCliE2ETestCase(
            description="waffle shop fixture scenarios pass on duckdb",
            command=("--no-color", "scenario", "test"),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Scenario (3 selected)",
                "Connecting to duckdb...",
                "Running scenarios...",
                "daily_revenue_minimal",
                "expect    expected daily_revenue",
                "expect    assertion no_negative_revenue",
                "daily_revenue_multi_order",
                "expect    assertion positive_average_order_value",
                "fact_order_retained_artifacts",
                "expect    expected scenario_order_prices",
                "expect    assertion positive_line_total",
                "PASS=3  FAIL=0  TOTAL=3",
            ),
            expected_retained_prefix_count=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_waffle_shop_fixture_when_running_scenario_test_then_scenarios_pass_on_duckdb(
    test_case: ScenarioCliE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    expected_stdout_fragment: str
    for expected_stdout_fragment in test_case.expected_stdout_fragments:
        assert expected_stdout_fragment in result.stdout
    retained_names: tuple[str, ...] = list_scenario_relation_names(
        db_path=project_dir / "waffle_shop.duckdb"
    )
    assert len(retained_names) == test_case.expected_retained_prefix_count


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioCliE2ETestCase(
            description="waffle shop fixture retain keeps scenario artifacts on duckdb",
            command=(
                "--no-color",
                "scenario",
                "test",
                "fact_order_retained_artifacts",
                "--retain",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Scenario (1 selected)",
                "fact_order_retained_artifacts",
                "expect    expected scenario_order_prices",
                "expect    assertion positive_line_total",
                "Retained relations:",
                "source raw_orders -> __sqb_",
                "ref    stg_payments -> __sqb_",
                "seed   waffle_types -> __sqb_",
                "model  stg_orders -> __sqb_",
                "model  scenario_order_prices -> __sqb_",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
            expected_retained_prefix_count=5,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_waffle_shop_fixture_when_running_with_retain_then_keeps_scenario_artifacts(
    test_case: ScenarioCliE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    expected_stdout_fragment: str
    for expected_stdout_fragment in test_case.expected_stdout_fragments:
        assert expected_stdout_fragment in result.stdout
    retained_names: tuple[str, ...] = list_scenario_relation_names(
        db_path=project_dir / "waffle_shop.duckdb"
    )
    assert len(retained_names) == test_case.expected_retained_prefix_count
    expected_suffix: str
    for expected_suffix in (
        "__source__raw_orders",
        "__ref__stg_payments",
        "__seed__waffle_types",
        "__model__stg_orders",
        "__model__scenario_order_prices",
    ):
        retained_relation_name: str = scenario_relation_name_by_suffix(
            db_path=project_dir / "waffle_shop.duckdb",
            suffix=expected_suffix,
        )
        assert (
            scenario_relation_row_count(
                db_path=project_dir / "waffle_shop.duckdb",
                relation_name=retained_relation_name,
            )
            == 1
        )


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioRuntimeArtifactTestCase(
            description="writes fixture runtime SQL under target run scenarios",
            command=("--no-color", "scenario", "test", "order_totals_pass"),
            expected_exit_code=0,
            artifact_relative_path=Path(
                "target/run/scenarios/order_totals_pass/fixtures/source__raw_orders.sql"
            ),
            expected_artifact_fragments=(
                "CREATE",
                "__source__raw_orders",
                "SELECT 1 AS id, 10 AS amount",
            ),
        ),
        ScenarioRuntimeArtifactTestCase(
            description="writes model runtime SQL under target run scenarios",
            command=("--no-color", "scenario", "test", "order_totals_pass"),
            expected_exit_code=0,
            artifact_relative_path=Path(
                "target/run/scenarios/order_totals_pass/sql/models/order_totals.sql"
            ),
            expected_artifact_fragments=(
                "CREATE",
                "__model__order_totals",
                "FROM main.__sqb_",
            ),
        ),
        ScenarioRuntimeArtifactTestCase(
            description="writes expected comparison SQL under target run scenarios",
            command=("--no-color", "scenario", "test", "order_totals_pass"),
            expected_exit_code=0,
            artifact_relative_path=Path(
                "target/run/scenarios/order_totals_pass/expectations/expected__order_totals.sql"
            ),
            expected_artifact_fragments=(
                "WITH __actual AS",
                "__expected AS",
                "unexpected_count",
            ),
        ),
        ScenarioRuntimeArtifactTestCase(
            description="writes cleanup SQL under target run scenarios",
            command=("--no-color", "scenario", "test", "order_totals_pass"),
            expected_exit_code=0,
            artifact_relative_path=Path("target/run/scenarios/order_totals_pass/cleanup/final.sql"),
            expected_artifact_fragments=("DROP", "__model__order_totals"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_scenario_project_when_running_scenario_test_then_writes_runtime_artifacts(
    test_case: ScenarioRuntimeArtifactTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_project",
        repo_files=build_scenario_project_files(),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert_runtime_artifact_contains(
        project_dir=project_dir,
        relative_path=test_case.artifact_relative_path,
        expected_fragments=test_case.expected_artifact_fragments,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioLocalRuntimeArtifactTestCase(
            description="writes local fixture artifact",
            scenario_name="order_totals_pass",
            capture_command=("--no-color", "scenario", "capture", "order_totals_pass"),
            command=("--no-color", "scenario", "test", "order_totals_pass", "--local"),
            expected_exit_code=0,
            artifact_relative_path=Path(
                "target/run/scenarios/order_totals_pass/local/fixtures/source__raw_orders.sql"
            ),
            expected_artifact_fragments=(
                "loaded from",
                "sources/raw_orders.jsonl",
                'CREATE TABLE "__sqb_local__source__raw_orders"',
            ),
        ),
        ScenarioLocalRuntimeArtifactTestCase(
            description="writes local model artifact",
            scenario_name="order_totals_pass",
            capture_command=("--no-color", "scenario", "capture", "order_totals_pass"),
            command=("--no-color", "scenario", "test", "order_totals_pass", "--local"),
            expected_exit_code=0,
            artifact_relative_path=Path(
                "target/run/scenarios/order_totals_pass/local/sql/models/order_totals.sql"
            ),
            expected_artifact_fragments=(
                "SUM(amount) AS total_amount",
                "__sqb_local__model__orders",
            ),
        ),
        ScenarioLocalRuntimeArtifactTestCase(
            description="writes local expected comparison artifact",
            scenario_name="order_totals_pass",
            capture_command=("--no-color", "scenario", "capture", "order_totals_pass"),
            command=("--no-color", "scenario", "test", "order_totals_pass", "--local"),
            expected_exit_code=0,
            artifact_relative_path=Path(
                "target/run/scenarios/order_totals_pass/local/expectations/expected__order_totals.sql"
            ),
            expected_artifact_fragments=(
                "WITH __actual AS",
                "__sqb_local__model__order_totals",
                "unexpected_count",
            ),
        ),
        ScenarioLocalRuntimeArtifactTestCase(
            description="writes local SQL function artifact",
            scenario_name="local_sql_function_pass",
            capture_command=("--no-color", "scenario", "capture", "local_sql_function_pass"),
            command=("--no-color", "scenario", "test", "local_sql_function_pass", "--local"),
            expected_exit_code=0,
            artifact_relative_path=Path(
                "target/run/scenarios/local_sql_function_pass/local/sql/functions/sql/is_large_order.sql"
            ),
            expected_artifact_fragments=(
                "CREATE OR REPLACE MACRO",
                "is_large_order",
                "amount > 9",
            ),
            additional_project_files=SCENARIO_LOCAL_DUCKDB_CASE_FIXTURES[
                4
            ].additional_project_files,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_local_scenario_when_running_then_writes_local_runtime_artifacts(
    test_case: ScenarioLocalRuntimeArtifactTestCase,
    tmp_path: Path,
) -> None:
    project_files: dict[str, str] = build_scenario_project_files()
    project_files.update(dict(test_case.additional_project_files))
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_project",
        repo_files=project_files,
    )
    capture_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.capture_command,
        project_dir=project_dir,
    )
    assert capture_result.returncode == 0, capture_result.stdout + capture_result.stderr

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert_runtime_artifact_contains(
        project_dir=project_dir,
        relative_path=test_case.artifact_relative_path,
        expected_fragments=test_case.expected_artifact_fragments,
    )
