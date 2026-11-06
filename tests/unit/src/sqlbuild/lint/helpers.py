"""Project writers shared by lint behavior tests."""

import itertools
import json
from pathlib import Path
from typing import cast

import pytest

from sqlbuild._native import run_cli

_PROJECT_TOML: str = 'name = "demo"\nadapter = "duckdb"\n'


def write_fixture_format_project(
    *, tmp_path: Path, fixture_sql: str, project_toml: str = _PROJECT_TOML
) -> Path:
    """Write a contracted two-model project with one SQL fixture."""

    (tmp_path / "sqlbuild_project.toml").write_text(project_toml, encoding="utf-8")
    staging_model: Path = tmp_path / "models" / "stg_rows.sql"
    staging_model.parent.mkdir()
    staging_model.write_text(
        "MODEL (\n"
        '  description "Staged rows",\n'
        "  contract enforced,\n"
        "  columns (\n"
        "    id (type INTEGER, nullable false),\n"
        "    flag (type BOOLEAN, nullable true),\n"
        "  ),\n"
        ");\n\n"
        "SELECT 1 AS id, TRUE AS flag\n",
        encoding="utf-8",
    )
    (tmp_path / "models" / "rows.sql").write_text(
        'MODEL (description "Rows");\n\nSELECT id, flag FROM __ref("stg_rows")\n',
        encoding="utf-8",
    )
    test_file: Path = tmp_path / "tests" / "unit" / "test_rows.sql"
    test_file.parent.mkdir(parents=True)
    test_file.write_text(
        "TEST();\n\n"
        "WITH\n"
        f"__ref__stg_rows AS ({fixture_sql}),\n"
        "__expected__rows AS (SELECT 1 AS id, TRUE AS flag)\n"
        "SELECT 1\n",
        encoding="utf-8",
    )
    return test_file


_RESERVED_NAMES: dict[str, frozenset[str]] = {
    "duckdb": frozenset({"any", "only", "order", "pivot", "some"}),
    "postgres": frozenset({"any", "only", "order", "some"}),
    "bigquery": frozenset(
        {
            "any",
            "exclude",
            "ignore",
            "left",
            "nulls",
            "order",
            "preceding",
            "range",
            "recursive",
            "rows",
            "some",
            "tablesample",
            "within",
        }
    ),
    "snowflake": frozenset({"any", "order", "row", "rows", "some", "tablesample"}),
    "tsql": frozenset(
        {"any", "index", "key", "left", "order", "pivot", "some", "tablesample", "view"}
    ),
    "databricks": frozenset(),
}
_FOLDED_KEYWORDS: tuple[str, ...] = ("select", "from", "where", "union", "order")
_DIALECTS: tuple[str, ...] = ("duckdb", "postgres", "bigquery", "snowflake", "tsql", "databricks")
_KEYWORD_NAMED_IDENTIFIERS: tuple[str, ...] = (
    "Left",
    "Filter",
    "Rows",
    "Index",
    "View",
    "Replace",
    "Any",
    "Some",
    "Only",
    "Semi",
    "Anti",
    "Pivot",
    "Tablesample",
    "Range",
    "Row",
    "Nulls",
    "Ignore",
    "Within",
    "Recursive",
    "Exclude",
    "Preceding",
    "Key",
    "Type",
    "Date",
    "First",
    "Value",
    "Order",
)
_NAME_SHAPES: tuple[tuple[str, str], ...] = (
    ("alias", "select order_id as {name} from orders"),
    ("qualified column", "select o.{name} from orders o"),
    ("qualified table", "select * from inventory.{name}"),
)
_NON_RESERVED_SHAPES: tuple[tuple[str, str], ...] = (
    ("projection", "select {name} from orders"),
    ("table", "select * from {name}"),
    ("operand", "select order_id, {name} ^ 2 from orders"),
)
_DUCKDB_SHAPES: tuple[tuple[str, str], ...] = (("struct key", "select {{{name}: 1}} as s"),)


def reserved_keywords(dialect: str) -> frozenset[str]:
    """Return which of the keyword-named identifiers SQLClean reserves in one dialect."""

    return _RESERVED_NAMES[dialect]


def keyword_name_positions() -> tuple[tuple[str, str, str, str], ...]:
    """Return (dialect, name, position, template) for names that must keep their case."""

    positions: list[tuple[str, str, str, str]] = []
    for dialect, name in itertools.product(_DIALECTS, _KEYWORD_NAMED_IDENTIFIERS):
        unreserved_shapes: tuple[tuple[str, str], ...] = _NON_RESERVED_SHAPES + {
            "duckdb": _DUCKDB_SHAPES
        }.get(dialect, ())
        shapes: tuple[tuple[str, str], ...] = (
            _NAME_SHAPES
            + {
                False: unreserved_shapes,
                True: (),
            }[name.lower() in reserved_keywords(dialect)]
        )
        positions.extend((dialect, name, shape, template) for shape, template in shapes)
    return tuple(positions)


def reserved_keyword_pairs() -> tuple[tuple[str, str], ...]:
    """Return (dialect, keyword) pairs for core keywords every dialect but Databricks reserves."""

    reserving: tuple[str, ...] = tuple(filter(_RESERVED_NAMES.__getitem__, _DIALECTS))
    return tuple(itertools.product(reserving, _FOLDED_KEYWORDS))


def format_project_json(
    *, project_dir: Path, capsys: pytest.CaptureFixture[str], arguments: tuple[str, ...] = ()
) -> dict[str, object]:
    """Run `sqb format --json` on a project and return its parsed report."""

    _ = run_cli(["--project-dir", str(project_dir), "format", "--json", *arguments])
    report: dict[str, object] = json.loads(capsys.readouterr().out)
    return report


def violation_reports(report: dict[str, object]) -> list[dict[str, object]]:
    """Return the violation objects of a format report."""

    return cast(list[dict[str, object]], report["violations"])


def formatted_files(report: dict[str, object]) -> list[str]:
    """Return the changed file paths of a format report."""

    return cast(list[str], report["formatted_files"])
