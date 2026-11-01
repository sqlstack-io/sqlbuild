"""Fingerprint write for model materialization lifecycle."""

from __future__ import annotations

import hashlib
from contextlib import AbstractContextManager, nullcontext
from datetime import UTC, datetime
from typing import Any

from sqlbuild._native import (
    NODE_TYPE_MODEL,
    AuditPlanEntry,
    Fingerprint,
    ModelPlanEntry,
    compute_query_hash,
    write_fingerprint,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.executor.auditing.models import AuditExecutionResult
from sqlbuild.executor.run._helpers.reuse.fingerprint_metadata import (
    model_fingerprint_metadata_with_audit_gate,
    model_fingerprint_metadata_with_migration_fingerprint,
)
from sqlbuild.executor.run.classes.fingerprint_writes import (
    FingerprintTableLocation,
    FingerprintWrites,
)


def try_write_fingerprint(
    *,
    entry: ModelPlanEntry,
    adapter: BaseAdapter,
    connection: Any,
    run_id: str,
    query_change_tracking: bool,
    model_audits: tuple[AuditPlanEntry, ...] = (),
    audit_results: tuple[AuditExecutionResult, ...] = (),
    fingerprint_writes: FingerprintWrites | None = None,
) -> tuple[str, ...]:
    """Attempt a best-effort fingerprint write unless the latest row matches, returning warnings."""

    if not query_change_tracking:
        return ()
    target_schema: str | None = entry.destination.schema
    if target_schema is None:
        return (
            "fingerprint write skipped for "
            f"'{entry.name}': target schema is missing while query_change_tracking is enabled",
        )
    try:
        schema_fp: str = hashlib.sha256(b"").hexdigest()
        definition_hash: str = compute_query_hash(
            query_sql=entry.fingerprint_query_sql, dialect=adapter.sql_analysis_dialect()
        )
        metadata_json: str = model_fingerprint_metadata_with_audit_gate(
            metadata_json=entry.fingerprint_metadata_json,
            model_audits=model_audits,
            audit_results=audit_results,
            run_id=run_id,
        )
        metadata_json = model_fingerprint_metadata_with_migration_fingerprint(
            metadata_json=metadata_json, migration_fingerprint=entry.migration_fingerprint
        )
        fingerprint: Fingerprint = Fingerprint(
            node_type=NODE_TYPE_MODEL,
            node_name=entry.name,
            target_database=entry.destination.database,
            target_schema=entry.destination.schema,
            target_name=entry.destination.name,
            run_id=run_id,
            definition_hash=definition_hash,
            version_hash=entry.fingerprint_version_hash or definition_hash,
            schema_fingerprint=schema_fp,
            definition=entry.fingerprint_query_sql,
            metadata_json=metadata_json,
            ts=datetime.now(tz=UTC),
        )
        location: FingerprintTableLocation = (entry.destination.database, target_schema)
        if fingerprint_writes is not None and not fingerprint_writes.records(
            location=location, previous=entry.previous_fingerprint, current=fingerprint
        ):
            return ()
        setting_up: AbstractContextManager[bool] = (
            nullcontext(True)
            if fingerprint_writes is None
            else fingerprint_writes.setting_up(location=location)
        )
        with setting_up as requested_setup:
            execute_fn: Any = adapter.execute
            set_up: bool = write_fingerprint(
                ensure_table=requested_setup,
                connection=connection,
                execute=execute_fn,
                database=entry.destination.database,
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
            f"fingerprint write failed for '{entry.name}'; "
            f"future query-change detection may be incorrect: {exc}",
        )
    return ()
