"""Real-CLI coverage of source, seed, and end-of-build audits whose query raises."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    AuditExecutionErrorE2ETestCase,
    AuditExecutionErrorTerminalE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import (
    audit_check_by_name,
    build_asset_statuses,
    prepare_audit_error_project,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        AuditExecutionErrorE2ETestCase(
            description="source audit with error severity at concurrency 1",
            audit_kind="source",
            severity="error",
            concurrency=1,
            expected_attachment_kind="source",
            expected_asset_statuses={"staged_orders": "skipped"},
        ),
        AuditExecutionErrorE2ETestCase(
            description="source audit with error severity at concurrency 4",
            audit_kind="source",
            severity="error",
            concurrency=4,
            expected_attachment_kind="source",
            expected_asset_statuses={"staged_orders": "skipped"},
        ),
        AuditExecutionErrorE2ETestCase(
            description="source audit with warn severity at concurrency 1",
            audit_kind="source",
            severity="warn",
            concurrency=1,
            expected_attachment_kind="source",
            expected_asset_statuses={"staged_orders": "skipped"},
        ),
        AuditExecutionErrorE2ETestCase(
            description="source audit with warn severity at concurrency 4",
            audit_kind="source",
            severity="warn",
            concurrency=4,
            expected_attachment_kind="source",
            expected_asset_statuses={"staged_orders": "skipped"},
        ),
        AuditExecutionErrorE2ETestCase(
            description="seed audit with error severity at concurrency 1",
            audit_kind="seed",
            severity="error",
            concurrency=1,
            expected_attachment_kind="seed",
            expected_asset_statuses={"order_codes": "failed", "coded_orders": "skipped"},
        ),
        AuditExecutionErrorE2ETestCase(
            description="seed audit with error severity at concurrency 4",
            audit_kind="seed",
            severity="error",
            concurrency=4,
            expected_attachment_kind="seed",
            expected_asset_statuses={"order_codes": "failed", "coded_orders": "skipped"},
        ),
        AuditExecutionErrorE2ETestCase(
            description="end audit with error severity at concurrency 1",
            audit_kind="end",
            severity="error",
            concurrency=1,
            expected_attachment_kind="end",
            expected_asset_statuses={"orders": "success", "customers": "success"},
        ),
        AuditExecutionErrorE2ETestCase(
            description="end audit with error severity at concurrency 4",
            audit_kind="end",
            severity="error",
            concurrency=4,
            expected_attachment_kind="end",
            expected_asset_statuses={"orders": "success", "customers": "success"},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_audit_query_raises_when_building_then_it_reports_a_failed_audit(
    test_case: AuditExecutionErrorE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_audit_error_project(
        tmp_path=tmp_path, audit_kind=test_case.audit_kind, severity=test_case.severity
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--json", "--concurrency", str(test_case.concurrency)),
        project_dir=project_dir,
    )

    assert result.returncode == 1, result.stdout + result.stderr
    assert "Traceback" not in result.stderr
    assert json.loads(result.stdout)["status"] == "failed"
    check: dict[str, object] = audit_check_by_name(stdout=result.stdout, name="broken_check")
    assert (check["attachment_kind"], check["status"], check["severity"]) == (
        test_case.expected_attachment_kind,
        "error",
        test_case.severity,
    )
    assert "missing_lookup" in str(check["execution_error"])
    statuses: dict[str, str] = build_asset_statuses(result.stdout)
    assert {name: statuses.get(name) for name in test_case.expected_asset_statuses} == (
        test_case.expected_asset_statuses
    )


@pytest.mark.parametrize(
    "test_case",
    [
        AuditExecutionErrorTerminalE2ETestCase(
            description="source audit error is listed with its message",
            audit_kind="source",
            expected_fragments=(
                "Failures:",
                "broken_check on raw_orders  (audit)",
                "missing_lookup",
                "PASS=0  WARN=0  FAIL=1  INSUFFICIENT=0  SKIP=1  TOTAL=2",
            ),
        ),
        AuditExecutionErrorTerminalE2ETestCase(
            description="seed audit error is listed and fails its seed",
            audit_kind="seed",
            expected_fragments=(
                "Failures:",
                "broken_check on order_codes  (audit)",
                "order_codes  (seed)",
                "missing_lookup",
                "PASS=0  WARN=0  FAIL=2  INSUFFICIENT=0  SKIP=1  TOTAL=3",
            ),
        ),
        AuditExecutionErrorTerminalE2ETestCase(
            description="end-of-build audit error is listed with its message",
            audit_kind="end",
            expected_fragments=(
                "Failures:",
                "broken_check  (audit)",
                "missing_lookup",
                "PASS=2  WARN=0  FAIL=1  INSUFFICIENT=0  SKIP=0  TOTAL=3",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_audit_query_raises_when_building_in_terminal_then_failure_is_listed(
    test_case: AuditExecutionErrorTerminalE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_audit_error_project(
        tmp_path=tmp_path, audit_kind=test_case.audit_kind, severity="error"
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == 1, output
    assert "Traceback" not in output
    for fragment in test_case.expected_fragments:
        assert fragment in output
