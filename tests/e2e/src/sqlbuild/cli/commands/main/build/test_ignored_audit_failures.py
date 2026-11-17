"""Real-CLI coverage of builds that let failing audits through instead of holding work back."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    IgnoredAuditFailureCommandE2ETestCase,
    IgnoredAuditFailureE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import (
    IGNORED_AUDITS_DATABASE,
    INCREMENTAL_AUDIT_FILES,
    INCREMENTAL_AUDIT_SEED_SQL,
    SOURCE_AUDIT_FILES,
    SOURCE_AUDIT_SEED_SQL,
    STAGED_AUDIT_FILES,
    ignored_audits_project,
    recorded_audit_outcomes,
    warehouse_table_rows,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb

_NO_SEED: str = "SELECT 1"
_IGNORING_SETTINGS: str = "\n[settings]\nignore_audit_failures = true\n"
_DEV_TARGET: str = '\n[targets.dev]\nschema = "main"\nignore_audit_failures = {value}\n'
_PLAN_NOTICE: str = "Audit failures won't stop this build"
_FAILED_LINE: str = "Completed with errors"
_STAGED_HELD: dict[str, int] = {"daily__staging": 1}
_STAGED_BUILT: dict[str, int] = {"daily": 1, "daily_summary": 1}
_STAGED_FAILED: tuple[tuple[str, str, bool], ...] = (("total_is_negative", "error", False),)
_STAGED_IGNORED: tuple[tuple[str, str, bool], ...] = (("total_is_negative", "error", True),)
_STAGED_IGNORED_LINES: tuple[str, ...] = (
    "audit total_is_negative returned 1 row (ignored)",
    "Completed successfully · 1 audit failure ignored",
)


@pytest.mark.parametrize(
    "test_case",
    [
        IgnoredAuditFailureE2ETestCase(
            description="a failing final audit keeps a staged table from replacing its target",
            project_files=STAGED_AUDIT_FILES,
            seed_sql=_NO_SEED,
            settings_toml="",
            build_args=(),
            expected_exit_code=1,
            expected_tables=_STAGED_HELD,
            expected_audit_results=_STAGED_FAILED,
            expected_fragments=(_FAILED_LINE,),
        ),
        IgnoredAuditFailureE2ETestCase(
            description="an ignored final audit failure promotes the staged table and builds downstream",
            project_files=STAGED_AUDIT_FILES,
            seed_sql=_NO_SEED,
            settings_toml="",
            build_args=("--ignore-audit-failures",),
            expected_exit_code=0,
            expected_tables=_STAGED_BUILT,
            expected_audit_results=_STAGED_IGNORED,
            expected_fragments=(_PLAN_NOTICE, *_STAGED_IGNORED_LINES),
        ),
        IgnoredAuditFailureE2ETestCase(
            description="a failing delta audit keeps an incremental batch out of its target",
            project_files=INCREMENTAL_AUDIT_FILES,
            seed_sql=INCREMENTAL_AUDIT_SEED_SQL,
            settings_toml="",
            build_args=(),
            expected_exit_code=1,
            expected_tables={"orders": 1, "orders__delta": 2, "raw_orders": 2},
            expected_audit_results=(("amount_is_negative", "error", False),),
            expected_fragments=(_FAILED_LINE,),
        ),
        IgnoredAuditFailureE2ETestCase(
            description="an ignored delta audit failure applies the incremental batch",
            project_files=INCREMENTAL_AUDIT_FILES,
            seed_sql=INCREMENTAL_AUDIT_SEED_SQL,
            settings_toml="",
            build_args=("--ignore-audit-failures",),
            expected_exit_code=0,
            expected_tables={"orders": 3, "raw_orders": 2},
            expected_audit_results=(
                ("amount_is_negative", "error", True),
                ("amount_is_negative", "error", True),
            ),
            expected_fragments=(
                _PLAN_NOTICE,
                "audit amount_is_negative returned 2 rows (ignored)",
                "Completed successfully · 2 audit failures ignored",
            ),
        ),
        IgnoredAuditFailureE2ETestCase(
            description="a failing source audit blocks the models that read the source",
            project_files=SOURCE_AUDIT_FILES,
            seed_sql=SOURCE_AUDIT_SEED_SQL,
            settings_toml="",
            build_args=(),
            expected_exit_code=1,
            expected_tables={"raw_orders": 2},
            expected_audit_results=(("raw_amount_is_negative", "error", False),),
            expected_fragments=(_FAILED_LINE,),
        ),
        IgnoredAuditFailureE2ETestCase(
            description="an ignored source audit failure lets the models that read the source build",
            project_files=SOURCE_AUDIT_FILES,
            seed_sql=SOURCE_AUDIT_SEED_SQL,
            settings_toml="",
            build_args=("--ignore-audit-failures",),
            expected_exit_code=0,
            expected_tables={"order_count": 1, "raw_orders": 2},
            expected_audit_results=(("raw_amount_is_negative", "error", True),),
            expected_fragments=(
                "audit raw_amount_is_negative returned 2 rows (ignored)",
                "Completed successfully · 1 audit failure ignored",
            ),
        ),
        IgnoredAuditFailureE2ETestCase(
            description="the project setting ignores audit failures without the flag",
            project_files=STAGED_AUDIT_FILES,
            seed_sql=_NO_SEED,
            settings_toml=_IGNORING_SETTINGS,
            build_args=(),
            expected_exit_code=0,
            expected_tables=_STAGED_BUILT,
            expected_audit_results=_STAGED_IGNORED,
            expected_fragments=(_PLAN_NOTICE, *_STAGED_IGNORED_LINES),
        ),
        IgnoredAuditFailureE2ETestCase(
            description="the negated flag lets audit failures block over the project setting",
            project_files=STAGED_AUDIT_FILES,
            seed_sql=_NO_SEED,
            settings_toml=_IGNORING_SETTINGS,
            build_args=("--no-ignore-audit-failures",),
            expected_exit_code=1,
            expected_tables=_STAGED_HELD,
            expected_audit_results=_STAGED_FAILED,
            expected_fragments=(_FAILED_LINE,),
        ),
        IgnoredAuditFailureE2ETestCase(
            description="a target that keeps audit failures blocking overrides the project setting",
            project_files=STAGED_AUDIT_FILES,
            seed_sql=_NO_SEED,
            settings_toml=_IGNORING_SETTINGS + _DEV_TARGET.format(value="false"),
            build_args=("--target", "dev"),
            expected_exit_code=1,
            expected_tables=_STAGED_HELD,
            expected_audit_results=_STAGED_FAILED,
            expected_fragments=(_FAILED_LINE,),
        ),
        IgnoredAuditFailureE2ETestCase(
            description="the flag ignores audit failures over a target that keeps them blocking",
            project_files=STAGED_AUDIT_FILES,
            seed_sql=_NO_SEED,
            settings_toml=_DEV_TARGET.format(value="false"),
            build_args=("--target", "dev", "--ignore-audit-failures"),
            expected_exit_code=0,
            expected_tables=_STAGED_BUILT,
            expected_audit_results=_STAGED_IGNORED,
            expected_fragments=_STAGED_IGNORED_LINES,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_failing_audit_when_building_then_it_blocks_unless_its_failure_is_ignored(
    test_case: IgnoredAuditFailureE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = ignored_audits_project(
        tmp_path, test_case.project_files, test_case.seed_sql, test_case.settings_toml
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", *test_case.build_args), project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    database: Path = project_dir / IGNORED_AUDITS_DATABASE
    assert result.returncode == test_case.expected_exit_code, output
    assert warehouse_table_rows(database) == test_case.expected_tables, output
    assert recorded_audit_outcomes(database) == test_case.expected_audit_results, output
    assert all(fragment in output for fragment in test_case.expected_fragments), output


@pytest.mark.parametrize(
    "test_case",
    [
        IgnoredAuditFailureCommandE2ETestCase(
            description="ignoring audit failures while running no audits is refused",
            settings_toml="",
            command=("build", "--ignore-audit-failures", "--no-audits"),
            expected_exit_code=1,
            expected_fragments=("C419", "cannot be combined with --no-audits"),
            expected_missing_fragments=("Plan ready",),
        ),
        IgnoredAuditFailureCommandE2ETestCase(
            description="a plan made with the flag says so at the top",
            settings_toml="",
            command=("plan", "--ignore-audit-failures"),
            expected_exit_code=0,
            expected_fragments=(f"Plan ready  2 selected\n{_PLAN_NOTICE}",),
            expected_missing_fragments=(),
        ),
        IgnoredAuditFailureCommandE2ETestCase(
            description="a plan made under the project setting says so at the top",
            settings_toml=_IGNORING_SETTINGS,
            command=("plan",),
            expected_exit_code=0,
            expected_fragments=(_PLAN_NOTICE,),
            expected_missing_fragments=(),
        ),
        IgnoredAuditFailureCommandE2ETestCase(
            description="a plan without it says nothing about audit failures",
            settings_toml="",
            command=("plan",),
            expected_exit_code=0,
            expected_fragments=("Plan ready",),
            expected_missing_fragments=(_PLAN_NOTICE,),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_ignored_audit_failures_when_planning_or_combining_flags_then_the_command_says_so(
    test_case: IgnoredAuditFailureCommandE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = ignored_audits_project(
        tmp_path, STAGED_AUDIT_FILES, _NO_SEED, test_case.settings_toml
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.command), project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_exit_code, output
    assert all(fragment in output for fragment in test_case.expected_fragments), output
    assert not any(fragment in output for fragment in test_case.expected_missing_fragments), output
