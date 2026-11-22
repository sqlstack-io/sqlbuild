"""Execution pipeline errors."""

from __future__ import annotations

from sqlbuild.executor.pipeline.models import AuditExecutionFailure


class AuditExecutionError(RuntimeError):
    """Raised after every selected standalone audit has been attempted."""

    def __init__(self, *, failures: tuple[AuditExecutionFailure, ...]) -> None:
        self.failures: tuple[AuditExecutionFailure, ...] = failures
        lines: list[str] = [f"Standalone audit execution failed ({len(failures)} audits):"]
        lines.extend(
            f"  {failure.resource_id}: {failure.error_type}: {failure.message}"
            for failure in failures
        )
        super().__init__("\n".join(lines))


class AuditCoordinatorError(BaseException):
    """Marks callback failures that must stop standalone audit submission."""

    def __init__(self, error: BaseException) -> None:
        self.error: BaseException = error
        super().__init__(type(error).__name__)


class AuditConcurrencyError(RuntimeError):
    """Raised when standalone audit concurrency is invalid."""


class AuditOutcomeError(RuntimeError):
    """Raised when audit execution returns an unknown quality outcome."""


class WarehouseConnectionError(Exception):
    """Raised when a command cannot open its warehouse connection."""

    code: str = "C115"

    def __init__(self, *, adapter_name: str, reason: str) -> None:
        self.message: str = f"could not connect to the {adapter_name} warehouse: {reason}"
        self.help: str = (
            "Check the connection settings of the target; `sqb debug` tests the connection."
        )
        super().__init__(self.message)


class TemporaryTestFunctionError(Exception):
    """Raised when a test connection cannot create the temporary function copies tests call."""

    code: str = "T006"

    def __init__(self, *, function_name: str, connection: str, reason: str) -> None:
        self.message: str = (
            f"could not create the temporary copy of function {function_name} on {connection}: "
            f"{reason}"
        )
        self.help: str = (
            "Tests call temporary copies of the functions they use, never the built functions; "
            "check that the target's role may create temporary functions in the session's "
            "current schema, or in the function's schema when the session has none, and that the "
            "schema exists."
        )
        super().__init__(self.message)
