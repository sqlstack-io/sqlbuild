"""MotherDuck adapter implementation."""

from __future__ import annotations

import json
from datetime import date, datetime
from decimal import Decimal
from typing import Any, ClassVar
from urllib.parse import urlencode

from sqlbuild.adapter.contract.classes.consumption import ConsumptionMixin
from sqlbuild.adapter.contract.classes.duckdb_backed_adapter import DuckDbBackedAdapter
from sqlbuild.adapter.contract.classes.microbatch import MicrobatchMixin
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.contract.types import BuiltinAdapter, LoaderLogicalType
from sqlbuild.adapters.motherduck.constants import (
    EMPTY_CONNECTION_VALUE,
    MOTHERDUCK_UNSUPPORTED_DUCKDB_HELP,
    MOTHERDUCK_UNSUPPORTED_DUCKDB_MARKER,
    URI_ADDITIONAL_PARAMETER_SEPARATOR,
    URI_QUERY_SEPARATOR,
)


class MotherDuckAdapter(MicrobatchMixin, ConsumptionMixin, DuckDbBackedAdapter):
    """MotherDuck adapter backed by DuckDB's MotherDuck connection support."""

    adapter_name: ClassVar[str] = BuiltinAdapter.MOTHERDUCK.value
    sql_analysis_dialect_name: ClassVar[str | None] = "duckdb"

    def render_loader_logical_type(self, type_name: LoaderLogicalType) -> str:
        match type_name:
            case LoaderLogicalType.BOOLEAN:
                return "BOOLEAN"
            case LoaderLogicalType.INTEGER:
                return "BIGINT"
            case LoaderLogicalType.FLOAT:
                return "DOUBLE"
            case LoaderLogicalType.STRING:
                return "VARCHAR"
            case LoaderLogicalType.TIMESTAMP:
                return "TIMESTAMP"
            case LoaderLogicalType.DATE:
                return "DATE"
            case LoaderLogicalType.JSON:
                return "JSON"

    def render_loader_value_literal(
        self, *, value: object, logical_type: LoaderLogicalType | None
    ) -> str:
        if value is None:
            return "NULL"
        if isinstance(value, bool):
            return "TRUE" if value else "FALSE"
        if isinstance(value, int | float | Decimal):
            return str(value)
        if isinstance(value, datetime | date):
            return self._quote_sql_string(value.isoformat())
        if isinstance(value, dict | list):
            return self._quote_sql_string(json.dumps(value, sort_keys=True))
        return self._quote_sql_string(str(value))

    def render_loader_rows_select(
        self,
        *,
        rows: tuple[dict[str, object], ...],
        column_names: tuple[str, ...],
        column_sql_types: dict[str, str],
        inferred_types: dict[str, LoaderLogicalType],
    ) -> str:
        if not rows:
            projections: str = ", ".join(
                "CAST(NULL AS "
                f"{column_sql_types.get(column_name, 'VARCHAR')}) AS "
                f"{self.render_identifier(column_name)}"
                for column_name in column_names
            )
            return f"SELECT {projections} WHERE 1 = 0"
        value_rows: list[str] = []
        for row in rows:
            row_values: list[str] = []
            for column_name in column_names:
                row_values.append(
                    self.render_loader_value_literal(
                        value=row.get(column_name),
                        logical_type=inferred_types.get(column_name),
                    )
                )
            value_rows.append(f"({', '.join(row_values)})")
        values_sql: str = ", ".join(value_rows)
        column_sql: str = ", ".join(
            self.render_identifier(column_name) for column_name in column_names
        )
        select_sql: str = ", ".join(
            (
                self.render_identifier(column_name)
                if column_name not in column_sql_types
                else "CAST("
                f"{self.render_identifier(column_name)} AS {column_sql_types[column_name]}) "
                f"AS {self.render_identifier(column_name)}"
            )
            for column_name in column_names
        )
        return f"SELECT {select_sql} FROM (VALUES {values_sql}) AS __loader_rows({column_sql})"

    def _quote_sql_string(self, value: str) -> str:
        return "'" + value.replace("'", "''") + "'"

    def render_source_expression_cast(
        self, *, expression: str, target_type: str, alias: str
    ) -> str:
        return f"CAST({expression} AS {target_type}) AS {alias}"

    def render_source_expression_relation(self, *, expression: str) -> str:
        stripped_expression: str = expression.strip().removesuffix(";").strip()
        if stripped_expression.startswith("("):
            return stripped_expression
        lowered: str = stripped_expression.lower()
        if lowered.startswith(("select", "with", "values")):
            return f"({stripped_expression})"
        return stripped_expression

    def render_source_freshness_max_query(
        self, *, column: str, source_relation: str, source_is_subquery: bool, where_sql: str
    ) -> str:
        del source_is_subquery
        return (
            f"SELECT MAX({self.render_identifier(column)}) AS data_version "
            f"FROM {source_relation}{where_sql}"
        )

    def render_source_expression_cast_subquery(
        self, *, source_relation: str, projections: tuple[str, ...]
    ) -> str:
        projection_clause: str = ", ".join(projections)
        return f"(SELECT {projection_clause} FROM {source_relation} AS __source_expression)"

    def render_source_relation_cast_subquery(
        self,
        *,
        source_relation: str,
        cast_projections: tuple[str, ...],
        cast_column_names: tuple[str, ...],
        all_columns_cast: bool,
    ) -> str:
        cast_clause: str = ", ".join(cast_projections)
        if all_columns_cast:
            return f"(SELECT {cast_clause} FROM {source_relation})"
        exclude_list: str = ", ".join(self.render_identifier(name) for name in cast_column_names)
        return f"(SELECT * EXCLUDE ({exclude_list}), {cast_clause} FROM {source_relation})"

    def connect(self, config: dict[str, Any]) -> Any:
        """Open a MotherDuck connection using DuckDB's md: connection string."""

        duckdb_config: dict[str, Any] = dict(config)
        duckdb_config["database"] = self._connection_database(config)

        import duckdb

        database: str = str(duckdb_config.get("database", ":memory:"))
        try:
            connection: duckdb.DuckDBPyConnection = duckdb.connect(database=database)
        except duckdb.Error as error:
            if MOTHERDUCK_UNSUPPORTED_DUCKDB_MARKER not in str(error):
                raise
            raise AdapterUserError(
                message=f"MotherDuck does not support DuckDB {duckdb.__version__} yet.",
                help=MOTHERDUCK_UNSUPPORTED_DUCKDB_HELP,
            ) from error

        extensions: list[str] | tuple[str, ...] = duckdb_config.get("extensions", ())
        extension_name: str
        for extension_name in extensions:
            self.execute(connection=connection, sql=f"INSTALL '{extension_name}'")
            self.execute(connection=connection, sql=f"LOAD '{extension_name}'")

        settings: dict[str, object] = duckdb_config.get("settings", {})
        setting_key: str
        setting_value: object
        for setting_key, setting_value in settings.items():
            self.execute(connection=connection, sql=f"SET {setting_key} = '{setting_value}'")

        attach_entries: list[dict[str, object]] = duckdb_config.get("attach", [])
        attach_entry: dict[str, object]
        for attach_entry in attach_entries:
            self.execute(connection=connection, sql=self.duckdb_build_attach_sql(attach_entry))

        return connection

    def default_schema(self) -> str:
        """MotherDuck uses DuckDB's main schema by default."""

        return "main"

    def _connection_database(self, config: dict[str, Any]) -> str:
        raw_database: object | None = config.get("database")
        database: str = "md:" if raw_database is None else str(raw_database)
        if database == EMPTY_CONNECTION_VALUE:
            database = "md:"
        elif not database.startswith("md:"):
            database = f"md:{database}"

        token: object | None = config.get("token")
        if token is None or str(token) == EMPTY_CONNECTION_VALUE:
            return database

        separator: str = (
            URI_ADDITIONAL_PARAMETER_SEPARATOR
            if URI_QUERY_SEPARATOR in database
            else URI_QUERY_SEPARATOR
        )
        return f"{database}{separator}{urlencode({'motherduck_token': str(token)})}"
