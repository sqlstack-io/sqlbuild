"""Test case types for the run-time relation guard."""

from dataclasses import dataclass, field

from sqlbuild._native import HardCodedRelationOwnerKind
from sqlbuild.python_nodes.models import SqlResourceRef


@dataclass(frozen=True)
class RuntimeRelationGuardTestCase:
    """SQL a Python node sends, and the hard-coded-name warnings it produces."""

    description: str
    statements: tuple[str, ...]
    expected_warning_fragments: tuple[str, ...]
    expected_warning_count: int
    resolved_refs: tuple[SqlResourceRef, ...] = field(default_factory=tuple)
    own_refs: frozenset[SqlResourceRef] = frozenset()
    owner_label: str = "task 'export_orders'"
    owner_kind: HardCodedRelationOwnerKind = HardCodedRelationOwnerKind.NODE
    upstream_loader_by_source: dict[str, str] = field(default_factory=dict)
    dialect: str = "duckdb"
