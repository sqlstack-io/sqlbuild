"""`ctx.log`: the few lines a node logs, kept with its run's events."""

from __future__ import annotations

import logging

from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.types import LogLevel

_PYTHON_LEVELS: dict[LogLevel, int] = {
    LogLevel.INFO: logging.INFO,
    LogLevel.WARNING: logging.WARNING,
    LogLevel.ERROR: logging.ERROR,
}


class NodeLogger:
    """Log a line at info, warning or error; everything a node prints stays its output."""

    def __init__(self, *, recorder: StatementRecorder, logger: logging.Logger | None) -> None:
        self._recorder: StatementRecorder = recorder
        self._logger: logging.Logger | None = logger

    def info(self, message: str) -> None:
        """Log `message` at info."""

        self._record(message, LogLevel.INFO)

    def warning(self, message: str) -> None:
        """Log `message` at warning."""

        self._record(message, LogLevel.WARNING)

    def error(self, message: str) -> None:
        """Log `message` at error."""

        self._record(message, LogLevel.ERROR)

    def _record(self, message: str, level: LogLevel) -> None:
        self._recorder.log(str(message), level)
        if self._logger is not None:
            self._logger.log(_PYTHON_LEVELS[level], "%s", message)
