"""A file-backed DuckDB with a staging schema, so a heartbeat can open its own connection."""

from __future__ import annotations

from collections.abc import Iterator
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.executor.dynamic_pauses.helpers import (
    PausingDuckDbAdapter,
)


@pytest.fixture
def database_path(tmp_path: Path) -> Path:
    return tmp_path / "lease.duckdb"


@pytest.fixture
def adapter() -> PausingDuckDbAdapter:
    return PausingDuckDbAdapter()


@pytest.fixture
def connection(adapter: PausingDuckDbAdapter, database_path: Path) -> Iterator[Any]:
    opened: Any = adapter.connect({"database": str(database_path)})
    opened.execute("CREATE SCHEMA staging")
    yield opened
    adapter.close(opened)
