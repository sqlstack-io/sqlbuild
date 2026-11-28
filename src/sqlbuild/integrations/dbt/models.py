"""dbt interop records."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class DbtMacroOverride:
    """The dbt macro a Python macro replaces, how it differs and what is left unchecked."""

    replaces: str
    differs: str
    unchecked: str
