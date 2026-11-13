"""E2E tests for janitor --target cleanup and inspection-only --as previews."""

from __future__ import annotations

import re
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.janitor._test_types import (
    JanitorTargetCleanupE2ETestCase,
    JanitorTargetErrorE2ETestCase,
    JanitorTargetPreviewE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.janitor.helpers import (
    EXPIRED_ARCHIVE_NAME,
    list_archive_names,
    prepare_cruft_in_both_targets,
    prepare_two_target_janitor_project,
    probe_fingerprint_count,
    snapshot_schema_contents,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    run_sqb,
    table_exists,
)

SEEDED_PROBE_FINGERPRINT_COUNT: int = 4
ARCHIVE_NAME_PATTERN: re.Pattern[str] = re.compile(
    r"^_sqb_archive__[0-9]{8}t[0-9]{6}z__(?P<logical_name>.+)$"
)


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorTargetPreviewE2ETestCase(
            description="as prod previews prod cruft through the default dev connection",
            janitor_command=("--no-color", "janitor", "--as", "prod"),
            expected_stdout_fragments=(
                "Previewing janitor as target 'prod' through the connection of target 'dev' "
                "(inspection only).",
                "Janitor preview  prod",
                "relations to archive   1",
                "archives to delete     1",
                "prod.customers  ->  prod._sqb_archive__",
                f"prod.{EXPIRED_ARCHIVE_NAME}  archived 2020-01-01 00:00:00",
                "prod._sqlbuild_fingerprints  keep latest 2",
                "Previewed janitor as target 'prod' through the connection of target 'dev'. "
                "Nothing was changed.",
                "Rerun with `--target prod` instead of `--as prod` to apply it.",
            ),
            unexpected_stdout_fragments=("dev.", "Type `", "Archived", "Deleted"),
        ),
        JanitorTargetPreviewE2ETestCase(
            description="as dev with target prod previews dev cruft through the prod connection",
            janitor_command=("--no-color", "janitor", "--target", "prod", "--as", "dev"),
            expected_stdout_fragments=(
                "Previewing janitor as target 'dev' through the connection of target 'prod' "
                "(inspection only).",
                "Janitor preview  dev",
                "dev.customers  ->  dev._sqb_archive__",
                f"dev.{EXPIRED_ARCHIVE_NAME}  archived 2020-01-01 00:00:00",
                "Nothing was changed.",
            ),
            unexpected_stdout_fragments=("prod.", "Type `", "Archived", "Deleted"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cruft_in_two_targets_when_previewing_janitor_as_target_then_nothing_changes(
    test_case: JanitorTargetPreviewE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_cruft_in_both_targets(
        tmp_path=tmp_path, project_name="janitor_target_preview"
    )
    db_path: Path = project_dir / "janitor.duckdb"
    before: tuple[object, ...] = snapshot_schema_contents(db_path=db_path, schemas=("dev", "prod"))

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command, project_dir=project_dir
    )

    after: tuple[object, ...] = snapshot_schema_contents(db_path=db_path, schemas=("dev", "prod"))
    assert result.returncode == 0, result.stdout + result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout
    for fragment in test_case.unexpected_stdout_fragments:
        assert fragment not in result.stdout
    assert after == before


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorTargetPreviewE2ETestCase(
            description="as prod with an early drop request previews the drop and keeps the view",
            janitor_command=(
                "--no-color",
                "janitor",
                "--as",
                "prod",
                "--drop-old-name-view",
                "prod.revenue",
            ),
            expected_stdout_fragments=(
                "└── prod.revenue  -> model:daily_revenue\n    └── drop  now  (requested; ",
                "Nothing was changed.",
            ),
            unexpected_stdout_fragments=("Type `", "Deleted"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_prod_old_name_view_when_previewing_early_drop_as_prod_then_view_is_kept(
    test_case: JanitorTargetPreviewE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_two_target_janitor_project(
        tmp_path=tmp_path, project_name="janitor_target_old_name", model_names=("revenue",)
    )
    origin_build: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--target", "prod"), project_dir=project_dir
    )
    assert origin_build.returncode == 0, origin_build.stdout + origin_build.stderr
    (project_dir / "models" / "revenue.sql").unlink()
    (project_dir / "models" / "daily_revenue.sql").write_text(
        "MODEL (description 'Test model daily_revenue.',\n  migrate_from revenue,\n);\n\nSELECT 1 AS revenue_id\n",
        encoding="utf-8",
    )
    rename_build: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--target", "prod"), project_dir=project_dir
    )
    assert rename_build.returncode == 0, rename_build.stdout + rename_build.stderr
    db_path: Path = project_dir / "janitor.duckdb"
    before: tuple[object, ...] = snapshot_schema_contents(db_path=db_path, schemas=("prod",))

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command, project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout
    for fragment in test_case.unexpected_stdout_fragments:
        assert fragment not in result.stdout
    assert snapshot_schema_contents(db_path=db_path, schemas=("prod",)) == before
    assert ("prod", "revenue", "VIEW") in {entry[:3] for entry in before}


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorTargetCleanupE2ETestCase(
            description="target prod cleans only the prod namespace",
            janitor_command=("--no-color", "janitor", "--target", "prod", "--auto-approve"),
            cleaned_schema="prod",
            untouched_schema="dev",
            expected_stdout_fragments=(
                "Janitor preview  prod",
                "prod.customers  ->  prod._sqb_archive__",
                "\u2713 Janitor completed  archived 1, deleted 1, pruned 4",
            ),
            expected_deleted_names=(EXPIRED_ARCHIVE_NAME, "customers"),
            expected_archived_original_names=("customers",),
            expected_fingerprint_probe_count_after=2,
        ),
        JanitorTargetCleanupE2ETestCase(
            description="default janitor cleans only the default dev namespace",
            janitor_command=("--no-color", "janitor", "--auto-approve"),
            cleaned_schema="dev",
            untouched_schema="prod",
            expected_stdout_fragments=(
                "Janitor preview  dev",
                "dev.customers  ->  dev._sqb_archive__",
                "\u2713 Janitor completed  archived 1, deleted 1, pruned 4",
            ),
            expected_deleted_names=(EXPIRED_ARCHIVE_NAME, "customers"),
            expected_archived_original_names=("customers",),
            expected_fingerprint_probe_count_after=2,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cruft_in_two_targets_when_running_janitor_for_one_target_then_other_is_untouched(
    test_case: JanitorTargetCleanupE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_cruft_in_both_targets(
        tmp_path=tmp_path, project_name="janitor_target_cleanup"
    )
    db_path: Path = project_dir / "janitor.duckdb"
    untouched_before: tuple[object, ...] = snapshot_schema_contents(
        db_path=db_path, schemas=(test_case.untouched_schema,)
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command, project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout
    assert f"{test_case.untouched_schema}." not in result.stdout
    for name in test_case.expected_deleted_names:
        assert not table_exists(db_path=db_path, schema=test_case.cleaned_schema, table_name=name)
    assert table_exists(db_path=db_path, schema=test_case.cleaned_schema, table_name="orders")
    archived: tuple[str, ...] = list_archive_names(db_path=db_path, schema=test_case.cleaned_schema)
    assert tuple(ARCHIVE_NAME_PATTERN.sub(r"\g<logical_name>", name) for name in archived) == (
        test_case.expected_archived_original_names
    )
    assert (
        probe_fingerprint_count(db_path=db_path, schema=test_case.cleaned_schema)
        == test_case.expected_fingerprint_probe_count_after
    )
    assert (
        snapshot_schema_contents(db_path=db_path, schemas=(test_case.untouched_schema,))
        == untouched_before
    )
    assert (
        probe_fingerprint_count(db_path=db_path, schema=test_case.untouched_schema)
        == SEEDED_PROBE_FINGERPRINT_COUNT
    )


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorTargetErrorE2ETestCase(
            description="unknown as target fails clearly",
            janitor_command=("--no-color", "janitor", "--as", "staging"),
            expected_exit_code=1,
            expected_output_fragments=(
                "unknown target 'staging' for janitor --as",
                "Configured targets: dev, prod.",
            ),
        ),
        JanitorTargetErrorE2ETestCase(
            description="unknown selected target fails like other commands",
            janitor_command=("--no-color", "janitor", "--target", "staging"),
            expected_exit_code=1,
            expected_output_fragments=("Unknown target 'staging'",),
        ),
        JanitorTargetErrorE2ETestCase(
            description="as with auto approve is a usage error",
            janitor_command=("--no-color", "janitor", "--as", "prod", "--auto-approve"),
            expected_exit_code=2,
            expected_output_fragments=(
                "janitor --as is inspection only and cannot be combined with --auto-approve",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_target_flags_when_running_janitor_then_fails_without_changes(
    test_case: JanitorTargetErrorE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_two_target_janitor_project(
        tmp_path=tmp_path, project_name="janitor_target_errors", model_names=("orders",)
    )
    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--target", "prod"), project_dir=project_dir
    )
    assert build_result.returncode == 0, build_result.stdout + build_result.stderr
    db_path: Path = project_dir / "janitor.duckdb"
    execute_duckdb(
        db_path=db_path,
        sql=f'CREATE TABLE prod."{EXPIRED_ARCHIVE_NAME}" AS SELECT 1 AS product_id',
    )
    before: tuple[object, ...] = snapshot_schema_contents(db_path=db_path, schemas=("prod",))

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command, project_dir=project_dir
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    for fragment in test_case.expected_output_fragments:
        assert fragment in result.stdout + result.stderr
    assert snapshot_schema_contents(db_path=db_path, schemas=("prod",)) == before


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
