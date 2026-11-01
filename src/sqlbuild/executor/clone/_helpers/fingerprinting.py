"""Best-effort fingerprint propagation for clone operations."""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Sequence
from dataclasses import replace
from datetime import UTC, datetime
from typing import Any

from sqlbuild._native import (
    FINGERPRINT_TABLE_NAME,
    NODE_TYPE_MODEL,
    NODE_TYPE_SEED,
    Fingerprint,
    FingerprintSet,
    MaterializationType,
    ModelPlanEntry,
    SeedPlanEntry,
    read_latest_fingerprints,
    write_fingerprints,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.models import RelationLookup
from sqlbuild.adapter.relations.main.relation_lookup import build_relation_lookup
from sqlbuild.executor.clone.models import CloneExecutionResult
from sqlbuild.executor.clone.types import (
    CloneAction,
    CloneFingerprintProgressCallback,
    CloneStatus,
)


def copy_clone_fingerprints(
    *,
    result: CloneExecutionResult,
    origin_model_entries: tuple[ModelPlanEntry, ...],
    destination_model_entries: tuple[ModelPlanEntry, ...],
    origin_seed_entries: tuple[SeedPlanEntry, ...],
    destination_seed_entries: tuple[SeedPlanEntry, ...],
    adapter: BaseAdapter,
    destination_connection: Any,
    run_id: str,
    query_change_tracking: bool,
    on_progress: CloneFingerprintProgressCallback | None = None,
) -> None:
    """Copy source fingerprints for successfully cloned/copied tables and seeds."""

    if not query_change_tracking:
        return
    successful_names: frozenset[str] = frozenset(
        item.name
        for item in result.item_results
        if item.status == CloneStatus.SUCCESS
        and item.action in {CloneAction.CLONED, CloneAction.COPIED}
    )
    if not successful_names:
        return

    copied_seed_fingerprints: tuple[Fingerprint, ...] = _collect_entry_fingerprints(
        node_type=NODE_TYPE_SEED,
        origin_entries=origin_seed_entries,
        destination_entries=destination_seed_entries,
        successful_names=successful_names,
        adapter=adapter,
        connection=destination_connection,
        run_id=run_id,
    )
    table_origin_entries: tuple[ModelPlanEntry, ...] = tuple(
        entry
        for entry in origin_model_entries
        if entry.materialization_type != MaterializationType.VIEW
    )
    table_destination_entries: tuple[ModelPlanEntry, ...] = tuple(
        entry
        for entry in destination_model_entries
        if entry.materialization_type != MaterializationType.VIEW
    )
    copied_fingerprints: tuple[Fingerprint, ...] = (
        copied_seed_fingerprints
        + _collect_entry_fingerprints(
            node_type=NODE_TYPE_MODEL,
            origin_entries=table_origin_entries,
            destination_entries=table_destination_entries,
            successful_names=successful_names,
            adapter=adapter,
            connection=destination_connection,
            run_id=run_id,
        )
    )
    _write_collected_fingerprints(
        fingerprints=copied_fingerprints,
        adapter=adapter,
        connection=destination_connection,
        on_progress=on_progress,
    )


def _collect_entry_fingerprints(
    *,
    node_type: str,
    origin_entries: Sequence[ModelPlanEntry | SeedPlanEntry],
    destination_entries: Sequence[ModelPlanEntry | SeedPlanEntry],
    successful_names: frozenset[str],
    adapter: BaseAdapter,
    connection: Any,
    run_id: str,
) -> tuple[Fingerprint, ...]:
    origin_by_name: dict[str, ModelPlanEntry | SeedPlanEntry] = {
        entry.name: entry for entry in origin_entries
    }
    destination_by_name: dict[str, ModelPlanEntry | SeedPlanEntry] = {
        entry.name: entry for entry in destination_entries
    }
    relevant_origin_entries: tuple[ModelPlanEntry | SeedPlanEntry, ...] = tuple(
        entry
        for entry in origin_entries
        if entry.name in successful_names
        and (destination_entry := destination_by_name.get(entry.name)) is not None
        and destination_entry.destination.schema is not None
    )
    origin_fingerprint_sets: dict[tuple[str | None, str], FingerprintSet] = (
        _read_origin_fingerprint_sets(
            origin_entries=relevant_origin_entries,
            adapter=adapter,
            connection=connection,
        )
    )
    copied_fingerprints: list[Fingerprint] = []
    name: str
    for name in sorted(successful_names):
        origin_entry: ModelPlanEntry | SeedPlanEntry | None = origin_by_name.get(name)
        destination_entry: ModelPlanEntry | SeedPlanEntry | None = destination_by_name.get(name)
        if origin_entry is None or destination_entry is None:
            continue
        fingerprint: Fingerprint | None = _lookup_origin_fingerprint(
            node_type=node_type,
            entry=origin_entry,
            origin_fingerprint_sets=origin_fingerprint_sets,
        )
        if fingerprint is None or destination_entry.destination.schema is None:
            continue
        copied_fingerprints.append(
            replace(
                fingerprint,
                target_database=destination_entry.destination.database,
                target_schema=destination_entry.destination.schema,
                target_name=destination_entry.destination.name,
                run_id=run_id,
                ts=datetime.now(tz=UTC),
            )
        )
    return tuple(copied_fingerprints)


def _write_collected_fingerprints(
    *,
    fingerprints: tuple[Fingerprint, ...],
    adapter: BaseAdapter,
    connection: Any,
    on_progress: CloneFingerprintProgressCallback | None,
) -> None:
    grouped: dict[tuple[str | None, str], list[Fingerprint]] = defaultdict(list)
    fingerprint: Fingerprint
    for fingerprint in fingerprints:
        if fingerprint.target_schema is not None:
            grouped[(fingerprint.target_database, fingerprint.target_schema)].append(fingerprint)
    ordered_groups: tuple[tuple[tuple[str | None, str], tuple[Fingerprint, ...]], ...] = tuple(
        (key, tuple(grouped[key]))
        for key in sorted(grouped, key=lambda item: (item[0] or "", item[1]))
    )
    ordered_fingerprint_list: list[Fingerprint] = []
    ordered_group: tuple[tuple[str | None, str], tuple[Fingerprint, ...]]
    for ordered_group in ordered_groups:
        ordered_fingerprint_list.extend(ordered_group[1])
    ordered_fingerprints: tuple[Fingerprint, ...] = tuple(ordered_fingerprint_list)
    pending_identities: tuple[str, ...] = tuple(
        f"{fingerprint.node_type}:{fingerprint.node_name}" for fingerprint in ordered_fingerprints
    )
    total_count: int = len(ordered_fingerprints)
    completed_before_group: int = 0
    if on_progress is not None:
        on_progress(completed=0, total=total_count, pending_identities=pending_identities)
    namespace: tuple[str | None, str]
    group: tuple[Fingerprint, ...]
    for namespace, group in ordered_groups:

        def _on_group_progress(
            *, completed: int, total: int, group_offset: int = completed_before_group
        ) -> None:
            del total
            confirmed: int = group_offset + completed
            if on_progress is not None:
                on_progress(
                    completed=confirmed,
                    total=total_count,
                    pending_identities=pending_identities[confirmed:],
                )

        write_fingerprints(
            connection=connection,
            execute=adapter.execute,
            database=namespace[0],
            schema=namespace[1],
            fingerprints=group,
            render_qualified_name=adapter.render_qualified_name,
            render_framework_type=adapter.render_framework_type,
            read_columns=adapter.get_columns,
            render_add_columns=adapter.render_add_columns,
            render_create_table_sql=adapter.render_create_fingerprint_table_sql,
            render_create_index_sqls=adapter.render_create_fingerprint_index_sqls,
            on_progress=_on_group_progress,
        )
        completed_before_group += len(group)


def _read_origin_fingerprint_sets(
    *,
    origin_entries: Sequence[ModelPlanEntry | SeedPlanEntry],
    adapter: BaseAdapter,
    connection: Any,
) -> dict[tuple[str | None, str], FingerprintSet]:
    """Read each distinct origin (database, schema) fingerprint state once."""

    fingerprint_table_lookup: RelationLookup = build_relation_lookup(
        adapter=adapter,
        connection=connection,
        locations=tuple(
            (entry.destination.database, entry.destination.schema, FINGERPRINT_TABLE_NAME)
            for entry in origin_entries
            if entry.destination.schema is not None
        ),
    )
    fingerprint_sets: dict[tuple[str | None, str], FingerprintSet] = {}
    entry: ModelPlanEntry | SeedPlanEntry
    for entry in origin_entries:
        schema: str | None = entry.destination.schema
        if schema is None:
            continue
        cache_key: tuple[str | None, str] = (entry.destination.database, schema)
        if cache_key in fingerprint_sets:
            continue
        fingerprint_sets[cache_key] = read_latest_fingerprints(
            connection=connection,
            execute=adapter.execute,
            table_exists=fingerprint_table_lookup.exists(
                database=entry.destination.database,
                schema=schema,
                name=FINGERPRINT_TABLE_NAME,
            ),
            database=entry.destination.database,
            schema=schema,
            render_qualified_name=adapter.render_qualified_name,
            render_read_latest_sql=adapter.render_read_latest_fingerprints_sql,
        )
    return fingerprint_sets


def _lookup_origin_fingerprint(
    *,
    node_type: str,
    entry: ModelPlanEntry | SeedPlanEntry,
    origin_fingerprint_sets: dict[tuple[str | None, str], FingerprintSet],
) -> Fingerprint | None:
    schema: str | None = entry.destination.schema
    if schema is None:
        return None
    fingerprint_set: FingerprintSet | None = origin_fingerprint_sets.get(
        (entry.destination.database, schema)
    )
    if fingerprint_set is None:
        return None
    return (fingerprint_set.fingerprints_by_identity or {}).get((node_type, entry.name))
