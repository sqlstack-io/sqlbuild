"""Direct node result store factory."""

from __future__ import annotations

from contextlib import AbstractContextManager
from typing import Any

from sqlbuild._native import TargetSettings
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.executor.node_results.classes.direct_store import DirectNodeResultStore
from sqlbuild.executor.node_results.constants import PRESERVED_SCHEMA_NAME


def build_direct_node_result_store(
    *,
    adapter: BaseAdapter,
    connection: Any,
    database: str | None,
    schema: str | None,
    target_config: TargetSettings | None = None,
    write_lock: AbstractContextManager[Any] | None = None,
) -> DirectNodeResultStore:
    """Build a direct-mode node result store that, for one release, also reads older locations."""

    earlier: list[tuple[str | None, str | None]] = [
        (adapter.default_database(), adapter.default_schema())
    ]
    if target_config is not None and target_config.schema_preserved:
        earlier.append((database, PRESERVED_SCHEMA_NAME))
    return DirectNodeResultStore(
        adapter=adapter,
        connection=connection,
        database=database,
        schema=schema,
        write_lock=write_lock,
        previous_locations=tuple(
            location for location in earlier if location != (database, schema)
        ),
    )
