"""E2E tests for the sqb check command."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.check._test_types import (
    CheckCommandTestCase,
    ReadSidePythonCheckCommandTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.check.helpers import (
    assert_expected_file_fragments,
    prepare_check_project_by_kind,
    prepare_python_check_project,
    resolve_check_command,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import query_duckdb, run_sqb


@pytest.mark.parametrize(
    "test_case",
    (
        CheckCommandTestCase(
            description="runs all checks without implicit dependency execution",
            command=("--no-color", "check"),
            expected_returncode=1,
            expected_stdout_fragments=(
                "Execution  sqb check",
                "Python checks",
                "check_orders_export",
                "warn_orders_export",
                "WARN",
                "fail_orders_export",
                "FAIL",
                "false_orders_export",
                "exception_orders_export",
                "orders exception check failed",
                "PASS=1  WARN=1  FAIL=5  TOTAL=7",
            ),
        ),
        CheckCommandTestCase(
            description="runs directly selected check without dependency execution",
            command=("--no-color", "check", "--select", "check:check_orders_export"),
            expected_returncode=0,
            expected_stdout_fragments=(
                "Python checks",
                "check_orders_export",
                "PASS",
                "PASS=1  WARN=0  FAIL=0  TOTAL=1",
            ),
        ),
        CheckCommandTestCase(
            description="rejects graph operator check selector",
            command=("--no-color", "check", "--select", "+check:check_orders_export"),
            expected_returncode=1,
            expected_stdout_fragments=("sqb check selectors do not support graph operators",),
        ),
        CheckCommandTestCase(
            description="build executes relevant checks",
            command=("--no-color", "build", "--exclude", "tag:failure"),
            expected_returncode=0,
            expected_stdout_fragments=(
                "Python checks",
                "check_orders_export",
                "check_orders_asset",
                "check_order_customer_exports",
                "warn_orders_export",
                "WARN",
            ),
        ),
        CheckCommandTestCase(
            description="failing check fails build",
            command=("--no-color", "build"),
            expected_returncode=1,
            expected_stdout_fragments=(
                "Python checks",
                "fail_orders_export",
                "FAIL",
                "orders export failed",
            ),
        ),
        CheckCommandTestCase(
            description="selected false check fails check command",
            command=("--no-color", "check", "--select", "check:false_orders_export"),
            expected_returncode=1,
            expected_stdout_fragments=(
                "false_orders_export",
                "FAIL",
                "PASS=0  WARN=0  FAIL=1  TOTAL=1",
            ),
        ),
        CheckCommandTestCase(
            description="selected exception check fails check command",
            command=("--no-color", "check", "--select", "check:exception_orders_export"),
            expected_returncode=1,
            expected_stdout_fragments=(
                "exception_orders_export",
                "orders exception check failed",
                "PASS=0  WARN=0  FAIL=1  TOTAL=1",
            ),
        ),
        CheckCommandTestCase(
            description="build json includes Python check results",
            command=("--no-color", "build", "--json", "--exclude", "tag:failure"),
            expected_returncode=0,
            expected_stdout_fragments=(
                '"kind": "python_check"',
                '"name": "check_orders_asset"',
                '"python_check_warn_count": 1',
            ),
        ),
        CheckCommandTestCase(
            description="build json marks failed Python checks",
            command=("--no-color", "build", "--json"),
            expected_returncode=1,
            expected_stdout_fragments=(
                '"status": "failed"',
                '"kind": "python_check"',
                '"name": "false_orders_export"',
                '"python_check_fail_count": 3',
            ),
        ),
        CheckCommandTestCase(
            description="tag selector runs multi-dependency check",
            command=("--no-color", "check", "--select", "tag:multi"),
            expected_returncode=1,
            expected_stdout_fragments=(
                "check_order_customer_exports",
                "No persisted result found for Python node 'export_orders'",
                "PASS=0  WARN=0  FAIL=1  TOTAL=1",
            ),
        ),
        CheckCommandTestCase(
            description="json output includes selected Python check",
            command=("--no-color", "check", "--json", "--select", "check:check_orders_export"),
            expected_returncode=0,
            expected_stdout_fragments=(
                '"checks"',
                '"name": "check_orders_export"',
                '"status": "pass"',
                '"metadata": {',
            ),
        ),
        CheckCommandTestCase(
            description="json output path writes selected Python check",
            command=(
                "--no-color",
                "check",
                "--json-output",
                "{project_dir}/target/check.json",
                "--select",
                "check:check_orders_export",
            ),
            expected_returncode=0,
            expected_stdout_fragments=("PASS=1  WARN=0  FAIL=0  TOTAL=1",),
            expected_file_fragments=(
                (
                    "target/check.json",
                    (
                        '"command": "check"',
                        '"name": "check_orders_export"',
                        '"check_id": "python_check:check_orders_export"',
                    ),
                ),
            ),
        ),
        CheckCommandTestCase(
            description="check command executes terminal loader check",
            command=("--no-color", "check"),
            expected_returncode=0,
            expected_stdout_fragments=(
                "check_raw_orders_loader",
                "PASS=1  WARN=0  FAIL=0  TOTAL=1",
            ),
            project_kind="terminal_loader",
        ),
        CheckCommandTestCase(
            description="build without tests or audits skips Python checks",
            command=(
                "--no-color",
                "build",
                "--no-tests",
                "--no-audits",
                "--exclude",
                "tag:failure",
            ),
            expected_returncode=0,
            expected_stdout_fragments=("Execution  sqb build",),
            expected_absent_fragments=("Python checks", "check_orders_export"),
        ),
        CheckCommandTestCase(
            description="audit remains SQL audit only",
            command=("--no-color", "audit"),
            expected_returncode=0,
            expected_stdout_fragments=("Execution  sqb audit",),
            expected_absent_fragments=("Python checks", "check_orders_export"),
        ),
        CheckCommandTestCase(
            description="build no python suppresses read-side checks",
            command=("--no-color", "build", "--no-python"),
            expected_returncode=0,
            expected_stdout_fragments=("Completed successfully",),
            expected_absent_fragments=("Python checks", "check_orders_export"),
        ),
        CheckCommandTestCase(
            description="check command writes runtime target artifact",
            command=("--no-color", "check", "--select", "check:check_orders_export"),
            expected_returncode=0,
            expected_stdout_fragments=("check_orders_export",),
            expected_file_fragments=(
                (
                    "target/run/checks/python_checks.json",
                    (
                        '"kind": "python_check"',
                        '"display_name": "check_orders_export"',
                    ),
                ),
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_python_checks_when_running_check_then_reports_expected_results(
    tmp_path: Path, test_case: CheckCommandTestCase
) -> None:
    project_dir: Path = prepare_check_project_by_kind(
        tmp_path=tmp_path,
        project_kind=test_case.project_kind,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=resolve_check_command(project_dir=project_dir, command=test_case.command),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_returncode
    combined_output: str = result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in combined_output
    for fragment in test_case.expected_absent_fragments:
        assert fragment not in combined_output
    assert_expected_file_fragments(project_dir=project_dir, test_case=test_case)


@pytest.mark.parametrize(
    "test_case",
    [
        CheckCommandTestCase(
            description="direct selected Python check persists check identity",
            command=("--no-color", "check", "--select", "check_orders_export"),
            expected_returncode=0,
            expected_stdout_fragments=("check_orders_export",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_successful_python_check_when_running_check_then_persists_check_identity(
    tmp_path: Path,
    test_case: CheckCommandTestCase,
) -> None:
    project_dir: Path = prepare_python_check_project(tmp_path=tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_returncode, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout
    assert query_duckdb(
        db_path=project_dir / "python_check_project.duckdb",
        sql=(
            "SELECT node_type, node_name FROM main._sqlbuild_fingerprints "
            "WHERE node_type = 'check' ORDER BY node_name"
        ),
    ) == [("check", "check_orders_export")]
    assert query_duckdb(
        db_path=project_dir / "python_check_project.duckdb",
        sql=(
            "SELECT node_type, node_name, status, metadata_json_b64 "
            "FROM main._sqlbuild_node_results "
            "WHERE node_type = 'check' AND node_name = 'check_orders_export'"
        ),
    ) == [("check", "check_orders_export", "success", "e30=")]


@pytest.mark.parametrize(
    "test_case",
    [
        CheckCommandTestCase(
            description="direct Python checks persist warn and fail result rows",
            command=("--no-color", "check", "--select", "check:warn_orders_export"),
            expected_returncode=0,
            expected_stdout_fragments=("warn_orders_export", "WARN"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_warning_and_failing_python_checks_when_running_check_then_persists_status_rows(
    tmp_path: Path,
    test_case: CheckCommandTestCase,
) -> None:
    project_dir: Path = prepare_python_check_project(tmp_path=tmp_path)

    warn_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )
    fail_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "check", "--select", "check:fail_orders_export"),
        project_dir=project_dir,
    )

    assert warn_result.returncode == test_case.expected_returncode, (
        warn_result.stdout + warn_result.stderr
    )
    fragment: str
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in warn_result.stdout
    assert fail_result.returncode == 1, fail_result.stdout + fail_result.stderr
    assert query_duckdb(
        db_path=project_dir / "python_check_project.duckdb",
        sql=(
            "SELECT node_name, status, error_message "
            "FROM main._sqlbuild_node_results "
            "WHERE node_type = 'check' AND node_name IN "
            "('warn_orders_export', 'fail_orders_export') "
            "ORDER BY node_name"
        ),
    ) == [
        ("fail_orders_export", "failed", "orders export failed"),
        ("warn_orders_export", "warn", "warning check failed"),
    ]


@pytest.mark.parametrize(
    "test_case",
    [
        ReadSidePythonCheckCommandTestCase(
            description="Python check reads persisted dependency result after producer build",
            missing_command=("--no-color", "check", "--select", "tag:multi"),
            build_command=(
                "--no-color",
                "build",
                "--select",
                "export_orders export_customers",
                "--exclude",
                "tag:failure",
            ),
            check_command=("--no-color", "check", "--select", "tag:multi"),
            expected_missing_returncode=1,
            expected_build_returncode=0,
            expected_check_returncode=0,
            expected_missing_fragments=(
                "No persisted result found for Python node 'export_orders'",
            ),
            expected_check_fragments=(
                "check_order_customer_exports",
                "PASS=1  WARN=0  FAIL=0  TOTAL=1",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_check_dependency_result_when_persisted_then_check_reads_result(
    tmp_path: Path,
    test_case: ReadSidePythonCheckCommandTestCase,
) -> None:
    project_dir: Path = prepare_python_check_project(tmp_path=tmp_path)

    missing_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.missing_command,
        project_dir=project_dir,
    )
    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.build_command,
        project_dir=project_dir,
    )
    check_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.check_command,
        project_dir=project_dir,
    )

    missing_output: str = missing_result.stdout + missing_result.stderr
    assert missing_result.returncode == test_case.expected_missing_returncode
    fragment: str
    for fragment in test_case.expected_missing_fragments:
        assert fragment in missing_output
    assert build_result.returncode == test_case.expected_build_returncode, (
        build_result.stdout + build_result.stderr
    )
    check_output: str = check_result.stdout + check_result.stderr
    assert check_result.returncode == test_case.expected_check_returncode, check_output
    for fragment in test_case.expected_check_fragments:
        assert fragment in check_output
