"""A thread that tells other builds, through the warehouse, that a build still holds its models."""

from __future__ import annotations

import threading
from collections.abc import Callable
from typing import Any

from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter


class MaintainedHeartbeat:
    """Write a heartbeat every interval on its own connection, which a busy build never blocks."""

    def __init__(
        self,
        *,
        adapter: BaseAdapter,
        connection_config: dict[str, object],
        write: Callable[..., None],
        interval_seconds: float,
    ) -> None:
        self._adapter: BaseAdapter = adapter
        self._connection_config: dict[str, object] = connection_config
        self._write: Callable[..., None] = write
        self._interval_seconds: float = interval_seconds
        self._stopped: threading.Event = threading.Event()
        self._thread: threading.Thread = threading.Thread(
            target=self._beat, name="sqlbuild-maintained-heartbeat", daemon=True
        )
        self.failure: Exception | None = None

    def start(self) -> None:
        self._thread.start()

    def stop(self) -> None:
        self._stopped.set()
        self._thread.join()

    def _beat(self) -> None:
        """Beat at once, then every interval, so no gap exceeds one interval plus a write."""

        connection: Any = None
        sequence: int = 0
        stopped: bool = False
        while not stopped:
            sequence += 1
            try:
                if connection is None:
                    connection = self._adapter.connect(dict(self._connection_config))
                self._write(connection=connection, sequence=sequence)
            except Exception as error:
                self.failure = error
                connection = self._closed(connection=connection)
            stopped = self._stopped.wait(self._interval_seconds)
        _ = self._closed(connection=connection)

    def _closed(self, *, connection: Any) -> None:
        if connection is not None:
            try:
                self._adapter.close(connection)
            except Exception as error:
                self.failure = error
        return None
