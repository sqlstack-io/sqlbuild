"""Local scenario SQL: SQLQuack's DuckDB conversion of captured-dialect SQL, and relation names."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.local._helpers.conversion import converted
from sqlbuild.executor.local._helpers.positions import file_placed_refusal
from sqlbuild.executor.local.constants import CONVERTED_DIALECT, LOCAL_DIALECT
from sqlbuild.executor.scenario.constants import SCENARIO_LOCAL_SQL_TRANSPILE_FAILED


class LocalSqlRefusedError(ExecutorInputError):
    """SQLQuack refused SQL a local scenario run needs; `refusals` are its JSON refusals."""

    def __init__(self, message: str, *, refusals: tuple[Mapping[str, Any], ...]) -> None:
        super().__init__(
            message,
            code=SCENARIO_LOCAL_SQL_TRANSPILE_FAILED,
            help="Run this scenario on the warehouse with --run-on warehouse or --run-on auto.",
        )
        self.refusals: tuple[Mapping[str, Any], ...] = refusals


def convert_sql_for_local_duckdb(
    *,
    sql: str,
    source_dialect: str | None,
    schema: Mapping[str, Any] | None,
    time_zone: str | None,
    resource: tuple[str, str, str],
    macros: list[str],
    file: tuple[str, str] | None = None,
) -> str:
    """DuckDB SQL for `sql`; SQLQuack converts Snowflake SQL and DuckDB SQL runs as authored.

    `file` is the path and text of the file that holds `sql`, where refusals are placed.
    """

    scenario_name, resource_kind, resource_name = resource
    if source_dialect == LOCAL_DIALECT:
        return sql
    if source_dialect != CONVERTED_DIALECT:
        raise LocalSqlRefusedError(
            f"Scenario '{scenario_name}' cannot run locally: SQLQuack converts Snowflake SQL "
            f"only, not {source_dialect}",
            refusals=(
                {
                    "category": "dialect",
                    "construct": str(source_dialect),
                    "reason": f"SQLQuack converts Snowflake SQL only, not {source_dialect}",
                },
            ),
        )
    conversion: Any = converted(
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
                for refusal in conversion.report["refusals"]
            ),
        )
    for macro in conversion.macros:
        if macro not in macros:
            macros.append(macro)
    return conversion.sql


def replace_local_relations(*, sql: str, relation_replacements: dict[str, str]) -> str:
    """Replace planned scenario relation names with local DuckDB relation names."""

    result: str = sql
    original: str
    for original in sorted(
        relation_replacements.keys(), key=lambda value: len(value), reverse=True
    ):
        result = result.replace(original, relation_replacements[original])
    return result
