"""E2E coverage of Dagster asset checks for every audit attachment kind.

The project has seed, source and model audits, a singular audit inferred to attach to a model,
and a singular audit that runs at the end of the build.
"""

import subprocess
from collections.abc import Iterator
from pathlib import Path

import pytest
from dagster import AssetExecutionContext, AssetKey, ExecuteInProcessResult, materialize

from sqlbuild.integrations.dagster import SqlBuildCliResource, SqlBuildProject, sqlbuild_assets
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    REPO_ROOT,
    prepare_inline_project,
    run_sqb,
)
from tests.e2e.src.sqlbuild.integrations.dagster._test_types import (
    DagsterAuditAttachmentChecksE2ETestCase,
    DagsterSkippedAuditCheckE2ETestCase,
)
from tests.e2e.src.sqlbuild.integrations.dagster.helpers import (
    ATTACHMENT_CHECK_PROJECT_FILES,
    AuditCheckOutcome,
    audit_check_outcome,
)

_SKIPPED_AUDIT_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "skipped_audit_checks"\nadapter = "duckdb"\n\n'
        '[connection]\ndatabase = "warehouse.duckdb"\n\n[defaults]\nmaterialized = "table"\n'
    ),
    "sql/hooks/python/skips.py": (
        "from sqlbuild.hooks import hook\n\n\n"
        "@hook\ndef skip_model(ctx):\n"
        "    '''Test hook skip_model.'''\n"
        "    return ctx.skip(reason='paused')\n"
    ),
    "sql/models/customers.sql": (
        "MODEL (description 'Test model customers.', pre_hooks [python(\"skip_model\")]);\n"
        "SELECT 1 AS customer_id\n"
    ),
    "sql/models/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1 AS order_id\n",
    "sql/audits/singular/orders_have_customers.sql": (
        "AUDIT ();\n"
        'SELECT o.order_id FROM __ref("orders") AS o\n'
        'LEFT JOIN __ref("customers") AS c ON c.customer_id = o.order_id\n'
        "WHERE c.customer_id IS NULL\n"
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterAuditAttachmentChecksE2ETestCase(
            description="build reports every audit attachment kind as checks",
            command=("build",),
            expected_checked_assets=(
                ("main", "products"),
                ("raw_orders",),
                ("main", "orders"),
                ("main", "customers"),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_every_audit_attachment_kind_when_building_through_dagster_then_checks_are_reported(
    test_case: DagsterAuditAttachmentChecksE2ETestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="attachment_checks",
        repo_files=ATTACHMENT_CHECK_PROJECT_FILES,
    )

    sqlbuild_project: SqlBuildProject = SqlBuildProject(
        project_dir=project_dir, sqb_command=(str(REPO_ROOT / ".venv" / "bin" / "sqb"),)
    )
    sqlbuild_project.prepare()

    @sqlbuild_assets(project=sqlbuild_project, required_resource_keys={"sqb"})
    def sqlbuild_attachment_checks(context: AssetExecutionContext) -> Iterator[object]:
        yield from context.resources.sqb.cli(list(test_case.command), context=context).stream()

    result: ExecuteInProcessResult = materialize(
        [sqlbuild_attachment_checks],
        resources={"sqb": SqlBuildCliResource(project_dir=sqlbuild_project)},
    )

    checks: AuditCheckOutcome = audit_check_outcome(
        result=result, assets=sqlbuild_attachment_checks
    )

    assert checks.success is True
    assert {
        AssetKey(list(path)) for path in test_case.expected_checked_assets
    } <= checks.declared_assets
    assert checks.declared <= checks.evaluated
    assert checks.all_passed is True
    assert "integration output degraded" not in capsys.readouterr().err


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterAuditAttachmentChecksE2ETestCase(
            description="audit reports every audit attachment kind as checks",
            command=("audit",),
            expected_checked_assets=(
                ("main", "products"),
                ("raw_orders",),
                ("main", "orders"),
                ("main", "customers"),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_every_audit_attachment_kind_when_auditing_through_dagster_then_checks_are_reported(
    test_case: DagsterAuditAttachmentChecksE2ETestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="attachment_checks",
        repo_files=ATTACHMENT_CHECK_PROJECT_FILES,
    )
    prebuild: subprocess.CompletedProcess[str] = run_sqb(
        command=("build",), project_dir=project_dir
    )

    sqlbuild_project: SqlBuildProject = SqlBuildProject(
        project_dir=project_dir, sqb_command=(str(REPO_ROOT / ".venv" / "bin" / "sqb"),)
    )
    sqlbuild_project.prepare()

    @sqlbuild_assets(project=sqlbuild_project, required_resource_keys={"sqb"})
    def sqlbuild_attachment_checks(context: AssetExecutionContext) -> Iterator[object]:
        yield from context.resources.sqb.cli(list(test_case.command), context=context).stream()

    result: ExecuteInProcessResult = materialize(
        [sqlbuild_attachment_checks],
        resources={"sqb": SqlBuildCliResource(project_dir=sqlbuild_project)},
    )

    checks: AuditCheckOutcome = audit_check_outcome(
        result=result, assets=sqlbuild_attachment_checks
    )

    assert prebuild.returncode == 0, prebuild.stdout + prebuild.stderr
    assert checks.success is True
    assert {
        AssetKey(list(path)) for path in test_case.expected_checked_assets
    } <= checks.declared_assets
    assert checks.declared <= checks.evaluated
    assert checks.all_passed is True
    assert "integration output degraded" not in capsys.readouterr().err


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterSkippedAuditCheckE2ETestCase(
            description="an end audit over a hook-skipped first-run model is not a failed check",
            skipped_check_name="audit__orders_have_customers",
            expected_log_line=(
                "SQLBuild audit orders_have_customers did not run: reads customers, which was not "
                "built"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_skipped_end_audit_when_building_through_dagster_then_no_failed_check_is_reported(
    test_case: DagsterSkippedAuditCheckE2ETestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="skipped_audit_checks",
        repo_files=_SKIPPED_AUDIT_PROJECT_FILES,
    )
    sqlbuild_project: SqlBuildProject = SqlBuildProject(
        project_dir=project_dir, sqb_command=(str(REPO_ROOT / ".venv" / "bin" / "sqb"),)
    )
    sqlbuild_project.prepare()

    @sqlbuild_assets(project=sqlbuild_project, required_resource_keys={"sqb"})
    def sqlbuild_skipped_audit(context: AssetExecutionContext) -> Iterator[object]:
        yield from context.resources.sqb.cli(["build"], context=context).stream()

    result: ExecuteInProcessResult = materialize(
        [sqlbuild_skipped_audit],
        resources={"sqb": SqlBuildCliResource(project_dir=sqlbuild_project)},
    )

    checks: AuditCheckOutcome = audit_check_outcome(result=result, assets=sqlbuild_skipped_audit)
    assert checks.success is True
    assert test_case.skipped_check_name in {name for _, name in checks.declared}
    assert test_case.skipped_check_name not in {name for _, name in checks.evaluated}
    assert checks.all_passed is True
    assert test_case.expected_log_line in capsys.readouterr().err
