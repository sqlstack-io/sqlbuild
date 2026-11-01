"""Run-owned raw-query diff artifact lifecycle."""

from __future__ import annotations

import hashlib
import json
from datetime import UTC, datetime
from typing import Any

from sqlbuild._native import (
    FINGERPRINT_TABLE_NAME,
    Fingerprint,
    FingerprintSet,
    build_query_diff_artifact_name,
    compute_query_hash,
    parse_query_diff_artifact_name,
    read_latest_fingerprints,
    write_fingerprint,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.models import ColumnInfo, RelationInfo, RelationLookup
from sqlbuild.adapter.relations.classes.inspection_catalog import InspectionCatalog
from sqlbuild.adapter.relations.main.active_inspection_catalog import active_inspection_catalog
from sqlbuild.adapter.relations.main.relation_lookup import build_relation_lookup
from sqlbuild.compiler.fingerprints.constants import NODE_TYPE_QUERY_DIFF_ARTIFACT
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.diff.models import (
    QueryDiffArtifact,
    QueryDiffArtifactCleanupResult,
    QueryDiffArtifactInspection,
)

_ARTIFACT_VERSION: int = 1


def build_query_diff_artifact(
    *,
    adapter: BaseAdapter,
    run_id: str,
    side: str,
    database: str | None,
    schema: str,
) -> QueryDiffArtifact:
    """Build one collision-resistant artifact identity in the active target schema."""

    name: str = build_query_diff_artifact_name(run_id=run_id, side=side)
    relation: str | None = adapter.render_qualified_name(
        database=database,
        schema=schema,
        name=name,
    )
    if relation is None:
        raise ExecutorInputError("query diff requires an active target schema", code="X306")
    return QueryDiffArtifact(
        side=side,
        run_id=run_id,
        database=database,
        schema=schema,
        name=name,
        relation=relation,
        node_name=f"{run_id}:{side}",
    )


def materialize_query_diff_artifact(
    *,
    adapter: BaseAdapter,
    connection: Any,
    artifact: QueryDiffArtifact,
    sql: str,
    expires_at: datetime,
) -> None:
    """Materialize a query and publish fingerprint ownership or clean up on failure."""

    recorder: StatementRecorder = StatementRecorder()
    adapter.create_table_as(
        connection=connection,
        destination=artifact.relation,
        sql=sql,
        statement_recorder=recorder,
    )
    try:
        columns: tuple[ColumnInfo, ...] = adapter.describe_relation(
            connection=connection,
            relation=artifact.relation,
        )
        definition_hash: str = compute_query_hash(
            query_sql=sql, dialect=adapter.sql_analysis_dialect()
        )
        metadata_json: str = json.dumps(
            {
                "artifact_version": _ARTIFACT_VERSION,
                "expires_at": expires_at.astimezone(UTC).isoformat(),
                "side": artifact.side,
            },
            sort_keys=True,
            separators=(",", ":"),
        )
        _ = write_fingerprint(
            connection=connection,
            execute=adapter.execute,
            database=artifact.database,
            schema=artifact.schema,
            fingerprint=Fingerprint(
                node_type=NODE_TYPE_QUERY_DIFF_ARTIFACT,
                node_name=artifact.node_name,
                target_database=artifact.database,
                target_schema=artifact.schema,
                target_name=artifact.name,
                run_id=artifact.run_id,
                definition_hash=definition_hash,
                version_hash=definition_hash,
                schema_fingerprint=_schema_fingerprint(columns=columns),
                definition="",
                metadata_json=metadata_json,
                ts=datetime.now(UTC),
            ),
            render_qualified_name=adapter.render_qualified_name,
            render_framework_type=adapter.render_framework_type,
            read_columns=adapter.get_columns,
            render_add_columns=adapter.render_add_columns,
            render_create_table_sql=adapter.render_create_fingerprint_table_sql,
            render_create_index_sqls=adapter.render_create_fingerprint_index_sqls,
        )
    except Exception:
        adapter.drop(
            connection=connection,
            destination=artifact.relation,
            statement_recorder=recorder,
        )
        raise


def cleanup_query_diff_artifact(
    *,
    adapter: BaseAdapter,
    connection: Any,
    artifact: QueryDiffArtifact,
) -> None:
    """Drop one artifact while retaining immutable ownership evidence."""

    adapter.drop(
        connection=connection,
        destination=artifact.relation,
        statement_recorder=StatementRecorder(),
    )


def cleanup_expired_query_diff_artifacts(
    *,
    adapter: BaseAdapter,
    connection: Any,
    database: str | None,
    schema: str,
    now: datetime,
) -> QueryDiffArtifactCleanupResult:
    """Remove expired physical artifacts with matching ownership fingerprints."""

    inspection: QueryDiffArtifactInspection = inspect_query_diff_artifacts(
        adapter=adapter,
        connection=connection,
        database=database,
        schema=schema,
        now=now,
    )
    cleaned: list[str] = []
    for artifact in inspection.expired:
        cleanup_query_diff_artifact(
            adapter=adapter,
            connection=connection,
            artifact=artifact,
        )
        cleaned.append(artifact.relation)
    return QueryDiffArtifactCleanupResult(
        cleaned=tuple(cleaned),
        untracked=inspection.untracked,
    )


def inspect_query_diff_artifacts(
    *,
    adapter: BaseAdapter,
    connection: Any,
    database: str | None,
    schema: str,
    now: datetime,
) -> QueryDiffArtifactInspection:
    """Classify strict-name query artifacts using immutable fingerprint ownership."""

    catalog: InspectionCatalog | None = active_inspection_catalog(
        adapter=adapter, connection=connection
    )
    relations: tuple[RelationInfo, ...] = (
        adapter.list_relations(connection=connection, database=database, schemas=(schema,))
        if catalog is None
        else catalog.list_relations(database=database, schemas=(schema,))
    )
    candidates: tuple[RelationInfo, ...] = tuple(
        relation
        for relation in relations
        if parse_query_diff_artifact_name(relation.name) is not None
    )
    if not candidates:
        return QueryDiffArtifactInspection()
    lookup: RelationLookup = build_relation_lookup(
        adapter=adapter,
        connection=connection,
        locations=((database, schema, FINGERPRINT_TABLE_NAME),),
    )
    fingerprints: FingerprintSet = read_latest_fingerprints(
        connection=connection,
        execute=adapter.execute,
        table_exists=lookup.exists(
            database=database,
            schema=schema,
            name=FINGERPRINT_TABLE_NAME,
        ),
        database=database,
        schema=schema,
        render_qualified_name=adapter.render_qualified_name,
        render_read_latest_sql=adapter.render_read_latest_fingerprints_sql,
    )
    owned: dict[str, Fingerprint] = {
        fingerprint.target_name.lower(): fingerprint
        for (node_type, _), fingerprint in (fingerprints.fingerprints_by_identity or {}).items()
        if node_type == NODE_TYPE_QUERY_DIFF_ARTIFACT and fingerprint.target_name is not None
    }
    expired: list[QueryDiffArtifact] = []
    untracked: list[RelationInfo] = []
    for relation in candidates:
        fingerprint: Fingerprint | None = owned.get(relation.name.lower())
        parts: tuple[str, str] | None = parse_query_diff_artifact_name(relation.name)
        if parts is None:
            continue
        artifact: QueryDiffArtifact = build_query_diff_artifact(
            adapter=adapter,
            run_id=parts[0],
            side=parts[1],
            database=database,
            schema=schema,
        )
        metadata: dict[str, object] | None = (
            _fingerprint_metadata(fingerprint=fingerprint) if fingerprint is not None else None
        )
        if (
            fingerprint is None
            or metadata is None
            or not _fingerprint_matches_artifact(
                fingerprint=fingerprint,
                metadata=metadata,
                relation=relation,
                artifact=artifact,
            )
        ):
            untracked.append(relation)
            continue
        expires_at: datetime | None = _metadata_expiry(metadata=metadata)
        if expires_at is None:
            untracked.append(relation)
            continue
        if expires_at <= now:
            expired.append(artifact)
    return QueryDiffArtifactInspection(
        expired=tuple(expired),
        untracked=tuple(untracked),
    )


def _schema_fingerprint(*, columns: tuple[ColumnInfo, ...]) -> str:
    payload: str = "\n".join(f"{column.name}:{column.type}" for column in columns)
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def _fingerprint_matches_artifact(
    *,
    fingerprint: Fingerprint,
    metadata: dict[str, object],
    relation: RelationInfo,
    artifact: QueryDiffArtifact,
) -> bool:
    return (
        fingerprint.node_type == NODE_TYPE_QUERY_DIFF_ARTIFACT
        and fingerprint.node_name.lower() == artifact.node_name.lower()
        and fingerprint.run_id.lower() == artifact.run_id.lower()
        and (fingerprint.target_database or "").lower() == (relation.database or "").lower()
        and (fingerprint.target_schema or "").lower() == (relation.schema or "").lower()
        and (fingerprint.target_name or "").lower() == relation.name.lower()
        and metadata.get("artifact_version") == _ARTIFACT_VERSION
        and metadata.get("side") == artifact.side
    )


def _fingerprint_metadata(*, fingerprint: Fingerprint) -> dict[str, object] | None:
    try:
        payload: object = json.loads(fingerprint.metadata_json)
        if not isinstance(payload, dict):
            return None
        return {str(key): value for key, value in payload.items()}
    except (ValueError, TypeError, json.JSONDecodeError):
        return None


def _metadata_expiry(*, metadata: dict[str, object]) -> datetime | None:
    raw: object = metadata.get("expires_at")
    if not isinstance(raw, str):
        return None
    try:
        parsed: datetime = datetime.fromisoformat(raw)
    except ValueError:
        return None
    return parsed if parsed.tzinfo is not None else parsed.replace(tzinfo=UTC)
