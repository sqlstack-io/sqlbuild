"""Subprocess coverage for formatting that keeps case- and line-sensitive SQL tokens."""

from __future__ import annotations

import subprocess
from pathlib import Path

import duckdb
import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.format._test_types import (
    TokenPreservationBuildTestCase,
    TokenPreservationFormatTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.format.helpers import run_sqb

_DUCKDB_PROJECT_TOML: str = (
    'name = "orders"\nadapter = "duckdb"\ndefault_target = "prod"\n\n'
    '[connections.local]\ndatabase = "warehouse.duckdb"\n\n'
    '[targets.prod]\nconnection = "local"\nschema = "prod"\n'
)


@pytest.mark.parametrize(
    "test_case",
    [
        TokenPreservationFormatTestCase(
            description="BigQuery keyword-named table and hash comment survive",
            project_toml='name = "inventory"\nadapter = "bigquery"\n',
            model_name="stock_levels",
            authored_sql=(
                'MODEL (description "Stock levels");\n\n'
                "select product_id, # on-hand units\n  quantity from inventory.view\n"
            ),
            expected_sql=(
                'MODEL (description "Stock levels");\n\n'
                "SELECT\n  product_id, # on-hand units\n  quantity\nFROM inventory.view\n"
            ),
        ),
        TokenPreservationFormatTestCase(
            description="DuckDB adjacent string literals keep the space after the comma",
            project_toml='name = "products"\nadapter = "duckdb"\n',
            model_name="product_regions",
            authored_sql=(
                'MODEL (description "Product regions");\n\n'
                "select sku as replace, 'north' 'east' as region from products\n"
            ),
            expected_sql=(
                'MODEL (description "Product regions");\n\n'
                "SELECT\n  sku AS replace,\n  'north' 'east' AS region\nFROM products\n"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_case_sensitive_tokens_when_formatting_then_cli_writes_them_exactly(
    test_case: TokenPreservationFormatTestCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(test_case.project_toml, encoding="utf-8")
    model: Path = tmp_path / "models" / f"{test_case.model_name}.sql"
    model.parent.mkdir()
    model.write_text(test_case.authored_sql, encoding="utf-8")

    formatted: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=tmp_path, arguments=("--no-color", "format")
    )
    checked: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=tmp_path, arguments=("--no-color", "format", "--check")
    )

    assert (formatted.returncode, checked.returncode) == (0, 0), formatted.stderr + checked.stderr
    assert model.read_text(encoding="utf-8") == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    [
        TokenPreservationBuildTestCase(
            description="split literal and keyword-named aliases build unchanged",
            authored_sql=(
                'MODEL (description "Order lines", materialized table);\n\n'
                "with line as (select 1 as pos, 'waffle' as sku)\n"
                "select 'north'\n'east' as region, line.pos as index, line.sku as replace "
                "from line\n"
            ),
            expected_sql=(
                'MODEL (description "Order lines", materialized table);\n\n'
                "WITH line AS (\n  SELECT\n    1 AS pos,\n    'waffle' AS sku\n)\n\n"
                "SELECT\n  'north'\n  'east' AS region,\n  line.pos AS index,\n"
                "  line.sku AS replace\n"
                "FROM line\n"
            ),
            expected_columns=["region", "index", "replace"],
            expected_rows=[("northeast", 1, "waffle")],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_case_and_line_sensitive_tokens_when_formatting_then_build_keeps_them(
    test_case: TokenPreservationBuildTestCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(_DUCKDB_PROJECT_TOML, encoding="utf-8")
    model: Path = tmp_path / "models" / "order_lines.sql"
    model.parent.mkdir()
    model.write_text(test_case.authored_sql, encoding="utf-8")

    formatted: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=tmp_path, arguments=("--no-color", "format")
    )
    built: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=tmp_path, arguments=("--no-color", "build")
    )

    assert (formatted.returncode, built.returncode) == (0, 0), formatted.stderr + built.stderr
    assert model.read_text(encoding="utf-8") == test_case.expected_sql
    with duckdb.connect(str(tmp_path / "warehouse.duckdb"), read_only=True) as connection:
        cursor: duckdb.DuckDBPyConnection = connection.execute("SELECT * FROM prod.order_lines")
        assert [column[0] for column in cursor.description] == test_case.expected_columns
        assert cursor.fetchall() == test_case.expected_rows
