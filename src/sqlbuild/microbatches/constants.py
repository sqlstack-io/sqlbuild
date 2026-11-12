"""Stable constants for microbatch state storage and retries."""

from sqlbuild._native import StateSqlValueType

DIRECT_MICROBATCH_SCOPE_KIND: str = "direct_logical"
MICROBATCH_GENERATION_COMMENT_PREFIX: str = "sqlbuild-generation:"
MICROBATCH_REPLAY_GENERATION_PREFIX: str = "replay:"
MICROBATCH_GENERATION_WILDCARD: str = "*"
MICROBATCH_WRITE_ATTEMPTS: int = 3
MICROBATCH_WRITE_RETRY_BASE_SECONDS: float = 0.05
MICROBATCH_INTEGER_COLUMNS: frozenset[str] = frozenset({"rows_affected", "observed_row_count"})

MICROBATCH_COLUMNS: tuple[str, ...] = (
    "event_id",
    "record_type",
    "scope_kind",
    "scope_key",
    "model_name",
    "target_database",
    "target_schema",
    "target_name",
    "physical_generation_id",
    "origin_run_id",
    "origin_run_started_at",
    "execution_run_id",
    "execution_run_started_at",
    "run_type",
    "completion_type",
    "run_start",
    "run_end",
    "partition_start",
    "partition_end",
    "batch_size",
    "cursor_column",
    "cursor_type",
    "cursor_grain",
    "model_version_hash",
    "definition_hash",
    "fingerprint_status",
    "replay_requirement_id",
    "required_model_version_hash",
    "previous_model_version_hash",
    "replay_policy",
    "rows_affected",
    "completed_at",
    "coverage_source",
    "observed_row_count",
    "observed_at",
    "synthetic_reason",
    "unaccounted_policy",
    "created_at",
)

MICROBATCH_TIMESTAMP_COLUMNS: frozenset[str] = frozenset(
    {
        "origin_run_started_at",
        "execution_run_started_at",
        "completed_at",
        "observed_at",
        "created_at",
    }
)

MICROBATCH_COLUMN_TYPES: dict[str, StateSqlValueType] = {
    column: (
        StateSqlValueType.INTEGER
        if column in MICROBATCH_INTEGER_COLUMNS
        else StateSqlValueType.TIMESTAMP
        if column in MICROBATCH_TIMESTAMP_COLUMNS
        else StateSqlValueType.STRING
    )
    for column in MICROBATCH_COLUMNS
}

MICROBATCH_DIRECT_INDEXES: dict[str, tuple[str, ...]] = {
    "_sqlbuild_microbatches_scope_idx": (
        "scope_kind",
        "scope_key",
        "physical_generation_id",
        "created_at",
        "event_id",
    ),
    "_sqlbuild_microbatches_model_idx": ("model_name", "created_at"),
    "_sqlbuild_microbatches_run_idx": ("execution_run_id", "created_at"),
    "_sqlbuild_microbatches_requirement_idx": (
        "replay_requirement_id",
        "record_type",
        "created_at",
    ),
    "_sqlbuild_microbatches_partition_idx": (
        "scope_key",
        "physical_generation_id",
        "partition_start",
        "partition_end",
        "created_at",
    ),
}

RETIRED_MICROBATCH_RECORD_TYPES: frozenset[str] = frozenset(
    {"producer_completion", "consumer_frontier"}
)
