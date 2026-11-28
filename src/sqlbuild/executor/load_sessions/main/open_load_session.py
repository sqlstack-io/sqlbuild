"""Open the session SQLLoad loads one table on."""

from __future__ import annotations

from typing import Any

from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.observed_connection import ObservedConnection
from sqlbuild.executor.load_sessions.classes.load_session import LoadSession
from sqlbuild.executor.load_sessions.constants import DUCKDB_BACKED_ADAPTERS, SNOWFLAKE_ADAPTER


def open_load_session(
    *, adapter: BaseAdapter, connection_config: dict[str, object], base_connection: Any
) -> LoadSession:
    """A session of its own, in autocommit: a DuckDB cursor of `base_connection`, else a new one."""
    if adapter.adapter_name in DUCKDB_BACKED_ADAPTERS:
        raw_base: Any = getattr(base_connection, "raw_connection", base_connection)
        connection: Any = ObservedConnection(
            raw_connection=raw_base.cursor(), adapter=adapter.adapter_name
        )
        return LoadSession(adapter=adapter, connection=connection)
    connection = adapter.connect(dict(connection_config))
    if adapter.adapter_name == SNOWFLAKE_ADAPTER:
        getattr(connection, "raw_connection", connection).autocommit(True)
    return LoadSession(adapter=adapter, connection=connection)
