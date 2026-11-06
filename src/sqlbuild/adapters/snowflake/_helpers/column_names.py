"""SQLBuild names for the column names Snowflake stores."""

from __future__ import annotations


def logical_column_name(stored: str) -> str:
    """Lower case for an unquoted name Snowflake stores in upper case, else the quoted name."""

    if stored == stored.upper():
        return stored.lower()
    return '"' + stored.replace('"', '""') + '"'
