"""Cross-domain expected input errors."""

from __future__ import annotations


class SharedInputError(ValueError):
    """Raised when a public cross-domain input contract is invalid."""

    code: str = "G000"

    def __init__(self, message: str, *, code: str | None = None, help: str | None = None) -> None:
        super().__init__(message)
        self.message = message
        self.code = code if code is not None else self.code
        self.help = help


class ExecutorInputError(ValueError):
    """Raised when runtime execution inputs are invalid."""

    code: str = "X000"

    def __init__(self, message: str, *, code: str | None = None, help: str | None = None) -> None:
        super().__init__(message)
        self.message = message
        self.code = code if code is not None else self.code
        self.help = help


class LocalRunRefusedError(ExecutorInputError):
    """Raised when SQL running in the local DuckDB fails in a way the warehouse would not."""

    code: str = "T007"

    def __init__(
        self, message: str, *, category: str, construct: str, help: str | None = None
    ) -> None:
        super().__init__(message, help=help)
        self.category: str = category
        self.construct: str = construct
