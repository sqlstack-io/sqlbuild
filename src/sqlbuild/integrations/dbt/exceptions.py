"""dbt interop exceptions."""

from __future__ import annotations

from sqlbuild._native import DbtInteropConfigError


class DbtProfileError(DbtInteropConfigError):
    """Raised when a dbt profile cannot be resolved for SQLBuild."""

    code: str = "C242"


class DbtMacroReturn(Exception):  # noqa: N818
    """dbt's `return(value)` inside a macro template: rendering stops and the macro returns it."""

    def __init__(self, value: object) -> None:
        super().__init__()
        self.value: object = value
