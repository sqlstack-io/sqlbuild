"""CLI e2e coverage for `sqb rename model:` and `sqb mv` on a real DuckDB project."""

from __future__ import annotations

import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.refactor._test_types import (
    DryRunE2ETestCase,
    ModelMigrationE2ETestCase,
    ModelRefactorE2ETestCase,
    PendingMigrationRefactorE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.refactor.helpers import (
    CUSTOMER_DECLARATIONS,
    ORDER_SHAPE,
    fragments_present,
    load_raw_orders,
    order_history_copy,
    order_history_files,
    order_ids,
    project_text,
    relation_type,
    remove_files,
    sqb,
    sqb_json,
    strip_migration_fingerprints,
    write_orders_project,
)

_TAGGED_SOURCES: str = (
    "# archived feeds\n"
    'sources: [{name: archived_orders, description: "Orders # archived", '
    'expression: !!str "(SELECT 1 AS id)"}, '
    "{name: archived_refunds, description: 'Refunds # archived', "
    "expression: !!str '(SELECT 2 AS id)'}]  # flow form\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        ModelRefactorE2ETestCase(
            description="rename keeps the folder and rewrites refs and fixtures",
            command=("rename", "model:stg_orders", "stg_order_lines"),
            expected_removed="sql/models/staging/stg_orders.sql",
            expected_file="sql/models/staging/stg_order_lines.sql",
            expected_fragments={
                "sql/models/marts/fact_orders.sql": ('__ref("stg_order_lines")',),
                "tests/unit/test_fact_orders.sql": ("__ref__stg_order_lines AS (",),
            },
            expected_relation="stg_order_lines",
        ),
        ModelRefactorE2ETestCase(
            description="rename rewrites seed and source relationships audits",
            command=("rename", "model:stg_orders", "stg_order_lines"),
            expected_removed="sql/models/staging/stg_orders.sql",
            expected_file="sql/models/staging/stg_order_lines.sql",
            expected_fragments={
                "sql/seeds/customers.yml": ('to: __ref("stg_order_lines")',),
                "sql/sources/customers.yml": ("to: '__ref(\"stg_order_lines\")'",),
                "sql/models/staging/stg_order_lines.sql": ("migrate_from stg_orders",),
            },
            expected_relation="stg_order_lines",
            extra_files=CUSTOMER_DECLARATIONS,
        ),
        ModelRefactorE2ETestCase(
            description="rename keeps tagged sources after quoted values holding ' # '",
            command=("rename", "model:stg_orders", "stg_order_lines"),
            expected_removed="sql/models/staging/stg_orders.sql",
            expected_file="sql/models/staging/stg_order_lines.sql",
            expected_fragments={"sql/sources/archived.yml": (_TAGGED_SOURCES,)},
            expected_relation="stg_order_lines",
            extra_files={"sql/sources/archived.yml": _TAGGED_SOURCES},
        ),
        ModelRefactorE2ETestCase(
            description="rename rewrites reusable schema relationships audits",
            command=("rename", "model:stg_orders", "stg_order_lines"),
            expected_removed="sql/models/staging/stg_orders.sql",
            expected_file="sql/models/staging/stg_order_lines.sql",
            expected_fragments={
                "sql/models/marts/_sqlbuild/_schemas/order_shape.sql": (
                    '(to __ref("stg_order_lines"), field customer_id)',
                ),
            },
            expected_relation="stg_order_lines",
            extra_files=ORDER_SHAPE,
        ),
        ModelRefactorE2ETestCase(
            description="rename rewrites cursor_inputs keys of incremental consumers",
            command=("rename", "model:stg_orders", "stg_order_lines"),
            expected_removed="sql/models/staging/stg_orders.sql",
            expected_file="sql/models/staging/stg_order_lines.sql",
            expected_fragments={
                "sql/models/marts/order_history.sql": (
                    "stg_order_lines order_date,",
                    '__ref("stg_order_lines")',
                ),
            },
            expected_relation="order_history",
            extra_files=order_history_files(materialized="incremental", udf=False),
        ),
        ModelRefactorE2ETestCase(
            description="move to a new file renames the model after the file",
            command=("mv", "model:fact_orders", "sql/models/reporting/order_facts.sql"),
            expected_removed="sql/models/marts/fact_orders.sql",
            expected_file="sql/models/reporting/order_facts.sql",
            expected_fragments={
                "sql/models/marts/customer_totals.sql": ('__ref("order_facts")',),
                "tests/unit/test_order_facts.sql": ("__expected__order_facts AS (",),
            },
            expected_relation="order_facts",
        ),
        ModelRefactorE2ETestCase(
            description="move into a folder keeps the model name",
            command=("mv", "model:customer_totals", "sql/models/reporting/"),
            expected_removed="sql/models/marts/customer_totals.sql",
            expected_file="sql/models/reporting/customer_totals.sql",
            expected_fragments={},
            expected_relation="customer_totals",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_model_refactor_when_applied_then_project_tests_and_builds(
    tmp_path: Path, test_case: ModelRefactorE2ETestCase
) -> None:
    """Every reference and fixture follows the model, and the edited project really runs."""

    project_dir: Path = write_orders_project(tmp_path=tmp_path, files=test_case.extra_files)

    result: subprocess.CompletedProcess[str] = sqb(project_dir, *test_case.command)
    tested: subprocess.CompletedProcess[str] = sqb(project_dir, "test")
    built: subprocess.CompletedProcess[str] = sqb(project_dir, "build")

    assert result.returncode == 0, result.stdout + result.stderr
    assert "Compiled: ok" in result.stdout
    assert not (project_dir / test_case.expected_removed).exists()
    assert (project_dir / test_case.expected_file).is_file()
    files: dict[str, str] = project_text(project_dir)
    assert fragments_present(files=files, expected=test_case.expected_fragments), files
    assert tested.returncode == 0, tested.stdout + tested.stderr
    assert built.returncode == 0, built.stdout + built.stderr
    assert relation_type(project_dir=project_dir, name=test_case.expected_relation) is not None


@pytest.mark.parametrize(
    "test_case",
    [
        DryRunE2ETestCase(
            description="model rename",
            command=("rename", "model:stg_orders", "stg_order_lines", "--dry-run"),
            expected_paths=frozenset(
                {
                    "sql/models/staging/stg_order_lines.sql",
                    "sql/models/marts/fact_orders.sql",
                    "tests/unit/test_fact_orders.sql",
                }
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_dry_run_json_when_refactoring_then_nothing_is_written(
    tmp_path: Path, test_case: DryRunE2ETestCase
) -> None:
    """A verified dry run reports its edits as JSON on stdout and progress on stderr."""

    project_dir: Path = write_orders_project(tmp_path=tmp_path)
    before: dict[str, str] = project_text(project_dir)

    result: subprocess.CompletedProcess[str] = sqb(project_dir, *test_case.command, "--json")
    code: int
    payload: dict[str, Any]
    code, payload = sqb_json(project_dir, *test_case.command)

    assert result.returncode == 0, result.stderr
    assert "Compiling project" in result.stderr
    assert code == 0
    assert payload["status"] == "dry_run"
    assert payload["files_changed"] == 0
    assert payload["compile"]["ok"] is True
    assert {item["path"] for item in payload["files"]} == test_case.expected_paths
    assert project_text(project_dir) == before


@pytest.mark.parametrize(
    "test_case",
    [
        ModelMigrationE2ETestCase(
            description="table",
            materialized="table",
            udf=False,
            expected_declaration="migrate_from order_history",
            expected_old_name_type="VIEW",
            expected_order_ids=(3, 4),
        ),
        ModelMigrationE2ETestCase(
            description="view calling a UDF",
            materialized="view",
            udf=True,
            expected_declaration="migrate_from order_history",
            expected_old_name_type="VIEW",
            expected_order_ids=(3, 4),
        ),
        ModelMigrationE2ETestCase(
            description="incremental with an unchanged definition",
            materialized="incremental",
            udf=False,
            expected_declaration="migrate_from order_history",
            expected_old_name_type="VIEW",
            expected_order_ids=(1, 2, 3, 4),
        ),
        ModelMigrationE2ETestCase(
            description="incremental that discovery would find ambiguous",
            materialized="incremental",
            udf=False,
            expected_declaration="migrate_from order_history",
            expected_old_name_type="VIEW",
            expected_order_ids=(1, 2, 3, 4),
            extra_files=order_history_copy(),
            removed_files=("sql/models/marts/order_history_copy.sql",),
        ),
        ModelMigrationE2ETestCase(
            description="incremental built before rename matching",
            materialized="incremental",
            udf=False,
            expected_declaration="migrate_from order_history",
            expected_old_name_type="VIEW",
            expected_order_ids=(1, 2, 3, 4),
            stripped_fingerprints=("order_history",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_built_model_when_renamed_then_build_migrates_its_relation(
    tmp_path: Path, test_case: ModelMigrationE2ETestCase
) -> None:
    """Every rename declares migrate_from, so history survives whatever discovery would do."""

    project_dir: Path = write_orders_project(
        tmp_path=tmp_path,
        files={
            **order_history_files(materialized=test_case.materialized, udf=test_case.udf),
            **test_case.extra_files,
        },
    )
    first: subprocess.CompletedProcess[str] = sqb(project_dir, "build")
    strip_migration_fingerprints(project_dir=project_dir, models=test_case.stripped_fingerprints)

    result: subprocess.CompletedProcess[str] = sqb(
        project_dir, "rename", "model:order_history", "order_ledger"
    )
    remove_files(project_dir=project_dir, paths=test_case.removed_files)
    header: str = (project_dir / "sql/models/marts/order_ledger.sql").read_text(encoding="utf-8")
    load_raw_orders(project_dir=project_dir, order_ids=(3, 4))
    second: subprocess.CompletedProcess[str] = sqb(project_dir, "build")

    assert first.returncode == 0, first.stdout + first.stderr
    assert result.returncode == 0, result.stdout + result.stderr
    assert header.count(test_case.expected_declaration) == 1, header
    assert second.returncode == 0, second.stdout + second.stderr
    assert relation_type(project_dir=project_dir, name="order_history") == (
        test_case.expected_old_name_type
    )
    assert order_ids(project_dir=project_dir, name="order_ledger") == test_case.expected_order_ids


@pytest.mark.parametrize(
    "test_case",
    [
        PendingMigrationRefactorE2ETestCase(
            description="move to another folder under the same name carries migrate_from",
            command=("mv", "model:order_ledger", "sql/models/reporting/"),
            expected_returncode=0,
            expected_file="sql/models/reporting/order_ledger.sql",
            expected_output="Compiled: ok",
        ),
        PendingMigrationRefactorE2ETestCase(
            description="rename on top of an undeployed migration is refused",
            command=("rename", "model:order_ledger", "order_journal"),
            expected_returncode=1,
            expected_file="sql/models/marts/order_ledger.sql",
            expected_output="still declares migrate_from",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_model_with_pending_migration_when_refactored_again_then_only_renames_are_refused(
    tmp_path: Path, test_case: PendingMigrationRefactorE2ETestCase
) -> None:
    """A same-name folder move keeps the relation, so the undeployed migrate_from moves with it."""

    project_dir: Path = write_orders_project(
        tmp_path=tmp_path, files=order_history_files(materialized="table", udf=False)
    )
    first: subprocess.CompletedProcess[str] = sqb(project_dir, "build")
    renamed: subprocess.CompletedProcess[str] = sqb(
        project_dir, "rename", "model:order_history", "order_ledger"
    )

    result: subprocess.CompletedProcess[str] = sqb(project_dir, *test_case.command)
    header: str = (project_dir / test_case.expected_file).read_text(encoding="utf-8")
    load_raw_orders(project_dir=project_dir, order_ids=(3, 4))
    second: subprocess.CompletedProcess[str] = sqb(project_dir, "build")

    assert first.returncode == 0, first.stdout + first.stderr
    assert renamed.returncode == 0, renamed.stdout + renamed.stderr
    assert result.returncode == test_case.expected_returncode, result.stdout + result.stderr
    assert test_case.expected_output in result.stdout + result.stderr
    assert header.count("migrate_from order_history") == 1, header
    assert second.returncode == 0, second.stdout + second.stderr
    assert relation_type(project_dir=project_dir, name="order_history") == "VIEW"
    assert order_ids(project_dir=project_dir, name="order_ledger") == (3, 4)
