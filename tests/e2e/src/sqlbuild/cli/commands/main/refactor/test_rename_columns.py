"""CLI e2e coverage for `sqb rename column:` and for refactorings that must not write."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.refactor._test_types import (
    ColumnMigrationE2ETestCase,
    ColumnRenameE2ETestCase,
    CombinedRenameE2ETestCase,
    RefusedRefactorE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.refactor.helpers import (
    CENTS_MACRO,
    CUSTOMER_DECLARATIONS,
    FLAGGED_AUDITS,
    FLAGGED_CONTRACT,
    FLAGGED_LAMBDA_AUDIT,
    FLAGGED_LITERAL_AUDIT,
    FLAGGED_SUBQUERY_AUDITS,
    FLAGGED_USING_AUDIT,
    ORDER_AMOUNT_CHECK,
    ORDER_EXPORT,
    ORDER_SHAPE,
    ORDERS_MACRO,
    SPLIT_CENTS_MACROS,
    fragments_present,
    load_raw_orders,
    order_amount_union,
    order_history_files,
    order_ids,
    project_text,
    relation_columns,
    sqb,
    sqb_json,
    write_orders_project,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ColumnRenameE2ETestCase(
            description="one step keeps downstream output names",
            command=("rename", "column:stg_orders.amount", "revenue"),
            expected_fragments={
                "sql/models/staging/stg_orders.sql": ("  amount AS revenue,",),
                "sql/models/marts/fact_orders.sql": ("  o.revenue AS amount,",),
                "tests/unit/test_fact_orders.sql": ("25 AS revenue",),
            },
            expected_columns={
                "stg_orders": ("order_id", "customer_id", "revenue", "order_date"),
                "fact_orders": ("order_id", "customer_id", "amount", "order_date"),
            },
        ),
        ColumnRenameE2ETestCase(
            description="cascade renames pass-through outputs downstream",
            command=("rename", "column:stg_orders.amount", "revenue", "--cascade"),
            expected_fragments={
                "sql/models/marts/fact_orders.sql": ("  o.revenue,",),
                "sql/models/marts/customer_totals.sql": ("SUM(revenue) AS total_amount",),
            },
            expected_columns={
                "fact_orders": ("order_id", "customer_id", "revenue", "order_date"),
                "customer_totals": ("customer_id", "total_amount"),
            },
        ),
        ColumnRenameE2ETestCase(
            description="seed and source relationships follow the column",
            command=("rename", "column:stg_orders.customer_id", "buyer_id"),
            extra_files=CUSTOMER_DECLARATIONS,
            expected_fragments={
                "sql/seeds/customers.yml": ("field: buyer_id",),
                "sql/sources/customers.yml": ("field: buyer_id}",),
                "sql/models/marts/fact_orders.sql": ("  o.buyer_id AS customer_id,",),
            },
            expected_columns={
                "stg_orders": ("order_id", "buyer_id", "amount", "order_date"),
            },
        ),
        ColumnRenameE2ETestCase(
            description="reusable schema relationships follow the column",
            command=("rename", "column:stg_orders.customer_id", "buyer_id"),
            extra_files=ORDER_SHAPE,
            expected_fragments={
                "sql/models/marts/_sqlbuild/_schemas/order_shape.sql": ("field buyer_id)",),
            },
            expected_columns={
                "fact_orders": ("order_id", "customer_id", "amount", "order_date"),
            },
        ),
        ColumnRenameE2ETestCase(
            description="cascade stops at a later set-operation branch",
            command=("rename", "column:fact_orders.amount", "revenue", "--cascade"),
            extra_files=order_amount_union(first="stg_orders", second="fact_orders"),
            expected_fragments={
                "sql/models/marts/order_amounts.sql": ('SELECT revenue FROM __ref("fact_orders")',),
                "sql/models/marts/order_amount_reads.sql": ("SELECT amount FROM",),
            },
            expected_columns={
                "order_amounts": ("amount",),
                "order_amount_reads": ("amount",),
            },
        ),
        ColumnRenameE2ETestCase(
            description="cascade follows the first set-operation branch",
            command=("rename", "column:fact_orders.amount", "revenue", "--cascade"),
            extra_files=order_amount_union(first="fact_orders", second="stg_orders"),
            expected_fragments={
                "sql/models/marts/order_amounts.sql": (
                    'SELECT revenue FROM __ref("fact_orders")',
                    'SELECT amount FROM __ref("stg_orders")',
                ),
                "sql/models/marts/order_amount_reads.sql": ("SELECT revenue FROM",),
            },
            expected_columns={
                "order_amounts": ("revenue",),
                "order_amount_reads": ("revenue",),
            },
        ),
        ColumnRenameE2ETestCase(
            description="cascade follows SELECT star consumers",
            command=("rename", "column:fact_orders.amount", "revenue", "--cascade"),
            extra_files=ORDER_EXPORT,
            expected_fragments={"sql/models/marts/customer_totals.sql": ("SUM(revenue)",)},
            expected_columns={
                "fact_orders": ("order_id", "customer_id", "revenue", "order_date"),
                "order_export": ("order_id", "customer_id", "revenue", "order_date"),
            },
        ),
        ColumnRenameE2ETestCase(
            description="a column declared in the model's own contract renames in one step",
            command=("rename", "column:flagged_orders.is_flagged", "is_marked", "--cascade"),
            extra_files=FLAGGED_CONTRACT,
            expected_fragments={
                "sql/models/marts/flagged_orders.sql": (
                    "    is_marked (type BOOLEAN)\n",
                    "CAST(TRUE AS BOOLEAN) AS is_marked\n",
                ),
                "sql/models/marts/flagged_order_summary.sql": ("  is_marked\nFROM orders",),
            },
            expected_columns={
                "flagged_orders": ("order_id", "is_marked"),
                "flagged_order_summary": ("order_id", "is_marked"),
            },
        ),
        ColumnRenameE2ETestCase(
            description="audit where_condition arguments rename the column as SQL",
            command=("rename", "column:flagged_orders.is_flagged", "is_marked", "--cascade"),
            extra_files=FLAGGED_AUDITS,
            expected_fragments={
                "sql/models/marts/flagged_orders.sql": (
                    "where_condition \"is_marked = TRUE AND 'is_flagged' <> ''\"",
                    'where_condition "NOT is_marked"',
                ),
                "sql/models/marts/flagged_order_summary.sql": (
                    'where_condition "is_marked IS NULL"',
                ),
            },
            expected_columns={
                "flagged_orders": ("order_id", "is_marked"),
                "flagged_order_summary": ("order_id", "is_marked"),
            },
        ),
        ColumnRenameE2ETestCase(
            description="audit subqueries rename only columns of the audited model",
            command=("rename", "column:flagged_orders.is_flagged", "is_marked"),
            extra_files=FLAGGED_SUBQUERY_AUDITS,
            expected_fragments={
                "sql/models/marts/flagged_orders.sql": (
                    "'is_marked IN (SELECT is_flagged FROM __ref(\"vip_flags\"))'",
                    "'is_marked IN (SELECT is_marked FROM __ref(\"flagged_orders\"))'",
                ),
            },
            expected_columns={
                "flagged_orders": ("order_id", "is_marked"),
                "vip_flags": ("is_flagged",),
            },
        ),
        ColumnRenameE2ETestCase(
            description="audit lambda parameters named like the column keep their name",
            command=("rename", "column:flagged_orders.is_flagged", "is_marked"),
            extra_files=FLAGGED_LAMBDA_AUDIT,
            expected_fragments={
                "sql/models/marts/flagged_orders.sql": (
                    "'len(list_filter([is_marked], lambda is_flagged: is_flagged)) > 0'",
                ),
            },
            expected_columns={"flagged_orders": ("order_id", "is_marked")},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_column_rename_when_applied_then_project_tests_and_builds(
    tmp_path: Path, test_case: ColumnRenameE2ETestCase
) -> None:
    """Column references, fixtures, and outputs change together, and the project still runs."""

    project_dir: Path = write_orders_project(tmp_path=tmp_path, files=test_case.extra_files)

    code: int
    payload: dict[str, Any]
    code, payload = sqb_json(project_dir, *test_case.command)
    tested: subprocess.CompletedProcess[str] = sqb(project_dir, "test")
    built: subprocess.CompletedProcess[str] = sqb(project_dir, "build")

    assert code == 0, payload
    assert payload["status"] == "applied"
    assert tuple(item["path"] for item in payload["manual"]) == test_case.expected_manual
    files: dict[str, str] = project_text(project_dir)
    assert fragments_present(files=files, expected=test_case.expected_fragments), files
    assert tested.returncode == 0, tested.stdout + tested.stderr
    assert built.returncode == 0, built.stdout + built.stderr
    assert {
        name: relation_columns(project_dir=project_dir, name=name)
        for name in test_case.expected_columns
    } == test_case.expected_columns


@pytest.mark.parametrize(
    "test_case",
    [
        RefusedRefactorE2ETestCase(
            description="move needing to split a declaration file",
            command=("mv", "model:stg_order_cents", "sql/models/marts/"),
            extra_files=SPLIT_CENTS_MACROS,
            expected_status="refused",
            expected_reason="would take macro:to_dollars along, which must stay in",
            expected_paths=("sql/models/staging/_sqlbuild/_macros/python/cents.py",),
        ),
        RefusedRefactorE2ETestCase(
            description="column named in Python SQL text needs a manual check",
            command=("rename", "column:stg_orders.amount", "revenue"),
            extra_files=ORDER_AMOUNT_CHECK,
            expected_status="refused",
            expected_reason="Python SQL may read column amount of stg_orders",
            expected_paths=("python/nodes/checks/order_amounts/checks.py",),
        ),
        RefusedRefactorE2ETestCase(
            description="rename onto an existing model",
            command=("rename", "model:stg_orders", "fact_orders"),
            extra_files={},
            expected_status="refused",
            expected_reason="model:fact_orders already exists",
            expected_paths=("sql/models/marts/fact_orders.sql",),
        ),
        RefusedRefactorE2ETestCase(
            description="column used inside macro-generated SQL",
            command=("rename", "column:stg_orders.amount", "revenue"),
            extra_files=CENTS_MACRO,
            expected_status="refused",
            expected_reason="referenced in SQL a macro generates",
            expected_paths=("sql/models/staging/stg_order_cents.sql",),
        ),
        RefusedRefactorE2ETestCase(
            description="SELECT star consumer without cascade",
            command=("rename", "column:fact_orders.amount", "revenue"),
            extra_files=ORDER_EXPORT,
            expected_status="refused",
            expected_reason="rerun with --cascade",
            expected_paths=("sql/models/marts/order_export.sql",),
        ),
        RefusedRefactorE2ETestCase(
            description="model reference produced by a macro",
            command=("rename", "model:stg_orders", "stg_order_lines"),
            extra_files=ORDERS_MACRO,
            expected_status="refused",
            expected_reason="reference to stg_orders produced by a macro",
            expected_paths=("sql/models/staging/stg_order_count.sql",),
        ),
        RefusedRefactorE2ETestCase(
            description="audit argument the audit quotes as a string needs a manual check",
            command=("rename", "column:flagged_orders.is_flagged", "is_marked"),
            extra_files=FLAGGED_LITERAL_AUDIT,
            expected_status="refused",
            expected_reason="audit prefix_check quotes argument prefix as a string",
            expected_paths=("sql/models/marts/flagged_orders.sql",),
        ),
        RefusedRefactorE2ETestCase(
            description="audit argument joining on the column with USING needs a manual edit",
            command=("rename", "column:flagged_orders.is_flagged", "is_marked"),
            extra_files=FLAGGED_USING_AUDIT,
            expected_status="refused",
            expected_reason="USING or NATURAL join on is_flagged in this audit argument",
            expected_paths=("sql/models/marts/flagged_orders.sql",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unsafe_refactor_when_running_then_no_file_changes(
    tmp_path: Path, test_case: RefusedRefactorE2ETestCase
) -> None:
    """Refusals and failed verification exit 1, explain why, and leave every file untouched."""

    project_dir: Path = write_orders_project(tmp_path=tmp_path, files=test_case.extra_files)
    before: dict[str, str] = project_text(project_dir)

    code: int
    payload: dict[str, Any]
    code, payload = sqb_json(project_dir, *test_case.command)
    text: subprocess.CompletedProcess[str] = sqb(project_dir, *test_case.command)

    assert code == 1
    assert payload["status"] == test_case.expected_status
    assert test_case.expected_reason in json.dumps(payload)
    assert (
        tuple(sorted({item["path"] for item in (*payload["manual"], *payload["blocking"])}))
        == test_case.expected_paths
    )
    assert text.returncode == 1
    assert "no files changed" in text.stdout
    assert project_text(project_dir) == before


@pytest.mark.parametrize(
    "test_case",
    [
        ColumnMigrationE2ETestCase(
            description="incremental column renamed in place",
            expected_declaration="revenue (migrate_from amount)",
            expected_columns=("order_id", "revenue", "order_date"),
            expected_order_ids=(1, 2, 3, 4),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_built_incremental_when_renaming_column_then_build_keeps_history(
    tmp_path: Path, test_case: ColumnMigrationE2ETestCase
) -> None:
    """The rename declares the column migration, so the next build renames it in place."""

    project_dir: Path = write_orders_project(
        tmp_path=tmp_path, files=order_history_files(materialized="incremental", udf=False)
    )
    first: subprocess.CompletedProcess[str] = sqb(project_dir, "build")

    result: subprocess.CompletedProcess[str] = sqb(
        project_dir, "rename", "column:order_history.amount", "revenue"
    )
    header: str = (project_dir / "sql/models/marts/order_history.sql").read_text(encoding="utf-8")
    load_raw_orders(project_dir=project_dir, order_ids=(3, 4))
    second: subprocess.CompletedProcess[str] = sqb(project_dir, "build")

    assert first.returncode == 0, first.stdout + first.stderr
    assert result.returncode == 0, result.stdout + result.stderr
    assert test_case.expected_declaration in header, header
    assert second.returncode == 0, second.stdout + second.stderr
    assert relation_columns(project_dir=project_dir, name="order_history") == (
        test_case.expected_columns
    )
    assert order_ids(project_dir=project_dir, name="order_history") == (
        test_case.expected_order_ids
    )


@pytest.mark.parametrize(
    "test_case",
    [
        CombinedRenameE2ETestCase(
            description="table renamed, then its column, before one build",
            expected_declarations=("migrate_from order_history", "revenue (migrate_from amount)"),
            expected_new_columns=("order_id", "revenue", "order_date"),
            expected_old_columns=("order_id", "amount", "order_date"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_renamed_table_when_renaming_its_column_then_build_keeps_both(
    tmp_path: Path, test_case: CombinedRenameE2ETestCase
) -> None:
    """A renamed table declares migrate_from, so its column rename is declared too."""

    project_dir: Path = write_orders_project(
        tmp_path=tmp_path, files=order_history_files(materialized="table", udf=False)
    )
    first: subprocess.CompletedProcess[str] = sqb(project_dir, "build")

    model: subprocess.CompletedProcess[str] = sqb(
        project_dir, "rename", "model:order_history", "order_ledger"
    )
    column: subprocess.CompletedProcess[str] = sqb(
        project_dir, "rename", "column:order_ledger.amount", "revenue"
    )
    header: str = (project_dir / "sql/models/marts/order_ledger.sql").read_text(encoding="utf-8")
    second: subprocess.CompletedProcess[str] = sqb(project_dir, "build")

    assert first.returncode == 0, first.stdout + first.stderr
    assert model.returncode == 0, model.stdout + model.stderr
    assert column.returncode == 0, column.stdout + column.stderr
    assert all(declaration in header for declaration in test_case.expected_declarations), header
    assert second.returncode == 0, second.stdout + second.stderr
    assert relation_columns(project_dir=project_dir, name="order_ledger") == (
        test_case.expected_new_columns
    )
    assert relation_columns(project_dir=project_dir, name="order_history") == (
        test_case.expected_old_columns
    )
