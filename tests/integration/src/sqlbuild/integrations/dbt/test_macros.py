"""Python macros translated from dbt render Jinja that calls their sibling macros by name."""

from __future__ import annotations

from pathlib import Path

import duckdb
import pytest

from sqlbuild._native import run_cli
from tests.integration.src.sqlbuild.integrations.dbt._test_types import DbtMacroBuildTestCase

_MACRO_SOURCE: str = '''from sqlbuild.dbt import jinja as _jinja, override as _override


def cents(column: str) -> str:
    """Convert an amount to cents."""
    return f"{column} * 100"


@_override(replaces="payments.cents_or_default", differs="casts nothing")
def cents_or_default(column: str, fallback: int) -> str:
    """Convert an amount to cents, or the fallback when it is null."""
    return _jinja("COALESCE({{ cents(column) }}, {{ fallback }})", column=column, fallback=fallback)
'''


@pytest.mark.parametrize(
    "test_case",
    [
        DbtMacroBuildTestCase(
            description="a template calls a sibling macro and reads the macro's values",
            macro_source=_MACRO_SOURCE,
            model_sql=(
                'SELECT @py.cents_or_default("amount", -1) AS amount_cents\n'
                "FROM (SELECT 2 AS amount UNION ALL SELECT NULL AS amount)\n"
            ),
            expected_rows=((-1,), (200,)),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dbt_jinja_macro_when_building_model_then_sibling_macro_renders_in_sql(
    test_case: DbtMacroBuildTestCase, tmp_path: Path
) -> None:
    files: dict[str, str] = {
        "sqlbuild_project.toml": (
            'name = "payments"\nadapter = "duckdb"\n[connection]\ndatabase = "payments.duckdb"\n'
        ),
        "sql/models/_macros/python/payments.py": test_case.macro_source,
        "sql/models/payments.sql": (
            f"MODEL (description 'Payments.', materialized table);\n{test_case.model_sql}"
        ),
    }
    for path, text in files.items():
        (tmp_path / path).parent.mkdir(parents=True, exist_ok=True)
        _ = (tmp_path / path).write_text(text)

    assert run_cli(["--project-dir", str(tmp_path), "build"]) == 0
    connection: duckdb.DuckDBPyConnection = duckdb.connect(
        str(tmp_path / "payments.duckdb"), read_only=True
    )
    try:
        rows: list[tuple[object, ...]] = connection.execute(
            "SELECT amount_cents FROM main.payments ORDER BY amount_cents"
        ).fetchall()
    finally:
        connection.close()
    assert tuple(rows) == test_case.expected_rows
