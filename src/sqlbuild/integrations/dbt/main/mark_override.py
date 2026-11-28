"""Record which dbt macro a Python macro replaces, without changing the macro."""

from __future__ import annotations

from collections.abc import Callable
from functools import partial
from typing import TypeVar

from sqlbuild.integrations.dbt.models import DbtMacroOverride

_OVERRIDE_ATTRIBUTE: str = "__sqlbuild_dbt_override__"
_Function = TypeVar("_Function", bound=Callable[..., object])


def override(
    replaces: str, differs: str = "", unchecked: str = ""
) -> Callable[[_Function], _Function]:
    """Record on the decorated macro the dbt macro it replaces, and return the macro unchanged."""

    record: DbtMacroOverride = DbtMacroOverride(
        replaces=replaces, differs=differs, unchecked=unchecked
    )
    return partial(_record_override, record=record)


def _record_override(function: _Function, *, record: DbtMacroOverride) -> _Function:
    setattr(function, _OVERRIDE_ATTRIBUTE, record)
    return function
