"""Runtime validation of the column names a model's configuration names."""

from __future__ import annotations

from collections.abc import Iterable

from sqlbuild.adapter.contract.main.column_name_key import column_name_key
from sqlbuild.adapter.contract.main.is_quoted_identifier import is_quoted_identifier
from sqlbuild.adapter.contract.models import ColumnInfo
from sqlbuild.errors.contracts.exceptions import ExecutorInputError


def require_quoted_spelling(
    *,
    model_name: str,
    setting: str,
    configured: Iterable[str],
    listed_columns: tuple[ColumnInfo, ...],
) -> None:
    """Reject an unquoted configured name that only a quoted, case-sensitive column matches."""

    spellings: dict[str, list[str]] = {}
    column: ColumnInfo
    for column in listed_columns:
        spellings.setdefault(column_name_key(column.name), []).append(column.name)
    name: str
    for name in configured:
        if is_quoted_identifier(name):
            continue
        matches: list[str] = spellings.get(column_name_key(name), [])
        if not matches or not all(is_quoted_identifier(match) for match in matches):
            continue
        raise ExecutorInputError(
            f"model '{model_name}' {setting} '{name}' names the case-sensitive column "
            f"{matches[0]}; write it with its quotes, as '{matches[0]}', so the generated SQL "
            "keeps its case"
        )
