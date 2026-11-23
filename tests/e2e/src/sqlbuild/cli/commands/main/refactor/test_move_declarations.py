"""CLI e2e coverage for `sqb mv` carrying scoped declarations to where placement requires."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.refactor._test_types import (
    DeclarationMoveE2ETestCase,
    DeclarationMoveRefusalE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.refactor.helpers import (
    CENTS_MACRO,
    ORDER_REFUNDS,
    ORDER_SHAPE,
    existing_files,
    grouped_role_files,
    project_text,
    relation_type,
    sqb,
    write_orders_project,
)

_CENTS: str = "sql/models/staging/_sqlbuild/_macros/python/cents.py"
_GROUPED_CENTS: str = "sql/models/staging/_sqlbuild/_macros/python/money/cents.py"
_PRIORITY_ENUM: dict[str, str] = {
    "sql/models/marts/_sqlbuild/_enums/priority.sql": (
        "ENUM (\n  name priority,\n  members (LOW 1, HIGH 3),\n);\n"
    ),
    "sql/models/marts/fact_orders.sql": ORDER_SHAPE["sql/models/marts/fact_orders.sql"].replace(
        "SELECT", 'SELECT\n  @enum("priority").LOW AS priority,', 1
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        DeclarationMoveE2ETestCase(
            description="declarations only the moved model uses move with it",
            model="fact_orders",
            extra_files={**ORDER_SHAPE, **_PRIORITY_ENUM},
            destination="sql/models/reporting/",
            expected_moved={
                "sql/models/marts/_sqlbuild/_schemas/order_shape.sql": (
                    "sql/models/reporting/_sqlbuild/_schemas/order_shape.sql"
                ),
                "sql/models/marts/_sqlbuild/_enums/priority.sql": (
                    "sql/models/reporting/_sqlbuild/_enums/priority.sql"
                ),
            },
        ),
        DeclarationMoveE2ETestCase(
            description="a macro still used at the old folder moves up to the shared owner",
            model="stg_order_cents",
            extra_files={**CENTS_MACRO, **ORDER_REFUNDS},
            destination="sql/models/staging/eu/",
            expected_moved={_CENTS: "sql/models/staging/_sqlbuild/macros/python/cents.py"},
        ),
        DeclarationMoveE2ETestCase(
            description="a macro shared across the models root moves to the top-level role",
            model="stg_order_cents",
            extra_files={**CENTS_MACRO, **ORDER_REFUNDS},
            destination="sql/models/marts/",
            expected_moved={_CENTS: "sql/macros/python/cents.py"},
        ),
        DeclarationMoveE2ETestCase(
            description="a grouped macro keeps its group in a grouped destination role",
            model="stg_order_cents",
            extra_files=grouped_role_files(
                cents_path=_GROUPED_CENTS,
                labels_path="sql/models/staging/_sqlbuild/macros/python/text/labels.py",
            ),
            destination="sql/models/staging/eu/",
            expected_moved={
                _GROUPED_CENTS: "sql/models/staging/_sqlbuild/macros/python/money/cents.py"
            },
        ),
        DeclarationMoveE2ETestCase(
            description="a grouped macro drops its group in a direct destination role",
            model="stg_order_cents",
            extra_files=grouped_role_files(
                cents_path=_GROUPED_CENTS,
                labels_path="sql/models/staging/_sqlbuild/macros/python/labels.py",
            ),
            destination="sql/models/staging/eu/",
            expected_moved={_GROUPED_CENTS: "sql/models/staging/_sqlbuild/macros/python/cents.py"},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_scoped_declarations_when_moving_model_then_they_follow_placement_rules(
    tmp_path: Path, test_case: DeclarationMoveE2ETestCase
) -> None:
    project_dir: Path = write_orders_project(tmp_path=tmp_path, files=test_case.extra_files)
    model: str = test_case.model

    moved: subprocess.CompletedProcess[str] = sqb(
        project_dir, "mv", f"model:{model}", test_case.destination
    )
    compiled: subprocess.CompletedProcess[str] = sqb(project_dir, "compile")
    built: subprocess.CompletedProcess[str] = sqb(project_dir, "build")

    assert moved.returncode == 0, moved.stdout + moved.stderr
    assert existing_files(
        project_dir=project_dir,
        paths=(*test_case.expected_moved, *test_case.expected_moved.values()),
    ) == tuple(sorted(test_case.expected_moved.values())), moved.stdout
    assert compiled.returncode == 0, compiled.stdout + compiled.stderr
    assert "error[S0" not in compiled.stdout + compiled.stderr
    assert built.returncode == 0, built.stdout + built.stderr
    assert relation_type(project_dir=project_dir, name=model) is not None


@pytest.mark.parametrize(
    "test_case",
    [
        DeclarationMoveRefusalE2ETestCase(
            description="a direct macro cannot join a grouped destination role",
            model="stg_order_cents",
            extra_files=grouped_role_files(
                cents_path=_CENTS,
                labels_path="sql/models/staging/_sqlbuild/macros/python/text/labels.py",
            ),
            destination="sql/models/staging/eu/",
            expected_message=(
                "macro:to_cents must move to sql/models/staging/_sqlbuild/macros/python/, which keeps its "
                f"files in group folders, but {_CENTS} is not in a group; move it into a group "
                "folder of its current role first"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_direct_declaration_and_grouped_destination_role_when_moving_then_move_is_refused(
    tmp_path: Path, test_case: DeclarationMoveRefusalE2ETestCase
) -> None:
    project_dir: Path = write_orders_project(tmp_path=tmp_path, files=test_case.extra_files)
    before: dict[str, str] = project_text(project_dir)

    moved: subprocess.CompletedProcess[str] = sqb(
        project_dir, "mv", f"model:{test_case.model}", test_case.destination
    )

    assert moved.returncode == 1, moved.stdout + moved.stderr
    assert test_case.expected_message in " ".join((moved.stdout + moved.stderr).split())
    assert project_text(project_dir) == before
