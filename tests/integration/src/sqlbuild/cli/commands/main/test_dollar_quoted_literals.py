"""Integration coverage for dollar-quoted string literals through the real CLI."""

from __future__ import annotations

from pathlib import Path

import duckdb
import pytest

from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    DollarQuotedLiteralBuildTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import run_build


@pytest.mark.parametrize(
    "test_case",
    [
        DollarQuotedLiteralBuildTestCase(
            description="dollar-quoted literals stay opaque while real references build",
            projection=(
                '$$Customer\'s order -- __ref("missing_orders") costs $5$$ AS order_label,\n'
                '  $note$ $$ nested __seed("missing_seed") @py.missing_macro() $note$ AS order_note,\n'
                "  o.order_id"
            ),
            expected_rows=(
                (
                    'Customer\'s order -- __ref("missing_orders") costs $5',
                    ' $$ nested __seed("missing_seed") @py.missing_macro() ',
                    1,
                ),
            ),
        ),
        DollarQuotedLiteralBuildTestCase(
            description="tagged dollar-quoted literal with an apostrophe stays opaque",
            projection=(
                "$note$ customer's note $note$ AS order_label,\n"
                "  $$plain$$ AS order_note,\n"
                "  o.order_id"
            ),
            expected_rows=((" customer's note ", "plain", 1),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dollar_quoted_literal_when_building_then_literal_is_preserved_and_refs_resolve(
    test_case: DollarQuotedLiteralBuildTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n[connection]\ndatabase = "orders.duckdb"\n',
        encoding="utf-8",
    )
    models: Path = tmp_path / "sql" / "models"
    models.mkdir()
    (models / "orders.sql").write_text(
        'MODEL (description "Orders", materialized table);\nSELECT 1 AS order_id\n',
        encoding="utf-8",
    )
    (models / "order_labels.sql").write_text(
        'MODEL (description "Order labels", materialized table);\n'
        f"SELECT\n  {test_case.projection}\n"
        'FROM __ref("orders") AS o\n',
        encoding="utf-8",
    )

    exit_code, output = run_build(project_dir=tmp_path, flags=(), capsys=capsys)

    assert exit_code == 0, output
    with duckdb.connect(str(tmp_path / "orders.duckdb"), read_only=True) as connection:
        rows: list[tuple[object, ...]] = connection.execute(
            "SELECT order_label, order_note, order_id FROM main.order_labels"
        ).fetchall()
    assert tuple(rows) == test_case.expected_rows
