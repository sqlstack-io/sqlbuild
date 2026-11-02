"""E2E coverage of `sqb test --local`: Snowflake tests converted by SQLQuack and run on DuckDB."""

from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import (
    LocalEngineE2ETestCase,
    RealLocalEngineE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.test.helpers import (
    build_local_engine_project_files,
    build_real_local_engine_project_files,
    read_fake_sqlquack_conversions,
    write_fake_sqlquack,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_SQLQUACK_ENV: str = "SQLBUILD_SQLQUACK"
_POLICY_FILE: str = "sqlquack.toml"
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
                "[T006] test 'order_labels_are_refused' was refused by SQLQuack:",
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
        repo_files=build_local_engine_project_files(),
    )
    converter_dir: Path = tmp_path / "converter"
    _ = write_fake_sqlquack(directory=converter_dir, version=test_case.converter_version)

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
        LocalEngineE2ETestCase(
            description="without a project policy the converter's bundled policy judges the tests",
            arguments=("test", "--local", "--select", "customer_orders"),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "policy bundled with SQLQuack 0.2.0 (no project sqlquack.toml)",
            ),
            expected_conversion_count=4,
            expected_converter_arguments=(
                "--bundled-policy",
                "--session",
                "QUOTED_IDENTIFIERS_IGNORE_CASE=true",
            ),
            unexpected_converter_arguments=("--policy", "--builtin-policy"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_no_project_policy_when_testing_locally_then_the_bundled_policy_is_used(
    test_case: LocalEngineE2ETestCase, tmp_path: Path
) -> None:
    files: dict[str, str] = build_local_engine_project_files()
    del files[_POLICY_FILE]
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="duckshop_local", repo_files=files
    )
    converter_dir: Path = tmp_path / "converter"
    _ = write_fake_sqlquack(directory=converter_dir, version=test_case.converter_version)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.arguments),
        project_dir=project_dir,
        env={_SQLQUACK_ENV: str(converter_dir / test_case.converter_name)},
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout + result.stderr, result.stdout + result.stderr
    conversions: tuple[tuple[str, ...], ...] = read_fake_sqlquack_conversions(
        directory=converter_dir
    )
    assert len(conversions) == test_case.expected_conversion_count, conversions
    for argument in test_case.expected_converter_arguments:
        assert all(argument in call for call in conversions), conversions
    for argument in test_case.unexpected_converter_arguments:
        assert not any(argument in call for call in conversions), conversions


@pytest.mark.skipif(not _REAL_SQLQUACK.is_file(), reason="the sqlquack executable is not installed")
@pytest.mark.parametrize(
    "test_case",
    [
        RealLocalEngineE2ETestCase(
            description="the bundled policy matches the target's session and judges every test",
            expected_stdout_fragments=(
                "PASS=1  FAIL=1  REFUSED=1  TOTAL=3",
                "differing column TOTAL_AMOUNT: actual=12.50, expected=99.00",
                "no rule converts fn:UNCONVERTIBLE_FN",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_real_sqlquack_without_project_policy_when_testing_locally_then_tests_are_judged(
    test_case: RealLocalEngineE2ETestCase, tmp_path: Path
) -> None:
    files: dict[str, str] = build_real_local_engine_project_files(policy="")
    del files[_POLICY_FILE]
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="duckshop_real_local", repo_files=files
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
        LocalEngineE2ETestCase(
            description="settings.strict = true refuses assumptions as --strict does",
            arguments=("test", "--local", "--select", "order_zones"),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "PASS=0  FAIL=0  REFUSED=1  TOTAL=1",
                "node:cast relies on assumption session_timezone_utc",
            ),
            unexpected_stdout_fragments=("Assumptions relied on",),
            expected_conversion_count=1,
            expected_converter_arguments=("--strict", "--policy"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_strict_setting_when_testing_locally_then_assumptions_are_refused(
    test_case: LocalEngineE2ETestCase, tmp_path: Path
) -> None:
    files: dict[str, str] = build_local_engine_project_files()
    files["sqlbuild_project.toml"] += "\n[settings]\nstrict = true\n"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="duckshop_local", repo_files=files
    )
    converter_dir: Path = tmp_path / "converter"
    _ = write_fake_sqlquack(directory=converter_dir, version=test_case.converter_version)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.arguments),
        project_dir=project_dir,
        env={_SQLQUACK_ENV: str(converter_dir / test_case.converter_name)},
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout, result.stdout + result.stderr
    for fragment in test_case.unexpected_stdout_fragments:
        assert fragment not in result.stdout, result.stdout
    conversions: tuple[tuple[str, ...], ...] = read_fake_sqlquack_conversions(
        directory=converter_dir
    )
    assert len(conversions) == test_case.expected_conversion_count, conversions
    for argument in test_case.expected_converter_arguments:
        assert all(argument in call for call in conversions), conversions
