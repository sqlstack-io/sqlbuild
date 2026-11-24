"""The project's declared relations and column types, as SQLQuack's schema option takes them."""

from __future__ import annotations

from collections.abc import Iterable
from typing import Any


def project_schema(project: Any) -> dict[str, Any]:
    """Sources, seeds and models with typed columns, keyed by their Snowflake-resolved names."""

    relations: dict[str, list[list[str]]] = {}
    for source in getattr(project, "sources", ()):
        entry: Any = source.source_entry
        relations.update(
            _typed_relation(
                parts=(entry.database, entry.schema, entry.table or entry.name),
                columns=((column.name, getattr(column, "type", None)) for column in entry.columns),
            )
        )
    for seed in getattr(project, "seeds", ()):
        destination: Any = seed.destination
        relations.update(
            _typed_relation(
                parts=(destination.database, destination.schema, destination.name),
                columns=((column.name, column.type) for column in seed.schema_entry.columns),
            )
        )
    for model in getattr(project, "models", ()):
        schema_entry: Any = model.schema_entry
        if schema_entry is None:
            continue
        destination = model.destination
        relations.update(
            _typed_relation(
                parts=(destination.database, destination.schema, destination.name),
                columns=((column.name, column.type) for column in schema_entry.columns),
            )
        )
    return {"relations": relations}


def _typed_relation(
    *, parts: tuple[str | None, ...], columns: Iterable[tuple[str, str | None]]
) -> dict[str, list[list[str]]]:
    """One relation keyed by its resolved name when every column has a declared type, else none."""

    typed: list[list[str]] = []
    for name, column_type in columns:
        if not column_type:
            return {}
        typed.append([_resolved(name), column_type])
    if not typed:
        return {}
    return {".".join(_resolved(part) for part in parts if part): typed}


def _resolved(name: str) -> str:
    """An identifier as Snowflake resolves it: quoted keeps its case, unquoted is upper case."""

    if len(name) > 1 and name.startswith('"') and name.endswith('"'):
        return name[1:-1]
    return name.upper()
