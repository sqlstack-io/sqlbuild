"""Direct node result store factory."""

from __future__ import annotations

from typing import Any

from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.executor.node_results.classes.direct_store import DirectNodeResultStore


def build_direct_node_result_store(
    *,
    adapter: BaseAdapter,
    connection: Any,
    database: str | None,
    schema: str | None,
) -> DirectNodeResultStore:
    """Build a direct-mode node result store that also reads the adapter's default location."""

    previous: tuple[str | None, str | None] = (adapter.default_database(), adapter.default_schema())
    return DirectNodeResultStore(
        adapter=adapter,
        connection=connection,
        database=database,
        schema=schema,
        previous_location=previous if previous != (database, schema) else None,
    )
