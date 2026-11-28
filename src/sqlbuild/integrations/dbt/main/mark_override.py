"""Record which dbt macro a Python macro replaces, without changing the macro."""

from __future__ import annotations

from collections.abc import Callable
from functools import partial

from sqlbuild.integrations.dbt.models import DbtMacroOverride

_OVERRIDE_ATTRIBUTE: str = "__sqlbuild_dbt_override__"


def override[Function: Callable[..., object]](
    replaces: str, reason: str = "", differs: str = "", unchecked: str = ""
) -> Callable[[Function], Function]:
    """Record on the decorated macro the dbt macro it replaces, and return the macro unchanged."""

    record: DbtMacroOverride = DbtMacroOverride(
        replaces=replaces, reason=reason, differs=differs, unchecked=unchecked
    )
    return partial(_record_override, record=record)


def _record_override[Function: Callable[..., object]](
    function: Function, *, record: DbtMacroOverride
) -> Function:
    setattr(function, _OVERRIDE_ATTRIBUTE, record)
    return function
