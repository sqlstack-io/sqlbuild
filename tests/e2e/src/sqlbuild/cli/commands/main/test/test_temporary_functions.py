"""Unit tests call the project's current function definitions without a build."""

from __future__ import annotations

import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import (
    TemporaryCalleeE2ETestCase,
    TemporaryFunctionConnectionE2ETestCase,
    TemporaryFunctionE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)

_FUNCTION_PATH: str = "sql/functions/sql/is_completed_order.sql"
_FUNCTION_HEADER: str = (
    "FUNCTION (description 'Whether an order status counts as completed.', "
    "arguments (order_status VARCHAR), returns BOOLEAN);\n"
)
_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders_temporary_functions"\nadapter = "duckdb"\n'
        '[connection]\ndatabase = "orders.duckdb"\n'
    ),
    "sql/models/order_statuses.sql": (
        "MODEL (description 'Test model order_statuses.', materialized table);\n"
        "SELECT 'completed' AS order_status\n"
    ),
    "sql/models/orders.sql": (
        "MODEL (description 'Test model orders.', materialized table);\n"
        'SELECT order_status, __udf("is_completed_order")(order_status) AS is_completed '
        'FROM __ref("order_statuses")\n'
    ),
    "tests/unit/orders_case.sql": (
        'TEST (name "orders_case");\nWITH\n'
        "__ref__order_statuses AS (SELECT 'completed' AS order_status),\n"
        "__expected__orders AS (SELECT 'completed' AS order_status, TRUE AS is_completed)\n"
        "SELECT 1\n"
    ),
}

_CALLEE_PATH: str = "sql/functions/sql/status_matches.sql"
_CALLEE_HEADER: str = (
    "FUNCTION (description 'Whether an order status is the completed one.', "
    "arguments (order_status VARCHAR), returns BOOLEAN);\n"
)
_CALLER_FILES: dict[str, str] = {
    **_PROJECT_FILES,
    _FUNCTION_PATH: _FUNCTION_HEADER + '__udf("status_matches")(order_status)',
}
_QUOTED_CALL_FILES: dict[str, str] = {
    **_PROJECT_FILES,
    _FUNCTION_PATH: _FUNCTION_HEADER + "order_status = 'completed'",
    "sql/models/orders.sql": (
        "MODEL (description 'Test model orders.', materialized table);\n"
        'SELECT order_status, __udf("is_completed_order")(order_status) AS is_completed, '
        "length('main.is_completed_order(order_status)') AS formula_length "
        "-- main.is_completed_order(order_status)\n"
        'FROM __ref("order_statuses")\n'
    ),
    "tests/unit/orders_case.sql": (
        'TEST (name "orders_case");\nWITH\n'
        "__ref__order_statuses AS (SELECT 'completed' AS order_status),\n"
        "__expected__orders AS (SELECT 'completed' AS order_status, TRUE AS is_completed, "
        "37 AS formula_length)\n"
        "SELECT 1\n"
    ),
}
_SECOND_CONNECTION_FAILS: dict[str, str] = {
    **_PROJECT_FILES,
    "sqlbuild_project.toml": _PROJECT_FILES["sqlbuild_project.toml"].replace(
        'adapter = "duckdb"', 'adapter = "duckdb_one_session"'
    ),
    _FUNCTION_PATH: _FUNCTION_HEADER + "order_status = 'completed'",
    "tests/unit/orders_again.sql": _PROJECT_FILES["tests/unit/orders_case.sql"].replace(
        "orders_case", "orders_again"
    ),
    "python/adapters/duckdb_one_session.py": (
        "from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter\n\n\n"
        "class DuckDbOneSessionAdapter(DuckDbAdapter):\n"
        '    adapter_name = "duckdb_one_session"\n'
        "    macro_sessions = []\n\n"
        "    def execute(self, *, connection, sql):\n"
        '        if sql.startswith("CREATE OR REPLACE TEMP MACRO"):\n'
        "            if not self.macro_sessions:\n"
        "                self.macro_sessions.append(connection)\n"
        "            if connection is not self.macro_sessions[0]:\n"
        '                raise PermissionError("temporary macros are disabled on this session")\n'
        "        return super().execute(connection=connection, sql=sql)\n"
    ),
}

