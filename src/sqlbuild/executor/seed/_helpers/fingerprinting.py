"""Fingerprint writes for direct seed loads."""

from __future__ import annotations

import hashlib
from datetime import UTC, datetime
from typing import Any

from sqlbuild._native import NODE_TYPE_SEED, Fingerprint, SeedPlanEntry, write_fingerprint
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter


def try_write_seed_fingerprint(
    *,
    seed_entry: SeedPlanEntry,
    adapter: BaseAdapter,
    connection: Any,
    run_id: str,
    query_change_tracking: bool,
) -> tuple[str, ...]:
    """Write active-target seed identity after a successful load, returning warnings."""

    if not query_change_tracking:
        return ()
    target_schema: str | None = seed_entry.destination.schema
    if target_schema is None:
        return (
            "fingerprint write skipped for "
            f"seed '{seed_entry.name}': target schema is missing while "
            "query_change_tracking is enabled",
        )
    if not seed_entry.fingerprint_version_hash:
        return (
            f"fingerprint write skipped for seed '{seed_entry.name}': seed identity is missing",
        )
    try:
        fingerprint: Fingerprint = Fingerprint(
            node_type=NODE_TYPE_SEED,
            node_name=seed_entry.name,
            target_database=seed_entry.destination.database,
            target_schema=seed_entry.destination.schema,
            target_name=seed_entry.destination.name,
            run_id=run_id,
            definition_hash=seed_entry.fingerprint_version_hash,
            version_hash=seed_entry.fingerprint_version_hash,
            schema_fingerprint=hashlib.sha256(b"").hexdigest(),
            definition=seed_entry.fingerprint_definition,
            metadata_json=seed_entry.fingerprint_metadata_json,
            ts=datetime.now(tz=UTC),
        )
        _ = write_fingerprint(
            connection=connection,
            execute=adapter.execute,
            database=seed_entry.destination.database,
            schema=target_schema,
            fingerprint=fingerprint,
            render_qualified_name=adapter.render_qualified_name,
            render_framework_type=adapter.render_framework_type,
            read_columns=adapter.get_columns,
            render_add_columns=adapter.render_add_columns,
            render_create_table_sql=adapter.render_create_fingerprint_table_sql,
            render_create_index_sqls=adapter.render_create_fingerprint_index_sqls,
        )
    except Exception as exc:
        return (
            f"fingerprint write failed for seed '{seed_entry.name}'; "
            f"future changes-only seed planning may be incorrect: {exc}",
        )
    return ()
