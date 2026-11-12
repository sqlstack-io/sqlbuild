"""Answer a plan preview's warehouse reads from a location's synced copy, and refuse the rest."""

from __future__ import annotations

import base64
import json
from pathlib import Path
from typing import Any

import duckdb

from sqlbuild._native import build_read_latest_sql
from sqlbuild.adapter.contract.classes.synced_copy_connection import SyncedCopyConnection
from sqlbuild.adapter.contract.exceptions import SyncedCopyGapError, WarehouseAccessRefusedError
from sqlbuild.adapter.contract.models import ColumnInfo, RelationInfo

_FINGERPRINT_COLUMNS: tuple[str, ...] = (
    "state_database",
    "state_schema",
    "node_type",
    "node_name",
    "target_database",
    "target_schema",
    "target_name",
    "run_id",
    "definition_hash",
    "version_hash",
    "schema_fingerprint",
    "definition_b64",
    "metadata_json_b64",
    "ts",
)
_FINGERPRINT_VIEW_PREFIX: str = "__sqlbuild_synced_fingerprints_"


def _same(left: str | None, right: str | None) -> bool:
    return (left or "").casefold() == (right or "").casefold()


class SyncedCopyAdapterMixin:
    """Placed before a resolved adapter's class, so its dialect and rendering stay the adapter's own."""

    _synced_relations: tuple[dict[str, Any], ...]
    _synced_fingerprints: duckdb.DuckDBPyConnection
    _synced_fingerprint_views: dict[tuple[str, str], str]

    def load_synced_copy(self, path: Path) -> None:
        """Read the copy a location wrote for this preview and stage its fingerprint rows locally."""

        copy: dict[str, Any] = json.loads(path.read_text())
        self._synced_relations = tuple(copy.get("relations", ()))
        self._synced_fingerprints = duckdb.connect(":memory:")
        self._synced_fingerprints.execute(
            "CREATE TABLE fingerprints ("
            + ", ".join(
                f"{name} {'TIMESTAMP' if name == 'ts' else 'VARCHAR'}"
                for name in _FINGERPRINT_COLUMNS
            )
            + ")"
        )
        rows: list[tuple[Any, ...]] = [
            (
                record.get("state_database"),
                record["state_schema"],
                record["node_type"],
                record["node_name"],
                record.get("target_database"),
                record.get("target_schema"),
                record.get("target_name"),
                record["run_id"],
                record["definition_hash"],
                record["version_hash"],
                record["schema_fingerprint"],
                base64.b64encode(record["definition"].encode()).decode(),
                base64.b64encode(record["metadata_json"].encode()).decode(),
                record["ts"],
            )
            for record in copy.get("fingerprints", ())
        ]
        if rows:
            self._synced_fingerprints.executemany(
                f"INSERT INTO fingerprints VALUES ({', '.join('?' for _ in _FINGERPRINT_COLUMNS)})",
                rows,
            )
        self._synced_fingerprint_views = {}

    def connect(self, connection_config: Any) -> Any:
        return SyncedCopyConnection()

    def close(self, connection: Any) -> None:
        return None

    def list_relations(
        self,
        *,
        connection: Any,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[RelationInfo, ...]:
        return tuple(
            RelationInfo(
                database=database,
                schema=record["state_schema"],
                name=record["name"],
                relation_type=record["relation_type"],
            )
            for record in self._listed(database, schemas, names)
        )

    def get_all_columns(
        self,
        *,
        connection: Any,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> dict[str, tuple[ColumnInfo, ...]]:
        return {
            record["name"]: self._columns(record)
            for record in self._listed(database, schemas, names)
        }

    def get_columns(
        self, *, connection: Any, database: str | None, schema: str | None, name: str
    ) -> tuple[ColumnInfo, ...]:
        listed: tuple[dict[str, Any], ...] = self._listed(
            database, None if schema is None else (schema,), (name,)
        )
        return self._columns(listed[0]) if listed else ()

    def get_columns_for_relations(
        self, *, connection: Any, relations: tuple[RelationInfo, ...]
    ) -> dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]]:
        found: dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]] = {}
        for relation in relations:
            listed: tuple[dict[str, Any], ...] = self._listed(
                relation.database,
                None if relation.schema is None else (relation.schema,),
                (relation.name,),
            )
            if listed:
                found[(relation.database, relation.schema, relation.name)] = self._columns(
                    listed[0]
                )
        return found

    def relation_exists(
        self, *, connection: Any, database: str | None, schema: str | None, name: str
    ) -> bool:
        return bool(self._listed(database, None if schema is None else (schema,), (name,)))

    def render_read_latest_fingerprints_sql(self, *, database: str | None, schema: str) -> str:
        key: tuple[str, str] = ((database or "").casefold(), schema.casefold())
        view: str | None = self._synced_fingerprint_views.get(key)
        if view is None:
            view = f"{_FINGERPRINT_VIEW_PREFIX}{len(self._synced_fingerprint_views)}"
            database_literal, schema_literal = (part.replace("'", "''") for part in key)
            self._synced_fingerprints.execute(
                f'CREATE VIEW "{view}" AS SELECT * FROM fingerprints '
                f"WHERE lower(coalesce(state_database, '')) = '{database_literal}' "
                f"AND lower(state_schema) = '{schema_literal}'"
            )
            self._synced_fingerprint_views[key] = view
        return build_read_latest_sql(
            database=database,
            schema=schema,
            render_qualified_name=lambda **_: f'"{view}"',
        )

    def execute(self, *, connection: Any, sql: str) -> Any:
        if _FINGERPRINT_VIEW_PREFIX not in sql:
            raise WarehouseAccessRefusedError(sql.strip().split("\n", 1)[0][:120])
        return self._synced_fingerprints.execute(sql)

    def query_column_names(self, *, connection: Any, sql: str) -> tuple[str, ...]:
        raise SyncedCopyGapError("the columns of a query-defined source")

    def _listed(
        self,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None,
    ) -> tuple[dict[str, Any], ...]:
        return tuple(
            record
            for record in self._synced_relations
            if (database is None or _same(record.get("state_database"), database))
            and (schemas is None or any(_same(record["state_schema"], s) for s in schemas))
            and (names is None or any(_same(record["name"], n) for n in names))
        )

    def _columns(self, record: dict[str, Any]) -> tuple[ColumnInfo, ...]:
        return tuple(
            ColumnInfo(name=column["name"], type=column["data_type"])
            for column in record.get("columns", ())
        )


def synced_copy_adapter(*, adapter: Any, path: Path) -> Any:
    """`adapter` answering warehouse reads from the synced copy at `path`; nothing reaches the warehouse."""

    facade_class: type = type(
        f"SyncedCopy{type(adapter).__name__}", (SyncedCopyAdapterMixin, type(adapter)), {}
    )
    facade: Any = object.__new__(facade_class)
    facade.__dict__.update(adapter.__dict__)
    facade.load_synced_copy(path)
    return facade
