"""Map INFORMATION_SCHEMA.TABLES rows onto the relations SHOW TABLES and SHOW VIEWS list."""

from __future__ import annotations

from typing import Any

from sqlbuild.adapter.contract.models import RelationInfo
from sqlbuild.adapter.relations.main.relation_age_timestamp import relation_age_timestamp_utc
from sqlbuild.adapter.relations.models import ListedRelation, RelationChange

_RELATION_TYPES: dict[str, str] = {
    "BASE TABLE": "base table",
    "LOCAL TEMPORARY": "temporary table",
    "TEMPORARY TABLE": "temporary table",
    "EXTERNAL TABLE": "external table",
    "EVENT TABLE": "event table",
    "VIEW": "view",
    "MATERIALIZED VIEW": "materialized view",
}
_VIEW_TYPES: frozenset[str] = frozenset({"view", "materialized view"})
_YES: str = "YES"


def relation_change_from_row(*, row: tuple[Any, ...], database: str | None) -> RelationChange:
    """One relation as SHOW would list it, with its last data change and last DDL in UTC."""

    stored_name: str = str(row[0])
    table_type: str = str(row[2] or "").upper()
    relation_type: str = _RELATION_TYPES.get(table_type, table_type.lower())
    view: bool = relation_type in _VIEW_TYPES
    retention: str = str(row[7] if row[7] is not None else "").strip()
    return RelationChange(
        listed=ListedRelation(
            stored_name=stored_name,
            relation=RelationInfo(
                database=database,
                schema=str(row[1]).lower(),
                name=stored_name.lower(),
                relation_type=relation_type,
                is_transient=None if view else str(row[3] or "").upper() == _YES,
                created_at=relation_age_timestamp_utc(row[4]),
                last_altered_at=None,
                retention_days=None
                if view or not retention.lstrip("-").isdigit()
                else int(retention),
            ),
        ),
        last_altered=relation_age_timestamp_utc(row[5]),
        last_ddl=relation_age_timestamp_utc(row[6]),
    )
