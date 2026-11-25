"""Local scenario SQL: SQLQuack's DuckDB conversion of captured-dialect SQL, and relation names."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from sqlbuild.executor.local.constants import (
    CONVERTED_DIALECT,
    DIALECT_REASON_CATEGORY,
    LOCAL_DIALECT,
)
from sqlbuild.executor.local.main._convert_local_sql import (
    convert_local_sql,
    file_placed_refusal,
)
from sqlbuild.executor.local.models import LocalConversion
from sqlbuild.executor.scenario.exceptions import LocalSqlRefusedError
from sqlbuild.executor.scenario.models import LocalSql


def convert_sql_for_local_duckdb(
    *,
    sql: str,
    source_dialect: str | None,
    schema: Mapping[str, Any] | None,
    time_zone: str | None,
    resource: tuple[str, str, str],
    file: tuple[str, str] | None = None,
) -> LocalSql:
    """DuckDB SQL for `sql`, refusals placed in `file` (path, text): converted or as authored."""

    scenario_name, resource_kind, resource_name = resource
    if source_dialect == LOCAL_DIALECT:
        return LocalSql(sql=sql)
    if source_dialect != CONVERTED_DIALECT:
        raise LocalSqlRefusedError(
            f"Scenario '{scenario_name}' cannot run locally: SQLQuack converts Snowflake SQL "
            f"only, not {source_dialect}",
            refusals=(
                {
                    "category": DIALECT_REASON_CATEGORY,
                    "construct": str(source_dialect),
                    "reason": f"SQLQuack converts Snowflake SQL only, not {source_dialect}",
                },
            ),
        )
    conversion: LocalConversion = convert_local_sql(
        sql=sql,
        schema=schema,
        time_zone=time_zone,
        path=f"{resource_kind} {resource_name}",
    )
    if not conversion.converted:
        raise LocalSqlRefusedError(
            f"SQLQuack refused the SQL of scenario '{scenario_name}', {resource_kind} "
            f"'{resource_name}':\n{conversion.rendered.rstrip()}",
            refusals=tuple(
                file_placed_refusal(refusal=refusal, sql=sql, file=file)
                for refusal in conversion.refusals
            ),
        )
    return LocalSql(sql=conversion.sql, macros=conversion.macros)


def replace_local_relations(*, sql: str, relation_replacements: dict[str, str]) -> str:
    """Replace planned scenario relation names with local DuckDB relation names."""

    result: str = sql
    original: str
    for original in sorted(
        relation_replacements.keys(), key=lambda value: len(value), reverse=True
    ):
        result = result.replace(original, relation_replacements[original])
    return result
