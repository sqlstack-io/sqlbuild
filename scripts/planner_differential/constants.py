"""Constants for the planner differential."""

from __future__ import annotations

import re

from scripts.planner_differential.models import PlannerStep

PLAN_CAPTURE_ENV_VAR: str = "SQLBUILD_PLANNER_CAPTURE_DIR"
MODE_RECORD: str = "record"
MODE_CHECK: str = "check"
MODES: tuple[str, ...] = (MODE_RECORD, MODE_CHECK)
DEFAULT_BASELINE_DIR: str = "/tmp/sqlbuild-planner-differential/baseline"
DEFAULT_SEED_COUNT: int = 6
DEFAULT_JOBS: int = 2
BASELINE_SUFFIX: str = ".json"
ACTUAL_SUFFIX: str = ".actual"
FIXTURE_ROOT: str = "tests/e2e/fixtures"
EXAMPLE_ROOT: str = "website/examples"
PROJECT_CONFIG_FILE: str = "sqlbuild_project.toml"
PROJECT_DIRECTORY: str = "project"
CAPTURE_DIRECTORY: str = "capture"
DUCKDB_ADAPTER: str = "duckdb"
DUCKDB_SUFFIX: str = ".duckdb"
COMMAND_TIMEOUT_SECONDS: float = 600.0
SQB_ENTRY: str = "import sys; from sqlbuild.cli.entry.main.entry import main; sys.exit(main())"
EXCLUDED_ENVIRONMENT_KEYS: frozenset[str] = frozenset(
    {"VIRTUAL_ENV", PLAN_CAPTURE_ENV_VAR, "SQLBUILD_COMPILER_STAGE_CAPTURE_DIR"}
)
EXCLUDED_ENVIRONMENT_PREFIX: str = "DBT_"
COPY_IGNORED: tuple[str, ...] = (
    "target",
    "logs",
    "__pycache__",
    ".sqlbuild",
    "*.duckdb",
    "*.duckdb.wal",
)

STATE_TABLE_PREFIX: str = "_sqlbuild"
ADDED_MODEL_NAME: str = "planner_differential_added"
ADDED_MODEL_PATH: str = "models/planner_differential_added.sql"
ADDED_COLUMN_NAME: str = "planner_differential_flag"
RENAMED_SUFFIX: str = "_renamed"
EDITED_DESCRIPTION: str = "Edited by the planner differential."
SOURCE_GROWTH_INTERVAL: str = "INTERVAL 1 DAY"
MAX_DIFF_LINES: int = 60

ROLE_TABLE: str = "table"
ROLE_INCREMENTAL: str = "incremental"
ROLE_MICROBATCH: str = "microbatch"
ROLE_CURSOR: str = "cursor"
ROLE_LEAF: str = "leaf"
ROLE_ANY: str = "any"

MUTATION_EDIT_QUERY: str = "edit_query"
MUTATION_TAGS: str = "tags"
MUTATION_CONFIG: str = "config"
MUTATION_MATERIALIZATION: str = "materialization"
MUTATION_DECLARE_COLUMN: str = "declare_column"
MUTATION_RENAME_COLUMN: str = "rename_column"
MUTATION_ADD_MODEL: str = "add_model"
MUTATION_RENAME_MODEL: str = "rename_model"
MUTATION_DROP_RELATION: str = "drop_relation"
MUTATION_SOURCE_GROWTH: str = "source_growth"

