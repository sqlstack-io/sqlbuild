"""Local scenario SQL transformation helpers."""

from __future__ import annotations

import json
from typing import NoReturn

from sqlbuild._native import convert_local_scenario_sql
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.scenario.constants import (
    LOCAL_SCENARIO_CONVERTED_DIALECT,
    LOCAL_SCENARIO_DUCKDB_DIALECTS,
    SCENARIO_LOCAL_SQL_TRANSPILE_FAILED,
)
from sqlbuild.executor.scenario.types import LocalSqlColumns


def convert_sql_for_local_duckdb(
    *,
    sql: str,
    source_dialect: str | None,
    schema: dict[str, LocalSqlColumns],
    scenario_name: str,
    resource_kind: str,
    resource_name: str,
) -> tuple[str, LocalSqlColumns]:
    """Return captured SQL as DuckDB SQL and its result columns; DuckDB SQL runs as authored.

    Snowflake SQL is converted by SQLQuack under its built-in default policy against `schema`, the
    Snowflake columns of each local relation keyed as Snowflake resolves its unquoted name. A
    statement SQLQuack refuses is refused here with SQLQuack's diagnostics.
    """

    if source_dialect in LOCAL_SCENARIO_DUCKDB_DIALECTS:
        return sql, ()
    if source_dialect != LOCAL_SCENARIO_CONVERTED_DIALECT:
        _raise_conversion_error(
            scenario_name=scenario_name,
            resource_kind=resource_kind,
            resource_name=resource_name,
            reason=f"local execution is unavailable for {source_dialect}",
        )
    try:
        converted_sql, columns, refusal = convert_local_scenario_sql(
            sql,
            json.dumps({name: [list(column) for column in columns] for name, columns in schema.items()}),
            f"{resource_kind} '{resource_name}'",
        )
    except ValueError as exc:
        _raise_conversion_error(
            scenario_name=scenario_name,
            resource_kind=resource_kind,
            resource_name=resource_name,
            reason=str(exc),
        )
    if converted_sql is None:
        _raise_conversion_error(
            scenario_name=scenario_name,
            resource_kind=resource_kind,
            resource_name=resource_name,
            reason=f"SQLQuack refuses it:\n{refusal}",
        )
    return converted_sql, tuple(columns)


def function_sql_for_local_duckdb(
    *,
    sql: str,
    source_dialect: str | None,
    scenario_name: str,
    resource_kind: str,
    resource_name: str,
) -> str:
    """Return a SQL function body for local DuckDB; only DuckDB captures run SQL functions locally."""

    if source_dialect not in LOCAL_SCENARIO_DUCKDB_DIALECTS:
        _raise_conversion_error(
            scenario_name=scenario_name,
            resource_kind=resource_kind,
            resource_name=resource_name,
            reason=f"local execution of {source_dialect} SQL functions is unavailable",
        )
    return sql


def _raise_conversion_error(
    *, scenario_name: str, resource_kind: str, resource_name: str, reason: str
) -> NoReturn:
    raise ExecutorInputError(
        f"Local SQL conversion failed for scenario '{scenario_name}', "
        f"{resource_kind} '{resource_name}': {reason}",
        code=SCENARIO_LOCAL_SQL_TRANSPILE_FAILED,
        help="Rewrite unsupported SQL or adjust the scenario so it can run in DuckDB locally.",
    )


def replace_local_relations(*, sql: str, relation_replacements: dict[str, str]) -> str:
    """Replace planned scenario relation names with local DuckDB relation names."""

    result: str = sql
    original: str
    original: str
    for original in sorted(
        relation_replacements.keys(), key=lambda value: len(value), reverse=True
    ):
        result = result.replace(original, relation_replacements[original])
    return result
