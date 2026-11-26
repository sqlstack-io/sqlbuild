from __future__ import annotations

import json
from typing import Any

from sqlbuild._native import run_sqlload_project
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.executor.load_sessions.classes.load_session import LoadSession
from sqlbuild.executor.load_sessions.main.open_load_session import open_load_session


def load_errors(*, project: str, adapter: BaseAdapter, base_connection: Any) -> list[str | None]:
    """Load `project` into the DuckDB database of `base_connection`: each table's error."""

    def opener() -> LoadSession:
        return open_load_session(
            adapter=adapter, connection_config={}, base_connection=base_connection
        )

    tables: list[dict[str, Any]] = json.loads(
        run_sqlload_project(
            project=project,
            sessions={
                "warehouse": {
                    "adapter": "duckdb",
                    "database": None,
                    "opener": opener,
                    "description": "a DuckDB file",
                }
            },
            select=[],
        )
    )
    return [table["error"] for table in tables]
