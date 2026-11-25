"""Write generated rows into a Snowflake source table from a self-managed loader.

SQLBuild's row-returning loaders render JSON values as `PARSE_JSON(...)` inside a `VALUES` list,
and Snowflake only accepts constants there. These loaders therefore create the table themselves and
insert each batch with `INSERT ... SELECT PARSE_JSON(columnN) ... FROM VALUES (...)`, where the
function call sits in the select list. Once the framework renders JSON that way, the loaders can
return their rows directly instead.
"""

from __future__ import annotations

import json
from datetime import date, datetime, time
from decimal import Decimal
from functools import lru_cache
from pathlib import Path

import yaml

SOURCES_DIR = Path(__file__).resolve().parents[3] / "sql" / "sources"
BATCH_SIZE = 500
JSON_TYPES = {"VARIANT": "", "OBJECT": "::OBJECT", "ARRAY": "::ARRAY"}


@lru_cache(maxsize=1)
def source_columns() -> dict[str, tuple[tuple[str, str], ...]]:
    """Declared (name, type) columns of every managed source, in declaration order."""

    columns: dict[str, tuple[tuple[str, str], ...]] = {}
    for path in sorted(SOURCES_DIR.glob("*.yml")):
        for source in yaml.safe_load(path.read_text())["sources"]:
            if source.get("managed"):
                columns[source["name"]] = tuple((c["name"], c["type"]) for c in source["columns"])
    return columns


def quote(text: str) -> str:
    """A Snowflake string literal; backslashes are escapes there, so they are doubled too."""

    return "'" + text.replace("\\", "\\\\").replace("'", "''") + "'"


def literal(value: object) -> str:
    if value is None:
        return "NULL"
    if isinstance(value, bool):
        return "TRUE" if value else "FALSE"
    if isinstance(value, int | Decimal):
        return str(value)
    if isinstance(value, float):
        return repr(value)
    if isinstance(value, datetime | date | time):
        return quote(value.isoformat())
    if isinstance(value, dict | list):
        return quote(json.dumps(value, sort_keys=True))
    return quote(str(value))


def create_sql(destination: str, columns: tuple[tuple[str, str], ...]) -> str:
    body = ", ".join(f"{name} {sql_type}" for name, sql_type in columns)
    return f"CREATE OR REPLACE TABLE {destination} ({body})"


def insert_sql(
    destination: str, columns: tuple[tuple[str, str], ...], rows: list[dict[str, object]]
) -> str:
    """One INSERT for a batch: typed casts and JSON parsing happen in the select list."""

    names = ", ".join(name for name, _ in columns)
    projections = []
    for index, (_, sql_type) in enumerate(columns, start=1):
        json_cast = JSON_TYPES.get(sql_type.upper())
        if json_cast is not None:
            projections.append(f"PARSE_JSON(column{index}){json_cast}")
        else:
            projections.append(f"column{index}::{sql_type}")
    values = ",\n".join(
        "(" + ", ".join(literal(row[name]) for name, _ in columns) + ")" for row in rows
    )
    select = ", ".join(projections)
    return f"INSERT INTO {destination} ({names})\nSELECT {select}\nFROM VALUES\n{values}"


def statements(destination: str, source_name: str, rows: list[dict[str, object]]) -> list[str]:
    """Every statement that replaces one source table with the given rows."""

    columns = source_columns()[source_name]
    sql = [create_sql(destination, columns)]
    for start in range(0, len(rows), BATCH_SIZE):
        sql.append(insert_sql(destination, columns, rows[start : start + BATCH_SIZE]))
    return sql