SELECT_MODEL_ARGUMENTS: tuple[str, ...] = ("--select", "{model}")
STEPS: tuple[PlannerStep, ...] = (
    PlannerStep(name="initial"),
    PlannerStep(name="no-op"),
    PlannerStep(name="edit-query", mutation=MUTATION_EDIT_QUERY, role=ROLE_TABLE),
    PlannerStep(name="tag-change", mutation=MUTATION_TAGS, role=ROLE_TABLE, index=1),
    PlannerStep(name="config-change", mutation=MUTATION_CONFIG, role=ROLE_INCREMENTAL, index=1),
    PlannerStep(name="declared-column", mutation=MUTATION_DECLARE_COLUMN, role=ROLE_INCREMENTAL),
    PlannerStep(name="column-rename", mutation=MUTATION_RENAME_COLUMN, role=ROLE_INCREMENTAL),
    PlannerStep(name="add-model", mutation=MUTATION_ADD_MODEL, role=ROLE_TABLE),
    PlannerStep(name="rename-model", mutation=MUTATION_RENAME_MODEL, role=ROLE_LEAF),
    PlannerStep(name="drop-relation", mutation=MUTATION_DROP_RELATION, role=ROLE_TABLE),
    PlannerStep(name="source-growth", mutation=MUTATION_SOURCE_GROWTH, role=ROLE_CURSOR),
    PlannerStep(
        name="max-microbatches", role=ROLE_MICROBATCH, arguments=("--max-microbatches", "1")
    ),
    PlannerStep(
        name="cursor-override",
        role=ROLE_CURSOR,
        arguments=(
            *SELECT_MODEL_ARGUMENTS,
            "--start-cursor",
            "{start_cursor}",
            "--end-cursor",
            "{end_cursor}",
        ),
    ),
    PlannerStep(
        name="exclude-edited",
        mutation=MUTATION_EDIT_QUERY,
        role=ROLE_TABLE,
        index=1,
        arguments=("--exclude", "{model}"),
    ),
    PlannerStep(name="select-edited", role=ROLE_TABLE, index=1, arguments=SELECT_MODEL_ARGUMENTS),
    PlannerStep(
        name="materialization-change", mutation=MUTATION_MATERIALIZATION, role=ROLE_TABLE, index=2
    ),
    PlannerStep(name="full-refresh", arguments=("--full-refresh",)),
    PlannerStep(name="final-no-op"),
)

PROJECT_PATH_MASK: str = "<project>"
WORK_PATH_MASK: str = "<work>"
RECENT_TIMESTAMP_MASK: str = "<recent-timestamp>"
VOLATILE_VALUE_MASK: str = "<volatile>"
ELAPSED_TIME_PATTERN: re.Pattern[str] = re.compile(r"\b\d+(?:\.\d+)?\s?m?s\b")
ELAPSED_TIME_MASK: str = "<elapsed>"
ISO_TIMESTAMP_PATTERN: re.Pattern[str] = re.compile(
    r"\b(\d{4}-\d{2}-\d{2})[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:[+-]\d{2}:?\d{2}|Z)?"
)
COMPACT_TIMESTAMP_PATTERN: re.Pattern[str] = re.compile(r"(?<!\d)(\d{8})[Tt]\d{6}[Zz](?!\d)")
RECENT_TIMESTAMP_DAYS: int = 2
DATE_PATTERN: re.Pattern[str] = re.compile(r"(?<![\d-])(\d{4}-\d{2}-\d{2})(?![\d:Tt-])")
RUN_RELATIVE_PAST_DAYS: int = 7
RUN_RELATIVE_FUTURE_DAYS: int = 400
MAPPING_MARKER: str = "__mapping__"
VOLATILE_KEY_SUFFIXES: tuple[str, ...] = ("event_id", "execution_id")
VOLATILE_KEYS: frozenset[str] = frozenset(
    {
        "invocation_time",
        "invocation_id",
        "run_id",
        "result_id",
        "ts",
        "occurred_at",
        "compile_seconds",
        "planning_seconds",
        "elapsed_seconds",
        "duration_seconds",
        "duration_ms",
        "started_at",
        "completed_at",
        "created_at",
        "updated_at",
        "recorded_at",
        "input_version",
    }
)
BASE64_COLUMN_SUFFIX: str = "_b64"
BUILD_SUMMARY_PREFIXES: tuple[str, ...] = ("✓ Completed", "✗ Completed")
ERROR_LINE_PATTERN: re.Pattern[str] = re.compile(r"^\s*(error|warning)\b|^\s*(error|warning)\[")
