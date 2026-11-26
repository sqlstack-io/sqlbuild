"""Scenario execution exceptions."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.scenario.constants import SCENARIO_LOCAL_SQL_TRANSPILE_FAILED


class ExecutorJsonTypeError(TypeError):
    """Raised when executor JSON serialization receives an unsupported value."""

    code: str = "X000"

    def __init__(self, message: str, *, code: str | None = None, help: str | None = None) -> None:
        super().__init__(message)
        self.message = message
        self.code = code if code is not None else self.code
        self.help = help


class ScenarioStopRequested(BaseException):  # noqa: N818 - a control signal, not an error
    """Raised at a step boundary when an interrupt asked running scenarios to stop."""


class LocalSqlRefusedError(ExecutorInputError):
    """SQLQuack refused SQL a local scenario run needs; `refusals` are its JSON refusals."""

    def __init__(self, message: str, *, refusals: tuple[Mapping[str, Any], ...]) -> None:
        super().__init__(
            message,
            code=SCENARIO_LOCAL_SQL_TRANSPILE_FAILED,
            help="Run this scenario on the warehouse with --run-on warehouse or --run-on auto.",
        )
        self.refusals: tuple[Mapping[str, Any], ...] = refusals
