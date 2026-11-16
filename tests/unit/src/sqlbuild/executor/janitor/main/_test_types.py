from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime

from sqlbuild.adapter.contract.models import RelationInfo
from sqlbuild.executor.janitor.models import (
    JanitorRelationKey,
)


@dataclass(frozen=True)
class JanitorPlanTestCase:
    description: str
    relation_infos: tuple[RelationInfo, ...]
    source_schema: str | None = None
    direct_mode: bool = True
    retention_days: int = 7
    direct_state_history_versions: int = 20
    delete_tracked_only: bool = False
    supports_age_metadata: bool = True
    tracked_relations: tuple[tuple[str | None, str | None, str], ...] = field(default_factory=tuple)
    exclude_patterns: tuple[str, ...] = field(default_factory=tuple)
    protected_relation_keys: frozenset[JanitorRelationKey] = frozenset()
    migrate_from: str | None = None
    expected_candidate_names: tuple[str, ...] = field(default_factory=tuple)
    expected_direct_state_table_names: tuple[str, ...] = field(default_factory=tuple)
    expected_skipped_relation_reasons: tuple[str, ...] = field(default_factory=tuple)
    expected_skipped_schema_sources: tuple[str, ...] = field(default_factory=tuple)
    expected_blocked_schema_sources: tuple[str, ...] = field(default_factory=tuple)
    expected_suppressed_candidate_names: tuple[str, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class JanitorTargetSchemaTestCase:
    description: str
    target_schema: str
    expected_inspected: bool


@dataclass(frozen=True)
class JanitorExecuteTestCase:
    description: str
    relation_infos: tuple[RelationInfo, ...]
    expected_dropped_targets: tuple[str, ...]
    direct_mode: bool = False
    expected_pruned_table_names: tuple[str, ...] = field(default_factory=tuple)


def relation_info(
    name: str,
    *,
    schema: str = "analytics",
    database: str | None = None,
    relation_type: str = "BASE TABLE",
    created_at: datetime | None = None,
    last_altered_at: datetime | None = None,
) -> RelationInfo:
    return RelationInfo(
        database=database,
        schema=schema,
        name=name,
        relation_type=relation_type,
        created_at=created_at,
        last_altered_at=last_altered_at,
    )


@dataclass(frozen=True)
class JanitorArchivePlanTestCase:
    description: str
    relation_infos: tuple[RelationInfo, ...]
    direct_mode: bool = True
    retention_days: int = 7
    archive_retention_days: int = 14
    source_schema: str | None = None
    exclude_patterns: tuple[str, ...] = field(default_factory=tuple)
    identifier_limit: int = 255
    expected_archive_source_names: tuple[str, ...] = field(default_factory=tuple)
    expected_existing_archive_deletion_names: tuple[str, ...] = field(default_factory=tuple)
    expected_same_run_deletion_source_names: tuple[str, ...] = field(default_factory=tuple)
    expected_retained_archive_names: tuple[str, ...] = field(default_factory=tuple)
    expected_skipped_relations: tuple[tuple[str, str], ...] = field(default_factory=tuple)
    expected_suppressed_archive_deletion_names: tuple[str, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class JanitorArchiveExecutionTestCase:
    description: str
    relation_infos: tuple[RelationInfo, ...]
    archive_retention_days: int
    expected_renamed_origins: tuple[str, ...]
    expected_view_rename_origins: tuple[str, ...]
    expected_dropped_targets: tuple[str, ...]
    expected_dropped_view_targets: tuple[str, ...]
    expected_event_insert_count: int
    expected_event_row_count: int
    expected_event_table_create_count: int


@dataclass(frozen=True)
class JanitorArchiveEventFailureTestCase:
    description: str
    relation_infos: tuple[RelationInfo, ...]
    expected_error_fragment: str
    expected_renamed_origins: tuple[str, ...]
    expected_dropped_targets: tuple[str, ...]


@dataclass(frozen=True)
class JanitorAddressingPlanTestCase:
    description: str
    relation_infos: tuple[RelationInfo, ...]
    direct_mode: bool
    expected_archive_source_names: tuple[str, ...] = field(default_factory=tuple)
    expected_archive_deletion_names: tuple[str, ...] = field(default_factory=tuple)
    expected_skipped_relations: tuple[tuple[str, str], ...] = field(default_factory=tuple)
    expected_candidate_names: tuple[str, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class JanitorFoldedIdentifierPlanTestCase:
    description: str
    relation_infos: tuple[RelationInfo, ...]
    direct_mode: bool = True
    delete_tracked_only: bool = False
    tracked_relations: tuple[tuple[str | None, str | None, str], ...] = field(default_factory=tuple)
    source_database: str | None = None
    source_schema: str | None = None
    protected_relation_keys: frozenset[JanitorRelationKey] = frozenset()
    expected_candidate_display_names: tuple[str, ...] = field(default_factory=tuple)
    expected_archive_display_names: tuple[str, ...] = field(default_factory=tuple)
    expected_archive_deletion_display_names: tuple[str, ...] = field(default_factory=tuple)
    expected_skipped_relations: tuple[tuple[str, str], ...] = field(default_factory=tuple)
    expected_blocked_sources: tuple[str, ...] = field(default_factory=tuple)
    expected_suppressed_names: tuple[str, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class JanitorSeparateAgeMetadataPlanTestCase:
    description: str
    relation_ages: dict[str, datetime]
    expected_candidate_names: tuple[str, ...]
    expected_skipped_relations: tuple[tuple[str, str], ...]


@dataclass(frozen=True)
class JanitorAgeReadScopeTestCase:
    description: str
    relation_infos: tuple[RelationInfo, ...]
    direct_mode: bool
    delete_tracked_only: bool
    tracked_relations: tuple[tuple[str | None, str | None, str], ...]
    exclude_patterns: tuple[str, ...]
    protected_relation_keys: frozenset[JanitorRelationKey]
    expected_age_requests: tuple[tuple[str, ...], ...]
    retention_days: int = 7


@dataclass(frozen=True)
class JanitorCaseSafetyTestCase:
    description: str
    relation_names: tuple[str, ...]
    direct_mode: bool
    tracked_names: tuple[str, ...]
    exclude_patterns: tuple[str, ...]
    expected_skipped_relations: tuple[tuple[str, str], ...]
    expected_dropped_targets: tuple[str, ...]


@dataclass(frozen=True)
class JanitorPhaseEventSafetyTestCase:
    description: str
    connection_count: int
    item_count: int
    failing_report: int
    report_error: type[BaseException]
    expected_error: type[BaseException]
    expected_minimum_completed: int


@dataclass(frozen=True)
class JanitorPhaseInterruptTestCase:
    description: str
    connection_count: int
    item_count: int
    expected_error: type[BaseException]
    expected_minimum_completed: int


@dataclass(frozen=True)
class JanitorPhaseSubmitInterruptTestCase:
    description: str
    connection_count: int
    item_count: int
    failing_submit: int
    expected_error: type[BaseException]
    expected_maximum_completed: int


@dataclass(frozen=True)
class JanitorPhaseFlushFailureTestCase:
    description: str
    item_count: int
    failing_item: int
    action_error: type[BaseException]
    flush_error: type[BaseException]
    expected_error: type[BaseException]
    expected_chained_error: type[BaseException]
    expected_unwritten: tuple[tuple[int, tuple[str, ...], str], ...]
