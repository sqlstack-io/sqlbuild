"""An empty in-memory DuckDB that finds the first relation a test's SQL reads and lacks."""

from __future__ import annotations

import re
from collections.abc import Sequence
from typing import Any

_MISSING_TABLE: re.Pattern[str] = re.compile(r"Table with name (\S+) does not exist")


class EmptyDuckDb:
    """An in-memory DuckDB with nothing in it, to bind test SQL before deciding where it runs."""

    def __init__(self) -> None:
        import duckdb

        self._connection: Any = duckdb.connect(database=":memory:")
        self._macros: set[str] = set()

    def missing_relation(self, *, sql: str, macros: Sequence[str]) -> str | None:
        """The first relation `sql` reads that an empty database lacks; `None` when it binds."""

        import duckdb

        for macro in macros:
            if macro in self._macros:
                continue
            self._macros.add(macro)
            try:
                _ = self._connection.execute(macro)
            except duckdb.Error:
                return None
        try:
            _ = self._connection.execute(f"EXPLAIN {sql}")
        except duckdb.CatalogException as error:
            match: re.Match[str] | None = _MISSING_TABLE.search(str(error))
            return None if match is None else match.group(1)
        except duckdb.Error:
            return None
        return None

    def close(self) -> None:
        self._connection.close()
