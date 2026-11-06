"""The key that matches a declared column name to a warehouse-listed one."""

from __future__ import annotations

from sqlbuild.adapter.contract.main.is_quoted_identifier import is_quoted_identifier


def column_name_key(name: str) -> str:
    """The lower-cased name without the quotes of a quoted identifier, as the compiler keys it."""

    return (name[1:-1] if is_quoted_identifier(name) else name).lower()
