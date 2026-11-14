"""Test case types for `sqb rename` and `sqb mv` e2e tests."""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass(frozen=True)
class ModelRefactorE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_removed: str
    expected_file: str
    expected_fragments: dict[str, tuple[str, ...]]
    expected_relation: str
    extra_files: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class ModelMigrationE2ETestCase:
    description: str
    materialized: str
    udf: bool
    expected_declaration: str
    expected_old_name_type: str
    expected_order_ids: tuple[int, ...]
    extra_files: dict[str, str] = field(default_factory=dict)
    removed_files: tuple[str, ...] = ()
    stripped_fingerprints: tuple[str, ...] = ()


@dataclass(frozen=True)
class PendingMigrationRefactorE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_returncode: int
    expected_file: str
    expected_output: str


@dataclass(frozen=True)
class ColumnRenameE2ETestCase:
    description: str
    command: tuple[str, ...]
    extra_files: dict[str, str] = field(default_factory=dict)
    expected_fragments: dict[str, tuple[str, ...]] = field(default_factory=dict)
    expected_columns: dict[str, tuple[str, ...]] = field(default_factory=dict)
    expected_manual: tuple[str, ...] = ()


@dataclass(frozen=True)
class RefusedRefactorE2ETestCase:
    description: str
    command: tuple[str, ...]
    extra_files: dict[str, str]
    expected_status: str
    expected_reason: str
    expected_paths: tuple[str, ...]


@dataclass(frozen=True)
class ColumnMigrationE2ETestCase:
    description: str
    expected_declaration: str
    expected_columns: tuple[str, ...]
    expected_order_ids: tuple[int, ...]


@dataclass(frozen=True)
class DryRunE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_paths: frozenset[str]


@dataclass(frozen=True)
class CombinedRenameE2ETestCase:
    description: str
    expected_declarations: tuple[str, ...]
    expected_new_columns: tuple[str, ...]
    expected_old_columns: tuple[str, ...]


@dataclass(frozen=True)
class MissingOriginE2ETestCase:
    description: str
    target_settings: str
    build_args: tuple[str, ...]
    expected_exit: int
    expected_output: str
    expected_relation_type: str | None
    expected_columns: tuple[str, ...] = ()


@dataclass(frozen=True)
class DeclarationMoveE2ETestCase:
    description: str
    model: str
    extra_files: dict[str, str]
    destination: str
    expected_moved: dict[str, str]


@dataclass(frozen=True)
class DeclarationMoveRefusalE2ETestCase:
    description: str
    model: str
    extra_files: dict[str, str]
    destination: str
    expected_message: str


@dataclass(frozen=True)
class HiddenOriginE2ETestCase:
    description: str
    target_settings: str
    expected_output: str


@dataclass(frozen=True)
class PendingOriginJanitorE2ETestCase:
    description: str
    janitor_args: tuple[str, ...]
    expected_order_ids: tuple[int, ...]


@dataclass(frozen=True)
class LayerMoveRefusalE2ETestCase:
    description: str
    project_files: dict[str, str]
    old_name: str
    new_name: str
    expected_hint: tuple[str, ...]
    expected_moved_file: str


@dataclass(frozen=True)
class LayerMovePlaceholderE2ETestCase:
    description: str
    project_files: dict[str, str]
    old_name: str
    new_name: str
    expected_hint: tuple[str, ...]


@dataclass(frozen=True)
class OtherRenameRefusalE2ETestCase:
    description: str
    project_files: dict[str, str]
    old_name: str
    new_name: str
    expected_codes: tuple[str, ...]


@dataclass(frozen=True)
class FollowedUnitTestE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_fragments: dict[str, tuple[str, ...]]
    extra_files: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class FollowedUnitTestDryRunE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_status: str
    expected_path: str


@dataclass(frozen=True)
class ScopedUnitTestMoveE2ETestCase:
    description: str
    extra_files: dict[str, str]
    amount_sql: str
    expected_path: str
    expected_notes: tuple[str, ...]


@dataclass(frozen=True)
class BatchMoveE2ETestCase:
    description: str
    arguments: tuple[str, ...]
    move_file: str
    expected_moved: dict[str, str]
    expected_compiles: int


@dataclass(frozen=True)
class SingleMoveOfBatchE2ETestCase:
    description: str
    move: tuple[str, str]
    expected_reason: str


@dataclass(frozen=True)
class BatchMoveConflictE2ETestCase:
    description: str
    files: dict[str, str]
    arguments: tuple[str, ...]
    expected_reason: str


@dataclass(frozen=True)
class BatchMoveDryRunE2ETestCase:
    description: str
    arguments: tuple[str, ...]
    expected_lines: tuple[str, ...]


@dataclass(frozen=True)
class BatchRenameE2ETestCase:
    description: str
    arguments: tuple[str, ...]
    rename_file: str
    expected_files: tuple[str, ...]
    expected_fragments: dict[str, tuple[str, ...]]
    expected_old_name_views: tuple[str, ...]


@dataclass(frozen=True)
class BatchRenameRefusalE2ETestCase:
    description: str
    arguments: tuple[str, ...]
    expected_message: str


@dataclass(frozen=True)
class PythonImportMoveE2ETestCase:
    description: str
    files: dict[str, str]
    expected_fragments: dict[str, tuple[str, ...]]


@dataclass(frozen=True)
class PythonImportRefusalE2ETestCase:
    description: str
    files: dict[str, str]
    expected_location: str
    expected_reason: str
    binary_files: dict[str, bytes] = field(default_factory=dict)


@dataclass(frozen=True)
class RenameUsageErrorE2ETestCase:
    description: str
    arguments: tuple[str, ...]
    expected_returncode: int
    expected_message: str


@dataclass(frozen=True)
class ProjectFailureE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_codes: tuple[str, ...]


@dataclass(frozen=True)
class InvalidRequestJsonE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_code: str
    expected_message: str


@dataclass(frozen=True)
class LayoutRepairE2ETestCase:
    description: str
    files: dict[str, str]
    arguments: tuple[str, ...]
    expected_status: str
    expected_baseline: dict[str, int]
    expected_blocking: tuple[str, ...]
    command: str = "mv"


@dataclass(frozen=True)
class RefusedBaselineE2ETestCase:
    description: str
    files: dict[str, str]
    command: tuple[str, ...]
    expected_codes: tuple[str, ...]


@dataclass(frozen=True)
class LayoutRepairDryRunE2ETestCase:
    description: str
    files: dict[str, str]
    arguments: tuple[str, ...]
    expected_lines: tuple[str, ...]


@dataclass(frozen=True)
class PythonNamedMoveE2ETestCase:
    """A move of a model a Python check names, and whether `sqb mv` carries it out."""

    description: str
    destination: str
    expected_exit_code: int
    expected_model_path: str


@dataclass(frozen=True)
class JobSelectorRefactorE2ETestCase:
    """A rename or move, the job file before and after it, and the job built afterwards."""

    description: str
    command: tuple[str, ...]
    job: str
    expected_job: str
    expected_output: tuple[str, ...] = ()
