"""One SQLLoad session: a warehouse connection SQLLoad alone uses, in autocommit."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.models import QueryResult
from sqlbuild.executor.load_sessions.constants import (
    BIGQUERY_ADAPTER,
    CLICKHOUSE_ADAPTER,
    POSTGRES_ADAPTER,
)
from sqlbuild.executor.load_sessions.exceptions import LoadSessionError


class LoadSession:
    """Runs SQLLoad's statements on its own connection and answers its queries as text."""

    def __init__(self, *, adapter: BaseAdapter, connection: Any) -> None:
        self._adapter: BaseAdapter = adapter
        self._connection: Any = connection

    def execute(self, *, statement: str) -> None:
        """Run one statement as written; SQLLoad sends its own BEGIN, COMMIT and ROLLBACK."""
        self._adapter.execute(connection=self._connection, sql=statement)

    def query_text(self, *, query: str) -> list[list[str | None]]:
        """Each row of `query`, every value as text and `None` for NULL."""
        result: QueryResult = self._adapter.query(
            connection=self._connection, sql=query, limit=None
        )
        rows: list[list[str | None]] = []
        row: tuple[object, ...]
        for row in result.rows:
            rows.append([_text(value=value) for value in row])
        return rows

    def copy_in(self, *, statement: str, data: bytes) -> None:
        """Run a Postgres `COPY ... FROM STDIN (FORMAT binary)` with `data` as its input."""
        if self._adapter.adapter_name != POSTGRES_ADAPTER:
            raise LoadSessionError(
                f"a {self._adapter.adapter_name} session cannot run COPY FROM STDIN"
            )
        raw: Any = _raw(connection=self._connection)
        with raw.cursor() as cursor, cursor.copy(statement) as copy:
            copy.write(data)

    def load_parquet(self, *, table: tuple[str | None, str, str], file: str) -> None:
        """Append the Parquet file at `file` to `table`, `(database, schema, name)`."""
        if self._adapter.adapter_name == BIGQUERY_ADAPTER:
            _load_bigquery(connection=self._connection, table=table, file=Path(file))
            return
        if self._adapter.adapter_name == CLICKHOUSE_ADAPTER:
            _load_clickhouse(connection=self._connection, table=table, file=Path(file))
            return
        raise LoadSessionError(f"a {self._adapter.adapter_name} session cannot load Parquet files")

    def close(self) -> None:
        """Close the session's connection."""
        self._adapter.close(self._connection)


def _text(*, value: object) -> str | None:
    if value is None:
        return None
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, bytes):
        return value.decode()
    return str(value)


def _raw(*, connection: Any) -> Any:
    return getattr(connection, "raw_connection", connection)


def _load_bigquery(*, connection: Any, table: tuple[str | None, str, str], file: Path) -> None:
    from google.cloud import bigquery

    client: Any = _raw(connection=connection)
    database, schema, name = table
    project: str = database or client.project
    config: Any = bigquery.LoadJobConfig(
        source_format=bigquery.SourceFormat.PARQUET,
        write_disposition=bigquery.WriteDisposition.WRITE_APPEND,
    )
    with file.open("rb") as handle:
        client.load_table_from_file(
            handle, f"{project}.{schema}.{name}", job_config=config
        ).result()


def _load_clickhouse(*, connection: Any, table: tuple[str | None, str, str], file: Path) -> None:
    _, schema, name = table
    _raw(connection=connection).insert_parquet(
        table=f"`{schema}`.`{name}`", payload=file.read_bytes()
    )
