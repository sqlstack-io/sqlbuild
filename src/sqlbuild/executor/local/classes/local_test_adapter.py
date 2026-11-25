"""A run-scoped in-memory DuckDB that runs a project's unit test SQL, converted by SQLQuack."""

from __future__ import annotations

import threading
from collections.abc import Mapping
from typing import Any

from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.local._helpers.conversion import converted
from sqlbuild.executor.local.constants import (
    CANNOT_RUN_LOCALLY_CODE,
    CONVERTED_DIALECT,
    UTC_TIME_ZONE,
)
from sqlbuild.executor.local.models import LocalConversion

_UTC_SESSION_SQL: str = f"SET TimeZone = '{UTC_TIME_ZONE}'"
_INVALIDATED_MARKER: str = "database has been invalidated"
_FATAL_ERRORS: frozenset[str] = frozenset({"FatalException", "InternalException"})


class LocalTestAdapter(DuckDbAdapter):
    """Renders test SQL as the project's adapter does and runs it in DuckDB after conversion."""

    def __init__(
        self,
        *,
        project_adapter: BaseAdapter,
        schema: Mapping[str, Any] | None,
        time_zone: str | None,
    ) -> None:
        super().__init__()
        self._project_adapter: BaseAdapter = project_adapter
        self._dialect: str | None = project_adapter.sql_analysis_dialect()
        self._schema: Mapping[str, Any] | None = schema
        self._time_zone: str | None = time_zone
        self._macros_lock: threading.Lock = threading.Lock()
        self._macros: dict[int, set[str]] = {}

    def sql_analysis_dialect(self) -> str | None:
        return self._dialect

    def render_set_difference_operator(self) -> str:
        return self._project_adapter.render_set_difference_operator()

    def create_sql_test_output(self, *, connection: Any, sql: str, name: str) -> str | None:
        """Compare inline: each comparison converts as one statement."""

        del connection, sql, name
        return None

    def connect(self, config: dict[str, Any]) -> Any:
        connection: Any = super().connect(config)
        _ = super().execute(connection=connection, sql=_UTC_SESSION_SQL)
        return connection

    def execute(self, *, connection: Any, sql: str) -> Any:
        try:
            return self._converted_execute(connection=connection, sql=sql)
        except Exception as error:
            if _INVALIDATED_MARKER in str(error) or type(error).__name__ in _FATAL_ERRORS:
                self._reopen(connection)
            raise

    def _reopen(self, connection: Any) -> None:
        """Replace a run-scoped database DuckDB invalidated, so later tests get a working one."""

        import duckdb

        object.__setattr__(connection, "raw_connection", duckdb.connect(database=":memory:"))
        with self._macros_lock:
            self._macros.pop(id(connection), None)
        _ = super().execute(connection=connection, sql=_UTC_SESSION_SQL)

    def _converted_execute(self, *, connection: Any, sql: str) -> Any:
        if self._dialect != CONVERTED_DIALECT or sql == _UTC_SESSION_SQL:
            return super().execute(connection=connection, sql=sql)
        conversion: LocalConversion = converted(
            sql=sql, schema=self._schema, time_zone=self._time_zone, path="test.sql"
        )
        if not conversion.converted:
            raise ExecutorInputError(
                "SQLQuack refused SQL the local test run needed: "
                + "; ".join(
                    f"{refusal.get('code')}: {refusal.get('label')}"
                    for refusal in conversion.refusals
                ),
                code=CANNOT_RUN_LOCALLY_CODE,
                help="Run this test on the warehouse with --run-on warehouse.",
            )
        self._install_macros(connection=connection, macros=conversion.macros)
        return super().execute(connection=connection, sql=conversion.sql)

    def _install_macros(self, *, connection: Any, macros: tuple[str, ...]) -> None:
        """Run the helper macros converted SQL calls, once per connection."""

        with self._macros_lock:
            installed: set[str] = self._macros.setdefault(id(connection), set())
            missing: list[str] = [macro for macro in macros if macro not in installed]
            installed.update(missing)
        for macro in missing:
            _ = super().execute(connection=connection, sql=macro)
