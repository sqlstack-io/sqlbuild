"""Helpers that take typed relation references."""

from sqlbuild.refs import SqlResourceRef


def union_by_columns(relations: list[SqlResourceRef], columns: list[str]) -> str:
    """UNION ALL of the same named columns from each relation, tagged with the relation name."""
    projection = ", ".join(columns)
    return "\nUNION ALL\n".join(
        f"SELECT CAST('{relation.name}' AS VARCHAR) AS source_relation, {projection} "
        f"FROM {relation}"
        for relation in relations
    )
