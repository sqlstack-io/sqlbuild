"""Record which dbt macro a Python macro replaces, without changing the macro."""

from __future__ import annotations

from collections.abc import Callable

from sqlbuild.integrations.dbt.models import DbtMacroOverride


def override[Function: Callable[..., object]](
    *, replaces: str, reason: str = "", differs: str = "", unchecked: str = ""
) -> Callable[[Function], Function]:
    """Record on the decorated macro the dbt macro it replaces, and return the macro unchanged."""

    record: DbtMacroOverride = DbtMacroOverride(
        replaces=replaces, reason=reason, differs=differs, unchecked=unchecked
    )

    def mark(function: Function) -> Function:
        _ = setattr(function, "__sqlbuild_dbt_override__", record)
        return function

    return mark
