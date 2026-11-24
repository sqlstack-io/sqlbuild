"""E2E coverage of audits of sources no model reads: run with loading off or when selected."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    SourceEndAuditBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "source_end_audits"\nadapter = "duckdb"\n\n'
        '[connection]\ndatabase = "warehouse.duckdb"\n\n[defaults]\nmaterialized = "table"\n'
    ),
    "sql/sources/raw.yml": (
        "sources:\n  - name: raw_refunds\n    description: Test source raw_refunds.\n"
        "    schema: main\n    table: raw_refunds\n    columns:\n"
        "      - name: refund_id\n        type: INTEGER\n        audits: [not_null]\n"
    ),
    "sql/models/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1 AS order_id\n",
}


@pytest.mark.parametrize(
    "test_case",
    [
        SourceEndAuditBuildE2ETestCase(
            description="with loading on an unread source is not audited",
            build_arguments=(),
            expected_exit_code=0,
            expected_audit_reported=False,
        ),
        SourceEndAuditBuildE2ETestCase(
            description="with loading off an unread source is audited",
            build_arguments=("--no-load",),
            expected_exit_code=1,
            expected_audit_reported=True,
        ),
        SourceEndAuditBuildE2ETestCase(
            description="a source selected by name is audited",
            build_arguments=("--select", "raw_refunds"),
            expected_exit_code=1,
            expected_audit_reported=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unread_source_with_missing_table_when_building_then_audit_runs_only_when_asked(
    test_case: SourceEndAuditBuildE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="source_end_audits", repo_files=_PROJECT_FILES
    )

    built: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", *test_case.build_arguments), project_dir=project_dir
    )

    assert built.returncode == test_case.expected_exit_code, built.stdout + built.stderr
    assert ("not_null on raw_refunds" in built.stdout) == test_case.expected_audit_reported
