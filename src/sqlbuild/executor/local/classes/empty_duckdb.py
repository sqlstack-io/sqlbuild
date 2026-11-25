"""An empty in-memory DuckDB that finds the first relation a test's SQL reads and lacks."""

from __future__ import annotations

import re
import threading
from collections.abc import Sequence
from typing import Any

_PROBE: str = "__sqb_local_probe"
_MISSING_TABLE: re.Pattern[str] = re.compile(r"Table with name (\S+) does not exist")


class EmptyDuckDb:
    """An in-memory DuckDB with nothing in it, to bind test SQL before deciding where it runs."""

    def __init__(self) -> None:
        import duckdb

        self._connection: Any = duckdb.connect(database=":memory:")
        self._macros: set[str] = set()
        self._lock: threading.Lock = threading.Lock()

    def missing_relation(self, *, sql: str, macros: Sequence[str]) -> str | None:
        """The first relation `sql` reads that an empty database lacks; `None` when it binds."""

        with self._lock:
            return self._missing_relation(sql=sql, macros=macros)

    def _missing_relation(self, *, sql: str, macros: Sequence[str]) -> str | None:
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
            _ = self._connection.execute(f"PREPARE {_PROBE} AS {sql}")
            _ = self._connection.execute(f"DEALLOCATE {_PROBE}")
        except duckdb.CatalogException as error:
            match: re.Match[str] | None = _MISSING_TABLE.search(str(error))
            return None if match is None else match.group(1)
        except duckdb.Error:
            return None
        return None

    def close(self) -> None:
        self._connection.close()
