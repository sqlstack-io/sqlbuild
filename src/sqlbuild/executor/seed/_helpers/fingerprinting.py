"""Fingerprint writes for direct seed loads."""

from __future__ import annotations

import hashlib
from contextlib import AbstractContextManager, nullcontext
from datetime import UTC, datetime
from typing import Any

from sqlbuild._native import NODE_TYPE_SEED, Fingerprint, SeedPlanEntry, write_fingerprint
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.executor.run.classes.fingerprint_writes import FingerprintWrites
from sqlbuild.executor.run.types import FingerprintTableLocation


def try_write_seed_fingerprint(
    *,
    seed_entry: SeedPlanEntry,
    adapter: BaseAdapter,
    connection: Any,
    run_id: str,
    query_change_tracking: bool,
    fingerprint_writes: FingerprintWrites | None = None,
) -> tuple[str, ...]:
    """Write a loaded seed's identity on the active target, setting each table up once per run."""

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
        location: FingerprintTableLocation = (seed_entry.destination.database, target_schema)
        setting_up: AbstractContextManager[bool] = (
            nullcontext(True)
            if fingerprint_writes is None
            else fingerprint_writes.setting_up(location=location)
        )
        with setting_up as requested_setup:
            set_up: bool = write_fingerprint(
                ensure_table=requested_setup,
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
            if fingerprint_writes is not None:
                fingerprint_writes.written(
                    location=location, requested_setup=requested_setup, set_up=set_up
                )
    except Exception as exc:
        return (
            f"fingerprint write failed for seed '{seed_entry.name}'; "
            f"future changes-only seed planning may be incorrect: {exc}",
        )
    return ()
