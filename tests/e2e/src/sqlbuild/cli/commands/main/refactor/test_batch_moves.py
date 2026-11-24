"""CLI e2e coverage for `sqb mv` moving several models in one verified step."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.refactor._test_types import (
    BatchMoveConflictE2ETestCase,
    BatchMoveDryRunE2ETestCase,
    BatchMoveE2ETestCase,
    SingleMoveOfBatchE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.refactor.helpers import (
    MOVE_FILE_PLACEHOLDER,
    existing_files,
    project_text,
    relation_type,
    same_named_local_macros,
    shared_cents_with_grouped_global_role,
    sqb,
    with_move_file,
    write_orders_project,
)

_CENTS: str = "sql/models/staging/_sqlbuild/_macros/python/cents.py"
_COMPILE_STARTS: tuple[str, ...] = (
    "Compiling project...",
    "Verifying the edited project compiles...",
)
_BOTH_USERS: tuple[str, ...] = (
    "model:stg_order_cents",
    "sql/models/finance/",
    "stg_order_refunds",
    "sql/models/finance/",
)
_EXPECTED_MOVED: dict[str, str] = {
    "sql/models/staging/stg_order_cents.sql": "sql/models/finance/stg_order_cents.sql",
    "sql/models/staging/stg_order_refunds.sql": "sql/models/finance/stg_order_refunds.sql",
    _CENTS: "sql/models/finance/_sqlbuild/_macros/python/cents.py",
}


@pytest.mark.parametrize(
    "test_case",
    [
        BatchMoveE2ETestCase(
            description="pairs on the command line",
            arguments=_BOTH_USERS,
            move_file="",
            expected_moved=_EXPECTED_MOVED,
            expected_compiles=2,
        ),
        BatchMoveE2ETestCase(
            description="pairs from a file with comments and blank lines",
            arguments=("--from-file", MOVE_FILE_PLACEHOLDER),
            move_file=(
                "# move every cents user together\n\n"
                "model:stg_order_cents\tmodels/finance/\n"
                "  stg_order_refunds   sql/models/finance/  \n"
            ),
            expected_moved=_EXPECTED_MOVED,
            expected_compiles=2,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_moves_invalid_one_at_a_time_when_moved_together_then_one_verification_applies_all(
    tmp_path: Path, test_case: BatchMoveE2ETestCase
) -> None:
    """Every user of a local macro moves at once, so the macro moves once to their new folder."""

    project_dir: Path = write_orders_project(
        tmp_path=tmp_path, files=shared_cents_with_grouped_global_role()
    )
    move_file: Path = tmp_path / "moves.tsv"
    _ = move_file.write_text(test_case.move_file, encoding="utf-8")

    moved: subprocess.CompletedProcess[str] = sqb(
        project_dir, "mv", *with_move_file(test_case.arguments, move_file), "--json"
    )
    payload: dict[str, Any] = json.loads(moved.stdout)
    built: subprocess.CompletedProcess[str] = sqb(project_dir, "build")

    assert moved.returncode == 0, moved.stdout + moved.stderr
    assert payload["status"] == "applied"
    assert sum(moved.stderr.count(start) for start in _COMPILE_STARTS) == (
        test_case.expected_compiles
    ), moved.stderr
    assert existing_files(
        project_dir=project_dir,
        paths=(*test_case.expected_moved, *test_case.expected_moved.values()),
    ) == tuple(sorted(test_case.expected_moved.values())), moved.stdout
    assert built.returncode == 0, built.stdout + built.stderr
    assert relation_type(project_dir=project_dir, name="stg_order_refunds") is not None


@pytest.mark.parametrize(
    "test_case",
    [
        SingleMoveOfBatchE2ETestCase(
            description="moving one user alone needs a group in the top-level role",
            move=("stg_order_cents", "sql/models/finance/"),
            expected_reason=(
                "macro:to_cents must move to sql/macros/python/, which keeps its files in group"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_one_move_of_the_batch_alone_when_moving_then_it_is_refused(
    tmp_path: Path, test_case: SingleMoveOfBatchE2ETestCase
) -> None:
    """The batch above is needed: its moves are not valid one at a time."""

    project_dir: Path = write_orders_project(
        tmp_path=tmp_path, files=shared_cents_with_grouped_global_role()
    )
    before: dict[str, str] = project_text(project_dir)

    moved: subprocess.CompletedProcess[str] = sqb(project_dir, "mv", *test_case.move)

    assert moved.returncode == 1, moved.stdout + moved.stderr
    assert test_case.expected_reason in " ".join(moved.stdout.split())
    assert project_text(project_dir) == before


@pytest.mark.parametrize(
    "test_case",
    [
        BatchMoveConflictE2ETestCase(
            files=shared_cents_with_grouped_global_role(),
            description="two models moved to the same file",
            arguments=(
                "stg_order_cents",
                "sql/models/finance/order_amounts.sql",
                "stg_order_refunds",
                "sql/models/finance/order_amounts.sql",
            ),
            expected_reason=(
                "model:stg_order_cents and model:stg_order_refunds both move to "
                "sql/models/finance/order_amounts.sql"
            ),
        ),
        BatchMoveConflictE2ETestCase(
            files=shared_cents_with_grouped_global_role(),
            description="one model moved twice",
            arguments=(
                "stg_order_cents",
                "sql/models/finance/",
                "stg_order_cents",
                "sql/models/marts/",
            ),
            expected_reason="model:stg_order_cents is moved more than once",
        ),
        BatchMoveConflictE2ETestCase(
            files=shared_cents_with_grouped_global_role(),
            description="a model taking the name another model gives up",
            arguments=(
                "stg_order_refunds",
                "sql/models/staging/stg_order_returns.sql",
                "stg_order_cents",
                "sql/models/staging/stg_order_refunds.sql",
            ),
            expected_reason=(
                "model:stg_order_cents would take the name model:stg_order_refunds gives up"
            ),
        ),
        BatchMoveConflictE2ETestCase(
            description="two moved models' same-named local macro files sharing one target",
            files=same_named_local_macros(),
            arguments=(
                "stg_order_cents",
                "sql/models/finance/",
                "order_labels",
                "sql/models/finance/",
            ),
            expected_reason=(
                "sql/models/marts/_sqlbuild/_macros/python/helpers.py and "
                "sql/models/staging/_sqlbuild/_macros/python/helpers.py would both move to "
                "sql/models/finance/_sqlbuild/_macros/python/helpers.py; rename one of them first"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_conflicting_moves_when_moved_together_then_batch_is_refused_and_nothing_changes(
    tmp_path: Path, test_case: BatchMoveConflictE2ETestCase
) -> None:
    project_dir: Path = write_orders_project(tmp_path=tmp_path, files=test_case.files)
    before: dict[str, str] = project_text(project_dir)

    moved: subprocess.CompletedProcess[str] = sqb(project_dir, "mv", *test_case.arguments)

    assert moved.returncode == 1, moved.stdout + moved.stderr
    assert test_case.expected_reason in " ".join(moved.stdout.split()), moved.stdout
    assert "Refused; no files changed" in moved.stdout
    assert project_text(project_dir) == before


@pytest.mark.parametrize(
    "test_case",
    [
        BatchMoveDryRunE2ETestCase(
            description="both cents users and the macro they share",
            arguments=_BOTH_USERS,
            expected_lines=(
                "Move 2 models",
                "├── stg_order_cents  ->  sql/models/finance/stg_order_cents.sql",
                "└── stg_order_refunds  ->  sql/models/finance/stg_order_refunds.sql",
                f"├── sql/models/finance/_sqlbuild/_macros/python/cents.py  (moved from {_CENTS})",
                "Dry run: compiles, 3 files would change; nothing written",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_batch_dry_run_when_moving_then_combined_plan_is_printed_and_nothing_written(
    tmp_path: Path, test_case: BatchMoveDryRunE2ETestCase
) -> None:
    project_dir: Path = write_orders_project(
        tmp_path=tmp_path, files=shared_cents_with_grouped_global_role()
    )
    before: dict[str, str] = project_text(project_dir)

    moved: subprocess.CompletedProcess[str] = sqb(
        project_dir, "mv", *test_case.arguments, "--dry-run"
    )

    assert moved.returncode == 0, moved.stdout + moved.stderr
    assert set(test_case.expected_lines) <= set(moved.stdout.splitlines()), moved.stdout
    assert project_text(project_dir) == before
