from collections.abc import Callable
from dataclasses import dataclass, field

from sqlbuild.adapter.contract.types import TablePromotionMode
from sqlbuild.executor.run.types import ExecutionPhase


@dataclass(frozen=True)
class TableSuccessTestCase:
    """Test case where execution succeeds and target is populated."""

    description: str
    setup_sql: tuple[str, ...]
    model_sql: str
    target_schema: str | None
    target_name: str
    promotion_mode: TablePromotionMode
    expected_row_count: int
    expected_audit_count: int = 0
    type_enforcement: bool = False
    declared_columns: tuple[tuple[str, str], ...] = field(default_factory=tuple)
    pre_hook: object = None
    post_hook: object = None
    audit_sql: str | None = None
    audit_severity: str = "warn"
    extra_audits: tuple[object, ...] = field(default_factory=tuple)
    query_change_tracking: bool = True
    expected_column_names: tuple[str, ...] = field(default_factory=tuple)
    expected_column_types: tuple[str, ...] = field(default_factory=tuple)
    expected_warning_fragment: str | None = None
    hook_functions: tuple[object, ...] = field(default_factory=tuple)
    expected_lifecycle_event_fragments: tuple[str, ...] = field(default_factory=tuple)
    expected_query_results: tuple[tuple[str, tuple[tuple[object, ...], ...]], ...] = field(
        default_factory=tuple
    )


@dataclass(frozen=True)
class TableFailureTestCase:
    """Test case where execution fails at a specific phase."""

    description: str
    setup_sql: tuple[str, ...]
    model_sql: str
    target_schema: str | None
    target_name: str
    promotion_mode: TablePromotionMode
    expected_failed_phase: ExecutionPhase
    expected_audit_count: int = 0
    expected_error_fragment: str | None = None
    expected_staging_relation: str | None = None
    expected_promoted_relation: str | None = None
    expected_row_count: int | None = None
    type_enforcement: bool = False
    declared_columns: tuple[tuple[str, str], ...] = field(default_factory=tuple)
    pre_hook: object = None
    post_hook: object = None
    audit_sql: str | None = None
    audit_severity: str = "warn"
    extra_audits: tuple[object, ...] = field(default_factory=tuple)
    hook_functions: tuple[object, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class AuditSqlResolutionTestCase:
    description: str
    unresolved_sql: str
    attached_target_name: str
    resolved_target_name: str
    expected_resolved_sql: str


@dataclass(frozen=True)
class RebuiltRowCountTestCase:
    """A rebuilt table whose adapter row count read returns or raises."""

    description: str
    promotion_mode: TablePromotionMode
    row_count: Callable[..., int | None]
    expected_table_rows: int | None
