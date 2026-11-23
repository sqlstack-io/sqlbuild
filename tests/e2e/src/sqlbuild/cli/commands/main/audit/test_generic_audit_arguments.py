"""CLI e2e coverage: generic audit arguments are checked and their rendered SQL is compiled."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.audit._test_types import (
    GenericAuditArgumentE2ETestCase,
    RenderedGenericAuditE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.audit.helpers import (
    GENERIC_AUDIT_PATH,
    generic_audit_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    (
        GenericAuditArgumentE2ETestCase(
            description="an argument the audit never reads names itself for removal",
            arguments='group_columns "channel", where_condition "channel = \'web\'"',
            expected_message=(
                "sql/models/orders.sql audit 'unique_within_group' passes argument "
                "'where_condition', which the audit's SQL in "
                f"{GENERIC_AUDIT_PATH} never uses; remove 'where_condition' from the attachment"
            ),
        ),
        GenericAuditArgumentE2ETestCase(
            description="a misspelt argument is reported as unused",
            arguments='group_columns "channel", group_column "channel"',
            expected_message="passes argument 'group_column', which the audit's SQL in",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_unused_generic_audit_argument_when_compiling_then_compile_fails(
    test_case: GenericAuditArgumentE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="audit_arguments",
        repo_files=generic_audit_project_files(arguments=test_case.arguments),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    assert result.returncode == 1, result.stdout + result.stderr
    assert test_case.expected_message in result.stdout + result.stderr


@pytest.mark.parametrize(
    "test_case",
    (
        RenderedGenericAuditE2ETestCase(
            description="every argument read by the audit compiles and renders",
            arguments='group_columns "channel"',
            detached_column=" (type VARCHAR)",
            expected_relative_path=(
                "target/compiled/audits/generic/orders/unique_within_group__status.sql"
            ),
            expected_sql=(
                "SELECT channel, COUNT(DISTINCT status) AS distinct_count\n"
                'FROM __ref("orders")\nWHERE status IS NOT NULL\nGROUP BY channel\n'
                "HAVING COUNT(DISTINCT status) > 1\n"
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_attached_generic_audit_when_compiling_then_rendered_sql_is_written_and_cleaned(
    test_case: RenderedGenericAuditE2ETestCase, tmp_path: Path
) -> None:
    """Compile writes each audit's rendered SQL beside the models and removes it once detached."""

    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="audit_arguments",
        repo_files=generic_audit_project_files(arguments=test_case.arguments),
    )
    audit_file: Path = project_dir / test_case.expected_relative_path

    first: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )
    rendered: str = audit_file.read_text(encoding="utf-8")
    model_path: Path = project_dir / "sql/models/orders.sql"
    _ = model_path.write_text(
        model_path.read_text(encoding="utf-8").replace(
            f" (audits [unique_within_group ({test_case.arguments})])",
            test_case.detached_column,
        ),
        encoding="utf-8",
    )
    (project_dir / GENERIC_AUDIT_PATH).unlink()
    second: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    assert first.returncode == 0, first.stdout + first.stderr
    assert rendered == test_case.expected_sql
    assert second.returncode == 0, second.stdout + second.stderr
    assert not audit_file.exists()
    assert (project_dir / "target/compiled/models/orders.sql").is_file()
