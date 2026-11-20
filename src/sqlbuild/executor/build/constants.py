"""Build executor constants."""

from __future__ import annotations

from sqlbuild._native import PlanAction

INCREMENTAL_ACTIONS: frozenset[PlanAction] = frozenset(
    {
        PlanAction.INCREMENTAL_APPEND,
        PlanAction.INCREMENTAL_DELETE_INSERT,
        PlanAction.INCREMENTAL_MERGE,
    }
)
DYNAMIC_ACTIONS: frozenset[PlanAction] = frozenset(
    {
        PlanAction.CREATE_DYNAMIC,
        PlanAction.ALTER_DYNAMIC,
        PlanAction.KEEP_DYNAMIC,
    }
)

BUILD_WORKER_FAILED_CODE: str = "B001"
BUILD_UNKNOWN_RESOURCE_FAILED_CODE: str = "B002"
BUILD_MODEL_ENTRY_MISSING_CODE: str = "B003"
BUILD_CUSTOM_MATERIALIZATION_MISSING_CODE: str = "B004"
BUILD_SOURCE_FRESHNESS_BLOCKED_CODE: str = "B005"
