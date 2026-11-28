"""dbt interop records."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class DbtMacroOverride:
    """The dbt macro a Python macro replaces, why, how it differs and what is left unchecked."""

    replaces: str
    reason: str
    differs: str
    unchecked: str
