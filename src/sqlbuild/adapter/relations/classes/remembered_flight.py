"""One remembered catalog computation in progress, which callers of the same key wait for."""

from __future__ import annotations

import threading


class RememberedFlight:
    """A computation's value or failure, shared with callers that asked while it ran."""

    def __init__(self) -> None:
        self._done: threading.Event = threading.Event()
        self._value: object = None
        self._error: BaseException | None = None

    def succeed(self, value: object) -> None:
        """Share ``value`` with every waiting caller."""

        self._value = value
        self._done.set()

    def fail(self, error: BaseException) -> None:
        """Raise ``error`` in every waiting caller."""

        self._error = error
        self._done.set()

    def wait(self) -> object:
        """The computation's value once it finishes, raising its failure."""

        _ = self._done.wait()
        if self._error is not None:
            raise self._error
        return self._value
