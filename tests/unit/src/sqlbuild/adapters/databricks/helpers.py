"""Helpers for Databricks adapter unit tests."""

from __future__ import annotations

from typing import Any

from sqlbuild.adapter.contract.models import RelationInfo, RelationUsageReport, RetentionRequest
from sqlbuild.adapter.contract.types import RetentionScope
from sqlbuild.adapters.databricks.classes.databricks_adapter import DatabricksAdapter


def build_retention_request(*, desired_days: int) -> RetentionRequest:
    return RetentionRequest(
        request_id="model.results",
        scope=RetentionScope.RELATION,
        database="main",
        schema="mart",
        name="results",
        desired_days=desired_days,
    )


class FakeDatabricksMetadataCursor:
    def __init__(
        self,
        *,
        rows: list[tuple[object, ...]] | None = None,
        description: tuple[tuple[str], ...] = (("format",), ("properties",)),
    ) -> None:
        self.rows: list[tuple[object, ...]] = rows or []
        self.description: tuple[tuple[str], ...] = description
        self.executed_sql: str | None = None
        self.closed: bool = False

    def execute(self, sql: str) -> None:
        self.executed_sql = sql

    def fetchall(self) -> list[tuple[object, ...]]:
        return self.rows

    def fetchone(self) -> tuple[object, ...] | None:
        return next(iter(self.rows), None)

    def close(self) -> None:
        self.closed = True


class ServerOperationError(Exception):
    """Stand-in named like the connector's error for a statement the server rejected."""


class RequestError(Exception):
    """Stand-in named like the connector's error for a failed request to the server."""


class FailingDatabricksMetadataCursor(FakeDatabricksMetadataCursor):
    def __init__(self, *, execute_error: Exception) -> None:
        super().__init__()
        self.execute_error = execute_error

    def execute(self, sql: str) -> None:
        self.executed_sql = sql
        raise self.execute_error


class FakeDatabricksMetadataConnection:
    def __init__(self, cursors: tuple[FakeDatabricksMetadataCursor, ...]) -> None:
        self._cursors: list[FakeDatabricksMetadataCursor] = list(cursors)
        self.returned_cursors: list[FakeDatabricksMetadataCursor] = []

    def cursor(self) -> Any:
        cursor: FakeDatabricksMetadataCursor = self._cursors.pop(0)
        self.returned_cursors.append(cursor)
        return cursor


class FakeLineageError(Exception):
    """A Databricks server error with a SQLSTATE."""

    def __init__(self, message: str, sqlstate: str | None) -> None:
        super().__init__(message)
        self.sqlstate: str | None = sqlstate


class FakeLineageCursor:
    """Answers lineage reads from stored rows, recording each read."""

    def __init__(self, connection: FakeLineageConnection) -> None:
        self.connection: FakeLineageConnection = connection
        self.rows: list[tuple[object, ...]] = []

    def execute(self, sql: str, params: dict[str, object]) -> None:
        self.connection.queries.append((sql, params))
        error: Exception
        for error in self.connection.errors:
            raise error
        names: frozenset[object] = frozenset(params.values())
        self.rows = list(filter(lambda row: row[0] in names, self.connection.rows))

    def fetchall(self) -> list[tuple[object, ...]]:
        return self.rows

    def close(self) -> None:
        return None


class FakeLineageConnection:
    def __init__(
        self,
        *,
        rows: tuple[tuple[object, ...], ...] = (),
        errors: tuple[Exception, ...] = (),
    ) -> None:
        self.rows: tuple[tuple[object, ...], ...] = rows
        self.errors: tuple[Exception, ...] = errors
        self.queries: list[tuple[str, dict[str, object]]] = []

    def cursor(self) -> FakeLineageCursor:
        return FakeLineageCursor(self)


def usage_relation(database: str | None, schema: str, name: str) -> RelationInfo:
    """A table relation for usage reads."""

    return RelationInfo(database=database, schema=schema, name=name, relation_type="table")


def read_databricks_usage(
    *, connection: FakeLineageConnection, relations: tuple[RelationInfo, ...], window_seconds: int
) -> RelationUsageReport:
    """Databricks' usage report for the relations from the fake connection."""

    return DatabricksAdapter().relation_usage(
        connection=connection, relations=relations, window_seconds=window_seconds
    )


class RecordingDatabricksAdapter(DatabricksAdapter):
    """Databricks adapter that records each statement instead of running it."""

    def __init__(self) -> None:
        super().__init__()
        self.executed: list[str] = []

    def execute(self, *, connection: Any, sql: str, **kwargs: Any) -> Any:
        del connection, kwargs
        self.executed.append(sql)
        return None
