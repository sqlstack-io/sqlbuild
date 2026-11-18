"""The warehouse each build connection runs on, switched only when a node needs another."""

from __future__ import annotations

import logging
import threading
from collections.abc import Mapping
from typing import Any

from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.errors.contracts.exceptions import ExecutorInputError

_CONNECTION_WAREHOUSE_KEY: str = "warehouse"
_UNSET_PREFIX: str = "<none"

_LOGGER: logging.Logger = logging.getLogger(__name__)


def configured_warehouse(connection_config: Mapping[str, object]) -> str | None:
    """The warehouse a connection opened with `connection_config` starts on, if it names one."""

    value: object | None = connection_config.get(_CONNECTION_WAREHOUSE_KEY)
    if not isinstance(value, str):
        return None
    stripped: str = value.strip()
    if not stripped or stripped.startswith(_UNSET_PREFIX):
        return None
    return stripped


def with_session_warehouse(
    connection_config: dict[str, object], warehouse: str | None
) -> dict[str, object]:
    """`connection_config` for code that opens its own connections on `warehouse`."""

    if warehouse is None:
        return connection_config
    return {**connection_config, _CONNECTION_WAREHOUSE_KEY: warehouse}


def _same_warehouse(first: str, second: str) -> bool:
    """Unquoted warehouse names match case-insensitively, as Snowflake resolves them."""

    if first.startswith('"') or second.startswith('"'):
        return first == second
    return first.upper() == second.upper()


class SessionWarehouses:
    """Moves a connection to a node's warehouse, and back to the build's, only when it changes."""

    def __init__(self, *, adapter: BaseAdapter, home: str | None) -> None:
        self._adapter: BaseAdapter = adapter
        self._home: str | None = home
        self._home_known: bool = home is not None
        self._lock: threading.Lock = threading.Lock()
        # Connections away from the build's warehouse: id -> (connection, warehouse). Holding the
        # connection keeps its id from being reused by another while it is listed.
        self._away: dict[int, tuple[Any, str]] = {}

    def use(self, *, connection: Any, warehouse: str | None) -> None:
        """Put `connection` on `warehouse`, or on the build's warehouse when it is None."""

        with self._lock:
            entry: tuple[Any, str] | None = self._away.get(id(connection))
        current: str | None = None if entry is None or entry[0] is not connection else entry[1]
        target: str | None = warehouse
        if target is not None and self._home is not None and _same_warehouse(target, self._home):
            target = None
        if current == target or (
            current is not None and target is not None and _same_warehouse(current, target)
        ):
            return
        if target is None:
            self._adapter.use_session_warehouse(connection=connection, warehouse=self._return_to())
            with self._lock:
                self._away.pop(id(connection), None)
            return
        if current is None:
            self._learn_home(connection)
            if self._home is not None and _same_warehouse(target, self._home):
                return
        self._adapter.use_session_warehouse(connection=connection, warehouse=target)
        with self._lock:
            self._away[id(connection)] = (connection, target)

    def restore(self) -> None:
        """Return every connection still on a node's warehouse to the build's warehouse."""

        with self._lock:
            away: list[tuple[Any, str]] = list(self._away.values())
            self._away.clear()
        if not away or self._home is None:
            return
        connection: Any
        for connection, _ in away:
            try:
                self._adapter.use_session_warehouse(connection=connection, warehouse=self._home)
            except Exception as error:  # noqa: BLE001 - its owner closes or replaces it next
                _LOGGER.warning(
                    "could not return a connection to warehouse %s: %s", self._home, error
                )

    def _learn_home(self, connection: Any) -> None:
        """Read the warehouse the build's connections start on before one first leaves it."""

        with self._lock:
            if self._home_known:
                return
            self._home = self._adapter.current_session_warehouse(connection=connection)
            self._home_known = True

    def _return_to(self) -> str:
        if self._home is None:
            raise ExecutorInputError(
                "cannot return a connection to the build's warehouse: the target sets no "
                "warehouse and the session had none; set the target's warehouse to build nodes "
                "outside the named jobs beside them"
            )
        return self._home
