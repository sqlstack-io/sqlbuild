"""Expected adapter-facing exception types."""

from __future__ import annotations

from sqlbuild.sql_values.exceptions import SqlValueRenderingError


class AdapterUserError(ValueError):
    """Raised when adapter configuration or optional dependencies are invalid."""

    code: str = "A000"

    def __init__(self, message: str, *, code: str | None = None, help: str | None = None) -> None:
        super().__init__(message)
        self.message = message
        self.code = code if code is not None else self.code
        self.help = help


class ViewRenameRefusedError(AdapterUserError):
    """Raised before renaming a view with table DDL, which DuckDB 2 would otherwise accept."""


class SqlStatementExecutionError(RuntimeError):
    """Warehouse error attributed to the exact SQL statement that raised it."""

    def __init__(self, *, sql: str, error: Exception) -> None:
        super().__init__(str(error))
        self.failed_sql = sql
        self.original_error = error
        self.message = str(getattr(error, "message", str(error)))

    def __getattr__(self, name: str) -> object:
        return getattr(self.original_error, name)


class UnsupportedTypedSqlRenderingError(AdapterUserError, SqlValueRenderingError):
    """Raised when an adapter cannot represent a requested typed SQL value."""

    def __init__(self, *, adapter_name: str, rendering: str, reason: str | None = None) -> None:
        reason_text: str = f": {reason}" if reason is not None else ""
        super().__init__(
            message=(
                f"adapter '{adapter_name}' does not support typed SQL {rendering} rendering"
                f"{reason_text}"
            )
        )


class WarehouseAccessRefusedError(RuntimeError):
    """Raised when a plan preview, which reads only the synced copy, would reach the warehouse."""

    def __init__(self, access: str) -> None:
        super().__init__(f"plan previews read only the synced copy; refused warehouse access: {access}")
        self.access = access


class SyncedCopyGapError(WarehouseAccessRefusedError):
    """Raised for a read the synced copy does not hold, which a preview treats as unknown."""
