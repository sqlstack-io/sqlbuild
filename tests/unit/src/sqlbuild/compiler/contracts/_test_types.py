from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from sqlbuild._native import InferredNullability


@dataclass(frozen=True)
class ContractValidationTestCase:
    description: str
    declared_columns: tuple[tuple[str, str | None], ...]
    inferred_columns: tuple[tuple[str, str | None], ...] | None
    type_enforcement: bool | None
    expected_codes: tuple[str, ...]
    expected_severities: tuple[str, ...]
    expected_messages: tuple[str, ...]
    contract: str | None = None
    check_declared_columns: bool = True
    not_null_audit_columns: tuple[str, ...] = ()
    declared_nullable_by_column: dict[str, bool | None] | None = None
    inferred_nullability_by_column: dict[str, InferredNullability] | None = None
    fast_lineage_has_star: bool = False


@dataclass(frozen=True)
class ContractLocationTestCase:
    description: str
    column_name: str
    path: Path
    line: int
    column: int
    expected_path: Path
    expected_line: int
    expected_column: int


@dataclass(frozen=True)
class ContractMissingDeclarationsTestCase:
    description: str
    contract: str | None
    expected_codes: tuple[str, ...]
    expected_messages: tuple[str, ...]


@dataclass(frozen=True)
class DynamicContractValidationTestCase:
    description: str
    declared_type: str
    output_proven: bool
    inferred_type: str | None
    failure_reason: str | None
    expected_code: str
    expected_message: str


@dataclass(frozen=True)
class PromotionConflictTestCase:
    description: str
    model_configs: tuple[tuple[str, dict[str, object]], ...]
    table_promotion_mode: str | None
    adapter_default: str
    settings_file: str
    expected_resource_names: tuple[str, ...]
    expected_help_fragments: tuple[str, ...] = ()
