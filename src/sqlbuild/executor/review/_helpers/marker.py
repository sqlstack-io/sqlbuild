"""Write and read the immutable ownership markers of review schemas."""

from __future__ import annotations

import json
from datetime import UTC, datetime
from typing import Any

from sqlbuild._native import (
    FINGERPRINT_TABLE_NAME,
    Fingerprint,
    FingerprintSet,
    parse_review_schema_name,
    read_latest_fingerprints,
    write_fingerprint,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.models import RelationLookup
from sqlbuild.adapter.relations.main.relation_lookup import build_relation_lookup
from sqlbuild.executor.review.constants import REVIEW_MARKER_VERSION, REVIEW_SCHEMA_NODE_TYPE
from sqlbuild.executor.review.models import LeftoverReviewSchema, ReviewSchemaMarker


def write_review_marker(
    *, adapter: BaseAdapter, connection: Any, marker: ReviewSchemaMarker, created_at: datetime
) -> None:
    """Append the marker that proves SQLBuild created `marker.schema` for one run."""

    metadata_json: str = json.dumps(
        {"marker_version": REVIEW_MARKER_VERSION, "created_at": created_at.isoformat()},
        sort_keys=True,
        separators=(",", ":"),
    )
    _ = write_fingerprint(
        connection=connection,
        execute=adapter.execute,
        database=marker.marker_database,
        schema=marker.marker_schema,
        fingerprint=Fingerprint(
            node_type=REVIEW_SCHEMA_NODE_TYPE,
            node_name=marker.schema,
            target_database=marker.database,
            target_schema=marker.schema,
            target_name=marker.schema,
            run_id=marker.run_id,
            definition_hash="",
            version_hash="",
            schema_fingerprint="",
            definition="",
            metadata_json=metadata_json,
            ts=created_at,
        ),
        render_qualified_name=adapter.render_qualified_name,
        render_framework_type=adapter.render_framework_type,
        read_columns=adapter.get_columns,
        render_add_columns=adapter.render_add_columns,
        render_create_table_sql=adapter.render_create_fingerprint_table_sql,
        render_create_index_sqls=adapter.render_create_fingerprint_index_sqls,
    )


def marked_review_schemas(
    *, adapter: BaseAdapter, connection: Any, database: str | None, schema: str
) -> tuple[LeftoverReviewSchema, ...]:
    """Review schemas whose markers `schema` holds, with the creation time each name encodes."""

    lookup: RelationLookup = build_relation_lookup(
        adapter=adapter,
        connection=connection,
        locations=((database, schema, FINGERPRINT_TABLE_NAME),),
    )
    fingerprints: FingerprintSet = read_latest_fingerprints(
        connection=connection,
        execute=adapter.execute,
        table_exists=lookup.exists(database=database, schema=schema, name=FINGERPRINT_TABLE_NAME),
        database=database,
        schema=schema,
        render_qualified_name=adapter.render_qualified_name,
        render_read_latest_sql=adapter.render_read_latest_fingerprints_sql,
    )
    marked: list[LeftoverReviewSchema] = []
    for (node_type, _), fingerprint in (fingerprints.fingerprints_by_identity or {}).items():
        if node_type != REVIEW_SCHEMA_NODE_TYPE or fingerprint.target_schema is None:
            continue
        created_at: datetime | None = parse_review_schema_name(fingerprint.target_schema)
        if created_at is None:
            continue
        marked.append(
            LeftoverReviewSchema(
                database=fingerprint.target_database,
                schema=fingerprint.target_schema,
                created_at=created_at.astimezone(UTC),
            )
        )
    return tuple(sorted(marked, key=lambda item: (item.created_at, item.schema)))
