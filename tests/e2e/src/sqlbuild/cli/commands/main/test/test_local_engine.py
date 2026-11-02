"""E2E coverage of `sqb test --local`: Snowflake tests converted by SQLQuack and run on DuckDB."""

from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import (
    LocalEngineE2ETestCase,
    LocalFunctionE2ETestCase,
    RealLocalEngineE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.test.helpers import (
    build_local_engine_project_files,
    build_local_function_project_files,
    build_real_local_engine_project_files,
    build_real_local_function_project_files,
    read_fake_sqlquack_conversions,
    read_fake_sqlquack_log,
    write_fake_sqlquack,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_SQLQUACK_ENV: str = "SQLBUILD_SQLQUACK"
_POLICY_FILE: str = "sqlquack.toml"
_ACCOUNT: str = (
    "[connection]\n"
    'account = "example-no-login.invalid"\nuser = "NO_LOGIN"\npassword = "not-a-credential"\n'
)
_LOCAL_STATEMENT_HEADER: str = "-- the DuckDB statement SQLQuack converted this test to"
_REAL_SQLQUACK: Path = Path(
    os.environ.get(_SQLQUACK_ENV) or shutil.which("sqlquack") or "sqlquack-not-installed"
)


@pytest.mark.parametrize(
    "test_case",
    [
        LocalEngineE2ETestCase(
            description="passes, failures, refusals and assumptions are reported apart",
            arguments=("test", "--local"),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "PASS=2  FAIL=1  REFUSED=2  TOTAL=5",
                "[T009] test 'order_labels_are_refused' was refused by SQLQuack:",
                "fn:UNCONVERTIBLE_FN (missing rule): no rule converts fn:UNCONVERTIBLE_FN",
                "was refused at run time by SQLQuack's `json_fidelity` guard: a guarded value",
                "[T003] test 'customer_orders_wrong_count' failed for models: customer_orders",
                "differing column order_count: actual=2, expected=3",
                "Assumptions relied on (rerun with --strict to refuse them):",
                "session_timezone_utc  1 test",
            ),
            unexpected_stdout_fragments=("3:9",),
            expected_conversion_count=7,
            expected_converter_arguments=(
                "--json",
                "--session",
                "QUOTED_IDENTIFIERS_IGNORE_CASE=true",
                "--policy",
            ),
            expected_run_files=(
                "target/run/tests/**/*customer_orders_count*.sql",
                "target/run/tests/**/*order_zones*.sql",
            ),
        ),
        LocalEngineE2ETestCase(
            description="strict mode refuses tests relying on an assumption",
            arguments=("test", "--local", "--strict", "--select", "order_zones"),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "PASS=0  FAIL=0  REFUSED=1  TOTAL=1",
                "node:cast relies on assumption session_timezone_utc",
            ),
            unexpected_stdout_fragments=("Assumptions relied on",),
            expected_conversion_count=1,
            expected_converter_arguments=("--strict", "--policy"),
        ),
        LocalEngineE2ETestCase(
            description="an empty converted result is an error, not a pass",
            arguments=("test", "--local", "--select", "customer_orders_wrong_count"),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "PASS=0  FAIL=1  TOTAL=1",
                "returned an unusable local result: the comparison returned steps []",
            ),
            converter_mode="empty",
            expected_conversion_count=1,
        ),
        LocalEngineE2ETestCase(
            description="a comparison row short of columns is an error",
            arguments=("test", "--local", "--select", "customer_orders_wrong_count"),
            expected_exit_code=1,
            expected_stdout_fragments=("PASS=0  FAIL=1  TOTAL=1", "is not 6 counted columns"),
            converter_mode="short",
            expected_conversion_count=1,
        ),
        LocalEngineE2ETestCase(
            description="a fractional row count is an error",
            arguments=("test", "--local", "--select", "customer_orders_wrong_count"),
            expected_exit_code=1,
            expected_stdout_fragments=("PASS=0  FAIL=1  TOTAL=1", "is not 6 counted columns"),
            converter_mode="fractional",
            expected_conversion_count=1,
        ),
        LocalEngineE2ETestCase(
            description="a comparison row for a step the plan does not have is an error",
            arguments=("test", "--local", "--select", "customer_orders_wrong_count"),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "PASS=0  FAIL=1  TOTAL=1",
                "the comparison returned steps [(7, 'customer_orders')]",
            ),
            converter_mode="wrong_step",
            expected_conversion_count=1,
        ),
        LocalEngineE2ETestCase(
            description="a converted report from a failing converter is an error",
            arguments=("test", "--local", "--select", "customer_orders_count_completed"),
            expected_exit_code=1,
            expected_stdout_fragments=("PASS=0  FAIL=1  TOTAL=1", "SQLQuack exited 3"),
            converter_mode="nonzero_exit",
            expected_conversion_count=1,
        ),
        LocalEngineE2ETestCase(
            description="converted as a string is not converted",
            arguments=("test", "--local", "--select", "customer_orders_count_completed"),
            expected_exit_code=1,
            expected_stdout_fragments=("PASS=0  FAIL=1  TOTAL=1", "(converted='false'"),
            converter_mode="string_false",
            expected_conversion_count=1,
        ),
        LocalEngineE2ETestCase(
            description="a report with refusals is refused even when marked converted",
            arguments=("test", "--local", "--select", "customer_orders_count_completed"),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "PASS=0  FAIL=0  REFUSED=1  TOTAL=1",
                "fn:ANY (missing rule): stand-in",
            ),
            converter_mode="refusals_with_converted",
            expected_conversion_count=1,
        ),
        LocalEngineE2ETestCase(
            description="a report that is not a conversion report is an error",
            arguments=("test", "--local", "--select", "customer_orders_count_completed"),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "PASS=0  FAIL=1  TOTAL=1",
                "without a usable conversion report",
            ),
            converter_mode="invalid_json",
            expected_conversion_count=1,
        ),
        LocalEngineE2ETestCase(
            description="strict mode refuses a conversion that relies on an assumption anyway",
            arguments=(
                "test",
                "--local",
                "--strict",
                "--select",
                "customer_orders_count_completed",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "PASS=0  FAIL=0  REFUSED=1  TOTAL=1",
                "was refused under --strict: its conversion relies on session_timezone_utc",
            ),
            unexpected_stdout_fragments=("Assumptions relied on",),
            converter_mode="assumes_anyway",
            expected_conversion_count=1,
        ),
        LocalEngineE2ETestCase(
            description="the connection's time zone is passed as a session parameter",
            arguments=("test", "--local", "--select", "customer_orders_count_completed"),
            expected_exit_code=0,
            expected_stdout_fragments=("PASS=1  FAIL=0  TOTAL=1",),
            connection=_ACCOUNT + 'timezone = "America/New_York"\n',
            expected_conversion_count=1,
            expected_converter_arguments=("TIMEZONE=America/New_York",),
        ),
        LocalEngineE2ETestCase(
            description="a time zone set two ways refuses every test",
            arguments=("test", "--local", "--select", "customer_orders_count_completed"),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "PASS=0  FAIL=0  REFUSED=1  TOTAL=1",
                "session parameter TIMEZONE is set to both",
            ),
            connection=(
                _ACCOUNT + 'timezone = "Europe/Paris"\nsession_parameters = { TIMEZONE = "UTC" }\n'
            ),
        ),
        LocalEngineE2ETestCase(
            description="a session parameter that is not a scalar refuses every test",
            arguments=("test", "--local", "--select", "customer_orders_count_completed"),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "PASS=0  FAIL=0  REFUSED=1  TOTAL=1",
                "session parameter QUERY_TAG has a list value, not a scalar",
            ),
            connection=_ACCOUNT + 'session_parameters = { QUERY_TAG = ["a", "b"] }\n',
        ),
        LocalEngineE2ETestCase(
            description="a session parameter name containing = refuses every test",
            arguments=("test", "--local", "--select", "customer_orders_count_completed"),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "PASS=0  FAIL=0  REFUSED=1  TOTAL=1",
                "session parameter name 'A=B' is not a plain name",
            ),
            connection=_ACCOUNT + 'session_parameters = { "A=B" = 1 }\n',
        ),
        LocalEngineE2ETestCase(
            description="local tests need no connection",
            arguments=("test", "--local", "--select", "customer_orders_count_completed"),
            expected_exit_code=0,
            expected_stdout_fragments=("PASS=1  FAIL=0  TOTAL=1",),
            connection="",
            expected_conversion_count=1,
        ),
        LocalEngineE2ETestCase(
            description="a pre-release converter is refused",
            arguments=("test", "--local"),
            expected_exit_code=1,
            expected_stderr_fragments=("[T007]", "did not report a SQLQuack release version"),
            converter_version="0.2.0-rc1",
        ),
        LocalEngineE2ETestCase(
            description="a conversion that uses up the time limit is a timeout",
            arguments=(
                "test",
                "--local",
                "--test-timeout",
                "1s",
                "--select",
                "customer_orders_count_completed",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=("PASS=0  FAIL=0  TIMEOUT=1  TOTAL=1",),
            converter_mode="slow",
            expected_conversion_count=1,
        ),
        LocalEngineE2ETestCase(
            description="strict mode needs --local",
            arguments=("test", "--strict"),
            expected_exit_code=1,
            expected_stderr_fragments=("[T008]", "--strict applies only to local tests"),
        ),
        LocalEngineE2ETestCase(
            description="a missing converter is named with how to install it",
            arguments=("test", "--local"),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "[T007]",
                "needs the SQLQuack converter",
                "SQLBUILD_SQLQUACK",
            ),
            converter_name="missing-sqlquack",
        ),
        LocalEngineE2ETestCase(
            description="an old converter is refused before any test runs",
            arguments=("test", "--local"),
            expected_exit_code=1,
            expected_stderr_fragments=("[T007]", "SQLQuack 0.1.9", "older than the 0.2.0"),
            converter_version="0.1.9",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_tests_when_testing_locally_then_sqlquack_results_are_reported(
    test_case: LocalEngineE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="duckshop_local",
        repo_files=build_local_engine_project_files(connection=test_case.connection),
    )
    converter_dir: Path = tmp_path / "converter"
    _ = write_fake_sqlquack(
        directory=converter_dir,
        version=test_case.converter_version,
        mode=test_case.converter_mode,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.arguments),
        project_dir=project_dir,
        env={_SQLQUACK_ENV: str(converter_dir / test_case.converter_name)},
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout, result.stdout + result.stderr
    for fragment in test_case.expected_stderr_fragments:
        assert fragment in result.stderr, result.stdout + result.stderr
    for fragment in test_case.unexpected_stdout_fragments:
        assert fragment not in result.stdout, result.stdout
    conversions: tuple[tuple[str, ...], ...] = read_fake_sqlquack_conversions(
        directory=converter_dir
    )
    assert len(conversions) == test_case.expected_conversion_count, conversions
    for argument in test_case.expected_converter_arguments:
        assert all(argument in call for call in conversions), conversions
    policy: str = str(project_dir / _POLICY_FILE)
    assert sum(policy in call for call in conversions) == len(conversions), conversions
    for pattern in test_case.expected_run_files:
        written: list[Path] = list(project_dir.glob(pattern))
        assert written, sorted(project_dir.glob("target/run/tests/**/*"))
        assert all(
            path.read_text(encoding="utf-8").startswith(_LOCAL_STATEMENT_HEADER) for path in written
        ), written


@pytest.mark.skipif(
    not _REAL_SQLQUACK.with_name(_POLICY_FILE).is_file(),
    reason="the sqlquack executable, with its policy file beside it, is not installed",
)
@pytest.mark.parametrize(
    "test_case",
    [
        RealLocalEngineE2ETestCase(
            description="converted tests pass and fail on DuckDB and a refusal is reported",
            expected_stdout_fragments=(
                "PASS=1  FAIL=1  REFUSED=1  TOTAL=3",
                "customer_totals_wrong_sum",
                "differing column TOTAL_AMOUNT: actual=12.50, expected=99.00",
                "no rule converts fn:UNCONVERTIBLE_FN",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_real_sqlquack_when_testing_locally_then_converted_tests_run_on_duckdb(
    test_case: RealLocalEngineE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="duckshop_real_local",
        repo_files=build_real_local_engine_project_files(
            policy=_REAL_SQLQUACK.with_name(_POLICY_FILE).read_text(encoding="utf-8")
        ),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test", "--local"),
        project_dir=project_dir,
        env={_SQLQUACK_ENV: str(_REAL_SQLQUACK)},
    )

    assert result.returncode == 1, result.stdout + result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout, result.stdout + result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        LocalFunctionE2ETestCase(
            description="SQL functions are called by bare name and run as macros",
            arguments=("test", "--local"),
            expected_stdout_fragments=(
                "PASS=1  FAIL=0  REFUSED=2  TOTAL=3",
                "SQLQuack refused the body of project function odd_label: "
                "no rule converts fn:UNCONVERTIBLE_FN",
                "project function tag_label is a python function",
            ),
            expected_declared=(("IS_COMPLETED", "IS_OPEN"),),
            expected_bare_calls=("IS_COMPLETED(", "IS_OPEN("),
            expected_function_bodies=(
                ("IS_COMPLETED", "order_status = 'completed'"),
                ("IS_OPEN", "NOT IS_COMPLETED(order_status)"),
                ("ODD_LABEL", "UNCONVERTIBLE_FN(order_status)"),
            ),
        ),
        LocalFunctionE2ETestCase(
            description="a test calling a Python function is refused before conversion",
            arguments=("test", "--local", "--select", "order_tags_are_refused"),
            expected_stdout_fragments=("PASS=0  FAIL=0  REFUSED=1  TOTAL=1",),
            expected_declared=(),
            expected_bare_calls=(),
            expected_function_bodies=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_functions_when_testing_locally_then_sqlquack_gets_bare_calls(
    test_case: LocalFunctionE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="duckshop_local_functions",
        repo_files=build_local_function_project_files(),
    )
    converter_dir: Path = tmp_path / "converter"
    _ = write_fake_sqlquack(directory=converter_dir, version="0.2.0")

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.arguments),
        project_dir=project_dir,
        env={_SQLQUACK_ENV: str(converter_dir / "sqlquack")},
    )

    assert result.returncode == 1, result.stdout + result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout, result.stdout + result.stderr
    statements: tuple[dict[str, Any], ...] = read_fake_sqlquack_log(
        directory=converter_dir, log="statements"
    )
    sent_sql: str = "\n".join(statement["sql"] for statement in statements).upper()
    assert ".IS_" not in sent_sql, sent_sql
    for call in test_case.expected_bare_calls:
        assert call in sent_sql, sent_sql
    declared: tuple[tuple[str, ...], ...] = tuple(
        sorted(tuple(statement["declared"]) for statement in statements)
    )
    assert declared == test_case.expected_declared, statements
    functions: tuple[dict[str, Any], ...] = read_fake_sqlquack_log(
        directory=converter_dir, log="functions"
    )
    assert (
        tuple(sorted((call["name"], call["body"].strip()) for call in functions))
        == test_case.expected_function_bodies
    ), functions


@pytest.mark.skipif(
    not _REAL_SQLQUACK.with_name(_POLICY_FILE).is_file(),
    reason="the sqlquack executable, with its policy file beside it, is not installed",
)
@pytest.mark.parametrize(
    "test_case",
    [
        RealLocalEngineE2ETestCase(
            description="a function test calling a SQL function converts and passes",
            expected_stdout_fragments=("PASS=1  FAIL=0  TOTAL=1",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_real_sqlquack_when_a_test_calls_a_sql_function_then_it_runs_on_duckdb(
    test_case: RealLocalEngineE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="duckshop_real_local_functions",
        repo_files=build_real_local_function_project_files(
            policy=_REAL_SQLQUACK.with_name(_POLICY_FILE).read_text(encoding="utf-8")
        ),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "test",
            "--local",
            "--select",
            "sku_family_extracts_leading_letters",
        ),
        project_dir=project_dir,
        env={_SQLQUACK_ENV: str(_REAL_SQLQUACK)},
    )

    assert result.returncode == 0, result.stdout + result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout, result.stdout + result.stderr
