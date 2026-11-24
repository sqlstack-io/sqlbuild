"""CLI e2e coverage for `sqb rename` renaming several models in one verified step."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.refactor._test_types import (
    BatchRenameE2ETestCase,
    BatchRenameRefusalE2ETestCase,
    RenameUsageErrorE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.refactor.helpers import (
    MOVE_FILE_PLACEHOLDER,
    existing_files,
    fragments_present,
    project_text,
    relation_type,
    sqb,
    with_move_file,
    write_orders_project,
)

_COMPILE_STARTS: tuple[str, ...] = (
    "Compiling project...",
    "Verifying the edited project compiles...",
)
_RENAMED_FILES: tuple[str, ...] = (
    "sql/models/marts/fact_order_lines.sql",
    "sql/models/staging/stg_order_lines.sql",
)
_RENAMED_FRAGMENTS: dict[str, tuple[str, ...]] = {
    "sql/models/marts/fact_order_lines.sql": (
        '__ref("stg_order_lines")',
        "migrate_from fact_orders",
    ),
    "sql/models/staging/stg_order_lines.sql": ("migrate_from stg_orders",),
    "sql/models/marts/customer_totals.sql": ('__ref("fact_order_lines")',),
    "tests/unit/test_fact_order_lines.sql": (
        "__ref__stg_order_lines AS (",
        "__expected__fact_order_lines AS (",
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        BatchRenameE2ETestCase(
            description="pairs on the command line",
            arguments=("stg_orders", "stg_order_lines", "model:fact_orders", "fact_order_lines"),
            rename_file="",
            expected_files=_RENAMED_FILES,
            expected_fragments=_RENAMED_FRAGMENTS,
            expected_old_name_views=("stg_orders", "fact_orders"),
        ),
        BatchRenameE2ETestCase(
            description="pairs from a file with comments and blank lines",
            arguments=("--from-file", MOVE_FILE_PLACEHOLDER),
            rename_file=(
                "# rename the order models together\n\n"
                "stg_orders\tstg_order_lines\n  fact_orders   fact_order_lines  \n"
            ),
            expected_files=_RENAMED_FILES,
            expected_fragments=_RENAMED_FRAGMENTS,
            expected_old_name_views=("stg_orders", "fact_orders"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_several_renames_when_renamed_together_then_one_verification_applies_all(
    tmp_path: Path, test_case: BatchRenameE2ETestCase
) -> None:
    """Both models rename at once, keep their history, and leave old-name views after a build."""

    project_dir: Path = write_orders_project(tmp_path=tmp_path)
    rename_file: Path = tmp_path / "renames.tsv"
    _ = rename_file.write_text(test_case.rename_file, encoding="utf-8")
    first_build: subprocess.CompletedProcess[str] = sqb(project_dir, "build")

    renamed: subprocess.CompletedProcess[str] = sqb(
        project_dir, "rename", *with_move_file(test_case.arguments, rename_file), "--json"
    )
    payload: dict[str, Any] = json.loads(renamed.stdout)
    built: subprocess.CompletedProcess[str] = sqb(project_dir, "build")

    assert first_build.returncode == 0, first_build.stdout + first_build.stderr
    assert renamed.returncode == 0, renamed.stdout + renamed.stderr
    assert payload["status"] == "applied"
    assert [item["new_name"] for item in payload["renames"]] == [
        "stg_order_lines",
        "fact_order_lines",
    ]
    assert sum(renamed.stderr.count(start) for start in _COMPILE_STARTS) == 2, renamed.stderr
    assert existing_files(
        project_dir=project_dir,
        paths=(
            "sql/models/staging/stg_orders.sql",
            "sql/models/marts/fact_orders.sql",
            *test_case.expected_files,
        ),
    ) == tuple(sorted(test_case.expected_files))
    assert fragments_present(
        files=project_text(project_dir), expected=test_case.expected_fragments
    ), project_text(project_dir)
    assert built.returncode == 0, built.stdout + built.stderr
    assert {
        name: relation_type(project_dir=project_dir, name=name)
        for name in test_case.expected_old_name_views
    } == dict.fromkeys(test_case.expected_old_name_views, "VIEW")


@pytest.mark.parametrize(
    "test_case",
    [
        BatchRenameRefusalE2ETestCase(
            description="a chain that gives a model the name another one gives up",
            arguments=("stg_orders", "stg_order_lines", "fact_orders", "stg_orders"),
            expected_message=(
                "model:fact_orders would take the name model:stg_orders gives up in this batch"
            ),
        ),
        BatchRenameRefusalE2ETestCase(
            description="two models given one name",
            arguments=("stg_orders", "order_lines", "fact_orders", "order_lines"),
            expected_message="would both be named order_lines",
        ),
        BatchRenameRefusalE2ETestCase(
            description="a model renamed twice",
            arguments=("stg_orders", "order_lines", "stg_orders", "order_items"),
            expected_message="model:stg_orders is renamed more than once in this batch",
        ),
        BatchRenameRefusalE2ETestCase(
            description="a column rename in a batch",
            arguments=("stg_orders", "stg_order_lines", "fact_orders.amount", "revenue"),
            expected_message=(
                "'fact_orders.amount' names a column; several renames in one sqb rename "
                "must all be models"
            ),
        ),
        BatchRenameRefusalE2ETestCase(
            description="--cascade with several renames",
            arguments=("stg_orders.amount", "revenue", "fact_orders", "x", "--cascade"),
            expected_message="--cascade applies to one column rename, not to several renames",
        ),
        BatchRenameRefusalE2ETestCase(
            description="a target without a new name",
            arguments=("stg_orders", "stg_order_lines", "fact_orders"),
            expected_message="'fact_orders' has no NEW_NAME",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_batch_rename_when_renaming_then_it_is_refused_and_nothing_changes(
    tmp_path: Path, test_case: BatchRenameRefusalE2ETestCase
) -> None:
    project_dir: Path = write_orders_project(tmp_path=tmp_path)
    before: dict[str, str] = project_text(project_dir)

    renamed: subprocess.CompletedProcess[str] = sqb(project_dir, "rename", *test_case.arguments)

    assert renamed.returncode == 1, renamed.stdout + renamed.stderr
    assert test_case.expected_message in " ".join((renamed.stdout + renamed.stderr).split())
    assert project_text(project_dir) == before


@pytest.mark.parametrize(
    "test_case",
    [
        RenameUsageErrorE2ETestCase(
            description="a target without a new name",
            arguments=("stg_orders",),
            expected_returncode=2,
            expected_message="the following arguments are required: NEW_NAME",
        ),
        RenameUsageErrorE2ETestCase(
            description="no target and no file",
            arguments=(),
            expected_returncode=2,
            expected_message="the following arguments are required: TARGET, NEW_NAME",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_rename_without_a_pair_when_parsing_then_it_is_a_usage_error(
    tmp_path: Path, test_case: RenameUsageErrorE2ETestCase
) -> None:
    project_dir: Path = write_orders_project(tmp_path=tmp_path)

    renamed: subprocess.CompletedProcess[str] = sqb(project_dir, "rename", *test_case.arguments)

    assert renamed.returncode == test_case.expected_returncode, renamed.stdout + renamed.stderr
    assert test_case.expected_message in renamed.stderr, renamed.stderr