_NO_COPIES: dict[str, str] = {
    **_PROJECT_FILES,
    "sqlbuild_project.toml": _PROJECT_FILES["sqlbuild_project.toml"].replace(
        'adapter = "duckdb"', 'adapter = "duckdb_no_copies"'
    ),
    "python/adapters/duckdb_no_copies.py": (
        "from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter\n\n\n"
        "class DuckDbNoCopiesAdapter(DuckDbAdapter):\n"
        '    adapter_name = "duckdb_no_copies"\n\n'
        "    def execute(self, *, connection, sql):\n"
        '        if sql.startswith("CREATE OR REPLACE TEMP MACRO"):\n'
        '            raise PermissionError("temporary macros are disabled on this session")\n'
        "        return super().execute(connection=connection, sql=sql)\n"
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        TemporaryFunctionE2ETestCase(
            description="edited function body decides the test without a rebuild",
            built_body="order_status = 'completed'",
            edited_body="order_status = 'shipped'",
            expected_exit_code=1,
            expected_stdout_fragment="FAIL=1",
            expected_built_result=True,
        ),
        TemporaryFunctionE2ETestCase(
            description="fixed function body passes before the built copy is replaced",
            built_body="order_status = 'shipped'",
            edited_body="order_status = 'completed'",
            expected_exit_code=0,
            expected_stdout_fragment="PASS=1",
            expected_built_result=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_edited_function_when_testing_then_current_definition_runs_and_build_is_untouched(
    test_case: TemporaryFunctionE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders_temporary_functions",
        repo_files={**_PROJECT_FILES, _FUNCTION_PATH: _FUNCTION_HEADER + test_case.built_body},
    )
    built: subprocess.CompletedProcess[str] = run_sqb(
        command=("build", "--no-tests", "--no-audits"), project_dir=project_dir
    )
    _ = (project_dir / _FUNCTION_PATH).write_text(
        _FUNCTION_HEADER + test_case.edited_body, encoding="utf-8"
    )

    tested: subprocess.CompletedProcess[str] = run_sqb(command=("test",), project_dir=project_dir)

    assert built.returncode == 0, built.stdout + built.stderr
    built_rows: list[tuple[Any, ...]] = query_duckdb(
        db_path=project_dir / "orders.duckdb",
        sql="SELECT main.is_completed_order('completed')",
    )
    assert tested.returncode == test_case.expected_exit_code, (
        test_case.description,
        tested.stdout + tested.stderr,
    )
    assert test_case.expected_stdout_fragment in tested.stdout, test_case.description
    assert built_rows == [(test_case.expected_built_result,)], test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        TemporaryFunctionE2ETestCase(
            description="never-built function runs from its temporary copy",
            built_body="order_status = 'completed'",
            edited_body="order_status = 'completed'",
            expected_exit_code=0,
            expected_stdout_fragment="PASS=1",
            expected_built_result=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unbuilt_function_when_testing_then_test_passes_and_no_function_is_created(
    test_case: TemporaryFunctionE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders_temporary_functions",
        repo_files={**_PROJECT_FILES, _FUNCTION_PATH: _FUNCTION_HEADER + test_case.edited_body},
    )
    execute_duckdb(db_path=project_dir / "orders.duckdb", sql="SELECT 1")

    tested: subprocess.CompletedProcess[str] = run_sqb(command=("test",), project_dir=project_dir)

    function_rows: list[tuple[Any, ...]] = query_duckdb(
        db_path=project_dir / "orders.duckdb",
        sql=(
            "SELECT count(*) > 0 FROM duckdb_functions() WHERE function_name = 'is_completed_order'"
        ),
    )
    assert tested.returncode == test_case.expected_exit_code, (
        test_case.description,
        tested.stdout + tested.stderr,
    )
    assert test_case.expected_stdout_fragment in tested.stdout, test_case.description
    assert function_rows == [(test_case.expected_built_result,)], test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        TemporaryCalleeE2ETestCase(
            description="edited callee decides the caller's test without a rebuild",
            build_first=True,
            callee_body="order_status = 'shipped'",
            expected_exit_code=1,
            expected_stdout_fragment="FAIL=1",
        ),
        TemporaryCalleeE2ETestCase(
            description="never-built caller and callee run from temporary copies",
            build_first=False,
            callee_body="order_status = 'completed'",
            expected_exit_code=0,
            expected_stdout_fragment="PASS=1",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_function_calling_another_when_testing_then_callee_current_definition_runs(
    test_case: TemporaryCalleeE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders_temporary_callee",
        repo_files={**_CALLER_FILES, _CALLEE_PATH: _CALLEE_HEADER + "order_status = 'completed'"},
    )
    commands: dict[bool, tuple[tuple[str, ...], ...]] = {
        True: (("build", "--no-tests", "--no-audits"),),
        False: (),
    }
    built: tuple[subprocess.CompletedProcess[str], ...] = tuple(
        run_sqb(command=command, project_dir=project_dir)
        for command in commands[test_case.build_first]
    )
    _ = (project_dir / _CALLEE_PATH).write_text(
        _CALLEE_HEADER + test_case.callee_body, encoding="utf-8"
    )

    tested: subprocess.CompletedProcess[str] = run_sqb(command=("test",), project_dir=project_dir)

    assert all(result.returncode == 0 for result in built), test_case.description
    assert tested.returncode == test_case.expected_exit_code, (
        test_case.description,
        tested.stdout + tested.stderr,
    )
    assert test_case.expected_stdout_fragment in tested.stdout, test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        TemporaryFunctionConnectionE2ETestCase(
            description="refused copy of a built function is a coded error, not the built function",
            expected_exit_code=1,
            expected_stderr_fragment=(
                "error[T006]: could not create the temporary copy of function "
                "is_completed_order on the first test connection: "
                "temporary macros are disabled on this session"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_copy_refused_when_testing_built_function_then_reports_coded_error(
    test_case: TemporaryFunctionConnectionE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders_temporary_functions",
        repo_files={**_NO_COPIES, _FUNCTION_PATH: _FUNCTION_HEADER + "order_status = 'completed'"},
    )
    built: subprocess.CompletedProcess[str] = run_sqb(
        command=("build", "--no-tests", "--no-audits"), project_dir=project_dir
    )
    _ = (project_dir / _FUNCTION_PATH).write_text(
        _FUNCTION_HEADER + "order_status = 'shipped'", encoding="utf-8"
    )

    tested: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test", "--run-on", "warehouse"), project_dir=project_dir
    )

    output: str = tested.stdout + tested.stderr
    assert built.returncode == 0, (test_case.description, built.stdout + built.stderr)
    assert tested.returncode == test_case.expected_exit_code, (test_case.description, output)
    assert test_case.expected_stderr_fragment in tested.stderr, (test_case.description, output)
    assert "PASS=1" not in tested.stdout, (test_case.description, output)


@pytest.mark.parametrize(
    "test_case",
    [
        TemporaryFunctionConnectionE2ETestCase(
            description="second test connection refusing the copies is a coded error",
            expected_exit_code=1,
            expected_stderr_fragment=(
                "error[T006]: could not create the temporary copy of function "
                "is_completed_order on test connection 2 of 2: "
                "temporary macros are disabled on this session"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_connection_refusing_copies_when_testing_concurrently_then_reports_coded_error(
    test_case: TemporaryFunctionConnectionE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders_temporary_functions",
        repo_files=_SECOND_CONNECTION_FAILS,
    )

    tested: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test", "--run-on", "warehouse", "--concurrency", "2"),
        project_dir=project_dir,
    )

    output: str = tested.stdout + tested.stderr
    assert tested.returncode == test_case.expected_exit_code, (test_case.description, output)
    assert test_case.expected_stderr_fragment in tested.stderr, (test_case.description, output)
    assert "Traceback" not in output, (test_case.description, output)


@pytest.mark.parametrize(
    "test_case",
    [
        TemporaryFunctionE2ETestCase(
            description="function calls quoted in strings and comments keep their text",
            built_body="order_status = 'completed'",
            edited_body="order_status = 'completed'",
            expected_exit_code=0,
            expected_stdout_fragment="PASS=1",
            expected_built_result=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_function_call_in_string_when_testing_then_only_code_calls_are_redirected(
    test_case: TemporaryFunctionE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders_temporary_functions",
        repo_files=_QUOTED_CALL_FILES,
    )

    tested: subprocess.CompletedProcess[str] = run_sqb(command=("test",), project_dir=project_dir)

    assert tested.returncode == test_case.expected_exit_code, (
        test_case.description,
        tested.stdout + tested.stderr,
    )
    assert test_case.expected_stdout_fragment in tested.stdout, test_case.description
