from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    BuildNoTestsNoAuditsFlagE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import (
    prepare_build_test_audit_flag_project,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb


@pytest.mark.parametrize(
    "test_case",
    (
        BuildNoTestsNoAuditsFlagE2ETestCase(
            description="no tests skips SQL tests but runs audits",
            project_name="build_no_tests_project",
            command=("--no-color", "build", "--no-tests"),
            expected_stdout_fragments=("audit     not_null", "check     check_orders_exist"),
            unexpected_stdout_fragments=("test      test_orders",),
        ),
        BuildNoTestsNoAuditsFlagE2ETestCase(
            description="no audits skips audits and Python checks but runs SQL tests",
            project_name="build_no_audits_project",
            command=("--no-color", "build", "--no-audits"),
            expected_stdout_fragments=("test      test_orders",),
            unexpected_stdout_fragments=("audit     not_null", "check_orders_exist"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_build_test_audit_flag_when_building_then_applies_execution_filter(
    test_case: BuildNoTestsNoAuditsFlagE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_build_test_audit_flag_project(
        tmp_path=tmp_path,
        project_name=test_case.project_name,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout
    for fragment in test_case.unexpected_stdout_fragments:
        assert fragment not in result.stdout
