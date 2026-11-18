"""Metadata-only row count estimates of named relations, read one relation at a time."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any

from sqlbuild.adapter.contract.models import RelationRowCountEstimate


def estimates_one_at_a_time(
    *,
    estimate: Callable[..., RelationRowCountEstimate],
    connection: Any,
    location: tuple[str | None, str],
    names: tuple[str, ...],
) -> tuple[RelationRowCountEstimate, ...]:
    """Each name's metadata-only `estimate` in `location`'s `(database, schema)`, in order."""

    database: str | None
    schema: str
    database, schema = location
    return tuple(
        estimate(
            connection=connection, database=database, schema=schema, name=name, metadata_only=True
        )
        for name in names
    )
