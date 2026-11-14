"""Test types for janitor e2e tests."""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass(frozen=True)
class JanitorDisabledE2ETestCase:
    """Test case for disabled janitor command behavior."""

    description: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_stdout_fragments: tuple[str, ...]


@dataclass(frozen=True)
class JanitorCleanupE2ETestCase:
    """Test case for tracked-only janitor cleanup behavior."""

    description: str
    build_command: tuple[str, ...]
    janitor_command: tuple[str, ...]
    expected_exit_code: int
    expected_stdout_fragments: tuple[str, ...]
    expected_existing_tables: tuple[str, ...]
    expected_missing_tables: tuple[str, ...]
    expected_archived_original_names: tuple[str, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class JanitorSourceOverlapE2ETestCase:
    """Test case for fail-closed source and managed schema overlap."""

    description: str
    janitor_command: tuple[str, ...]
    expected_exit_code: int
    expected_stdout_fragments: tuple[str, ...]
    expected_existing_relations: tuple[tuple[str, str], ...]


@dataclass(frozen=True)
class JanitorInvalidConfigE2ETestCase:
    """Test case for invalid janitor config behavior."""

    description: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_stderr_fragments: tuple[str, ...]
    expected_stdout_fragments: tuple[str, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class JanitorDirectStatePruningE2ETestCase:
    description: str
    build_command: tuple[str, ...]
    janitor_command: tuple[str, ...]
    plan_command: tuple[str, ...]
    expected_exit_code: int
    expected_stdout_fragments: tuple[str, ...]
    expected_fingerprint_count_before: int
    expected_fingerprint_count_after: int
    expected_source_freshness_count_before: int
    expected_source_freshness_count_after: int
    expected_fingerprint_run_ids_after: tuple[str, ...]
    expected_source_freshness_run_ids_after: tuple[str, ...]
    expected_audit_results_after: tuple[tuple[str, str], ...]


@dataclass(frozen=True)
class JanitorArchiveRetentionE2ETestCase:
    """Test case for direct-mode archival after the relation retention period."""

    description: str
    janitor_command: tuple[str, ...]
    retired_model_ages_days: dict[str, int]
    expected_archived_names: tuple[str, ...]
    expected_retained_names: tuple[str, ...]
    expected_stdout_fragments: tuple[str, ...]
    expected_second_run_fragments: tuple[str, ...]


@dataclass(frozen=True)
class JanitorArchiveExpiryE2ETestCase:
    """Test case for name-based deletion of expired archives."""

    description: str
    janitor_command: tuple[str, ...]
    expected_deleted_names: tuple[str, ...]
    expected_kept_names: tuple[str, ...]
    expected_kept_other_schema_names: tuple[str, ...]
    expected_stdout_fragments: tuple[str, ...]
    expected_delete_event_count: int
    expected_total_event_count: int


@dataclass(frozen=True)
class JanitorZeroRetentionE2ETestCase:
    """Test case for archive and delete in one run."""

    description: str
    janitor_command: tuple[str, ...]
    stale_tables: tuple[str, ...]
    stale_views: tuple[str, ...]
    expected_stdout_fragments: tuple[str, ...]
    expected_event_types: tuple[tuple[str, str], ...]
    expected_fingerprint_count_after: int
    expected_source_freshness_count_after: int


@dataclass(frozen=True)
class JanitorArchiveNameFittingE2ETestCase:
    """Test case for identifier-fitted archive names."""

    description: str
    janitor_command: tuple[str, ...]
    long_relation_name: str
    identifier_limit: int
    expected_archive_prefix_length: int


@dataclass(frozen=True)
class JanitorArchiveInterruptionE2ETestCase:
    """Test case for an audit write failure after a rename."""

    description: str
    janitor_command: tuple[str, ...]
    stale_table: str
    expected_first_exit_code: int
    expected_first_output_fragments: tuple[str, ...]
    expected_second_exit_code: int
    expected_second_stdout_fragments: tuple[str, ...]


@dataclass(frozen=True)
class JanitorUnaddressableRelationE2ETestCase:
    """Test case for relations the janitor cannot safely address unquoted."""

    description: str
    janitor_command: tuple[str, ...]
    plain_relation: str
    quoted_relation: str
    expected_stdout_fragments: tuple[str, ...]


@dataclass(frozen=True)
class JanitorTargetPreviewE2ETestCase:
    """Test case for an inspection-only janitor --as preview of another target."""

    description: str
    janitor_command: tuple[str, ...]
    expected_stdout_fragments: tuple[str, ...]
    unexpected_stdout_fragments: tuple[str, ...]


@dataclass(frozen=True)
class JanitorTargetCleanupE2ETestCase:
    """Test case for janitor cleanup scoped to one selected target namespace."""

    description: str
    janitor_command: tuple[str, ...]
    cleaned_schema: str
    untouched_schema: str
    expected_stdout_fragments: tuple[str, ...]
    expected_deleted_names: tuple[str, ...]
    expected_archived_original_names: tuple[str, ...]
    expected_fingerprint_probe_count_after: int


@dataclass(frozen=True)
class JanitorTargetErrorE2ETestCase:
    """Test case for rejected janitor target flags."""

    description: str
    janitor_command: tuple[str, ...]
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class JanitorNonInteractiveE2ETestCase:
    """Test case for a janitor confirmation that stdin cannot answer."""

    description: str
    janitor_command: tuple[str, ...]
    expected_exit_code: int
    expected_stderr_fragments: tuple[str, ...]
    expected_existing_tables: tuple[str, ...]


@dataclass(frozen=True)
class JanitorProgressE2ETestCase:
    """Test case for streamed janitor phase and action lines."""

    description: str
    janitor_command: tuple[str, ...]
    stale_tables: int
    expected_exit_code: int
    expected_ordered_lines: tuple[str, ...]
    expected_archive_events: int
    unexpected_fragments: tuple[str, ...] = field(default_factory=tuple)
    fail_rename: str | None = None
    expected_remaining_tables: tuple[str, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class JanitorJsonE2ETestCase:
    """Test case for `sqb janitor --json` output streams."""

    description: str
    janitor_command: tuple[str, ...]
    stale_tables: int
    expected_status: str
    expected_archived: int
    expected_item_statuses: tuple[str, ...]
    expected_stderr_fragments: tuple[str, ...]


@dataclass(frozen=True)
class JanitorInspectionCountE2ETestCase:
    """Test case for warehouse reads and audit writes per schema."""

    description: str
    stale_tables: dict[str, int]
    renamed_models: tuple[str, ...]
    expected_listings_per_schema: int
    expected_multi_schema_listings: int
    expected_existence_queries_per_schema: int
    expected_view_definition_reads: int
    expected_batched_view_definition_reads: int
    expected_event_inserts: dict[str, int]


@dataclass(frozen=True)
class JanitorConcurrencyE2ETestCase:
    """Test case comparing concurrent janitor actions with sequential ones."""

    description: str
    stale_tables: dict[str, int]
    concurrency: int
    expected_exit_code: int
    expected_connection_line: str


@dataclass(frozen=True)
class JanitorConcurrentFailureE2ETestCase:
    """Test case for one failing action among concurrent janitor actions."""

    description: str
    stale_tables: dict[str, int]
    concurrency: int
    fail_rename: str
    expected_exit_code: int
    expected_failed_lines: int
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class JanitorShortenedArchiveE2ETestCase:
    """Test case for the audit event of an archive whose name was shortened."""

    description: str
    long_relation_name: str
    identifier_limit: int
    expected_rows_in_event_insert: int
    expected_event_count: int


@dataclass(frozen=True)
class JanitorJsonErrorE2ETestCase:
    """Test case for the `--json` error document of a janitor run that fails before cleanup."""

    description: str
    janitor_command: tuple[str, ...]
    expected_exit_code: int
    expected_error_code: str
    expected_stderr_fragment: str


@dataclass(frozen=True)
class JanitorJsonFailedRunE2ETestCase:
    """Test case for the single `--json` document of a janitor run that fails mid-phase."""

    description: str
    janitor_command: tuple[str, ...]
    fail_rename: str
    expected_exit_code: int
    expected_status: str
    expected_error_fragment: str
    expected_failed_items: int


@dataclass(frozen=True)
class JanitorUsageUncheckedE2ETestCase:
    """Test case for janitor runs on an adapter with no usage source."""

    description: str
    janitor_command: tuple[str, ...]
    expected_ordered_lines: tuple[str, ...]
    expected_archived: int


@dataclass(frozen=True)
class JanitorUsageE2ETestCase:
    """Test case for janitor runs whose adapter reports which candidates are read."""

    description: str
    janitor_config: str
    janitor_command: tuple[str, ...]
    reads: dict[str, tuple[tuple[str, ...], int]]
    expected_ordered_lines: tuple[str, ...]
    unexpected_fragments: tuple[str, ...]
    expected_remaining: tuple[str, ...]
    expected_archived: int
    via: dict[str, tuple[str, ...]] = field(default_factory=dict)


@dataclass(frozen=True)
class JanitorUsagePromptE2ETestCase:
    """Test case for the confirmation prompt's usage line."""

    description: str
    janitor_command: tuple[str, ...]
    reads: dict[str, tuple[tuple[str, ...], int]]
    unavailable: str | None
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class JanitorUsageJsonE2ETestCase:
    """Test case for the usage fields of `sqb janitor --json`."""

    description: str
    janitor_command: tuple[str, ...]
    expected_usage_check: dict[str, object]
    expected_archive_in_use: tuple[tuple[str, object], ...]
    expected_kept_in_use: tuple[tuple[str, object], ...]
    expected_archived: int


@dataclass(frozen=True)
class JanitorKeepInUseIncompleteE2ETestCase:
    """Test case for `--keep-in-use` when usage is unavailable or only partly read."""

    description: str
    reads: dict[str, tuple[tuple[str, ...], int]]
    unavailable: str | None
    notices: tuple[str, ...]
    expected_error: str


@dataclass(frozen=True)
class JanitorUsageFailureE2ETestCase:
    """Test case for janitor runs whose usage source fails outright."""

    description: str
    failure: str
    expected_ordered_lines: tuple[str, ...]
