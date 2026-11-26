"""The session a load's SQLLoad target writes through, on the build's warehouse connection."""

from __future__ import annotations

from typing import Any

from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.executor.load_sessions.classes.load_session import LoadSession
from sqlbuild.executor.load_sessions.main.open_load_session import open_load_session


def load_sessions(
    *,
    session: str,
    adapter: BaseAdapter,
    connection_config: dict[str, object],
    base_connection: Any,
    database: str | None,
) -> dict[str, dict[str, object]]:
    """`session` as `run_sqlload_project` takes it: each table SQLLoad loads opens its own."""

    def opener() -> LoadSession:
        return open_load_session(
            adapter=adapter, connection_config=connection_config, base_connection=base_connection
        )

    return {
        session: {
            "adapter": adapter.adapter_name,
            "database": database,
            "opener": opener,
            "description": f"the project's {adapter.adapter_name} warehouse",
        }
    }
