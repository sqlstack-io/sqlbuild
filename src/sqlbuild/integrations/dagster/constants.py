"""Dagster integration protocol constants."""

from __future__ import annotations

from sqlbuild.compiler.dag.types import NodeKind
from sqlbuild.executor.node_results.types import NodeResultStatus
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.python_nodes.types import PythonCheckSeverity

DAGSTER_ASSET_ENABLED_FIELD: str = "_dagster_asset_enabled"
DAGSTER_CHECK_ENABLED_FIELD: str = "_dagster_check_enabled"
DAGSTER_CHECK_NAME_FIELD: str = "_dagster_check_name"

DAGSTER_ASSET_NODE_KIND_MEMBERS: frozenset[NodeKind] = frozenset(
    {
        NodeKind.SOURCE,
        NodeKind.LOADER,
        NodeKind.SEED,
        NodeKind.MODEL,
        NodeKind.UDF,
        NodeKind.TABLE_FN,
        NodeKind.ASSET,
    }
)
DAGSTER_ASSET_NODE_KIND_EXCLUSIONS: frozenset[NodeKind] = frozenset(
    {NodeKind.CHECK, NodeKind.SQL_TEST, NodeKind.AUDIT, NodeKind.SCENARIO, NodeKind.PYTHON_CHECK}
)
DAGSTER_ASSET_NODE_KINDS: frozenset[str] = frozenset(
    kind.value for kind in DAGSTER_ASSET_NODE_KIND_MEMBERS
)
DAGSTER_DIRECT_KIND_NODE_KIND_MEMBERS: frozenset[NodeKind] = frozenset(
    DAGSTER_ASSET_NODE_KIND_MEMBERS - {NodeKind.MODEL}
)
DAGSTER_DIRECT_KIND_NODE_KIND_EXCLUSIONS: frozenset[NodeKind] = frozenset(
    DAGSTER_ASSET_NODE_KIND_EXCLUSIONS | {NodeKind.MODEL}
)
DAGSTER_DIRECT_KIND_NODE_KINDS: frozenset[str] = frozenset(
    kind.value for kind in DAGSTER_DIRECT_KIND_NODE_KIND_MEMBERS
)
MODEL_NODE_KIND: str = NodeKind.MODEL.value
SOURCE_NODE_KIND: str = NodeKind.SOURCE.value
LOADER_NODE_KIND: str = NodeKind.LOADER.value
VIEW_MATERIALIZATION_TYPE: str = "view"

ASSET_SELECTION_COMMANDS: frozenset[str] = frozenset(
    {"build", "run", "test", "check", "audit", "seed", "load", "clone"}
)
CLONE_COMMAND: str = "clone"
CHECK_COMMAND: str = "check"
STANDALONE_CHECK_COMMANDS: frozenset[str] = frozenset({"audit", CHECK_COMMAND, "test"})
LIVE_EVENT_COMMANDS: frozenset[str] = ASSET_SELECTION_COMMANDS
EXPLICIT_SELECTION_FLAGS: frozenset[str] = frozenset({"--select", "-s", "--select-file"})
SCENARIO_CHECK_KIND: str = NodeKind.SCENARIO.value
LOAD_SELECTABLE_NODE_KIND_MEMBERS: frozenset[NodeKind] = frozenset(
    {NodeKind.SOURCE, NodeKind.LOADER}
)
LOAD_SELECTABLE_NODE_KIND_EXCLUSIONS: frozenset[NodeKind] = frozenset(
    {
        NodeKind.SEED,
        NodeKind.UDF,
        NodeKind.TABLE_FN,
        NodeKind.MODEL,
        NodeKind.ASSET,
        NodeKind.CHECK,
        NodeKind.SQL_TEST,
        NodeKind.AUDIT,
        NodeKind.SCENARIO,
        NodeKind.PYTHON_CHECK,
    }
)
LOAD_SELECTABLE_NODE_KINDS: frozenset[str] = frozenset(
    kind.value for kind in LOAD_SELECTABLE_NODE_KIND_MEMBERS
)
DEFAULT_SELECTABLE_NODE_KIND_MEMBERS: frozenset[NodeKind] = frozenset(
    {NodeKind.SOURCE, NodeKind.SEED, NodeKind.MODEL, NodeKind.UDF, NodeKind.TABLE_FN}
)
DEFAULT_SELECTABLE_NODE_KIND_EXCLUSIONS: frozenset[NodeKind] = frozenset(
    {
        NodeKind.LOADER,
        NodeKind.ASSET,
        NodeKind.CHECK,
        NodeKind.SQL_TEST,
        NodeKind.AUDIT,
        NodeKind.SCENARIO,
        NodeKind.PYTHON_CHECK,
    }
)
DEFAULT_SELECTABLE_NODE_KINDS: frozenset[str] = frozenset(
    kind.value for kind in DEFAULT_SELECTABLE_NODE_KIND_MEMBERS
)
MATERIALIZABLE_NODE_KIND_MEMBERS: frozenset[NodeKind] = frozenset(
    {
        NodeKind.SOURCE,
        NodeKind.LOADER,
        NodeKind.SEED,
        NodeKind.MODEL,
        NodeKind.UDF,
        NodeKind.TABLE_FN,
    }
)
MATERIALIZABLE_NODE_KIND_EXCLUSIONS: frozenset[NodeKind] = frozenset(
    {
        NodeKind.ASSET,
        NodeKind.CHECK,
        NodeKind.SQL_TEST,
        NodeKind.AUDIT,
        NodeKind.SCENARIO,
        NodeKind.PYTHON_CHECK,
    }
)
MATERIALIZABLE_NODE_KINDS: frozenset[str] = frozenset(
    kind.value for kind in MATERIALIZABLE_NODE_KIND_MEMBERS
)
COMPLETED_EXECUTION_STATUS_MEMBERS: frozenset[ExecutionStatus] = frozenset(
    {ExecutionStatus.SUCCESS, ExecutionStatus.SKIPPED}
)
COMPLETED_EXECUTION_STATUS_EXCLUSIONS: frozenset[ExecutionStatus] = frozenset(
    {ExecutionStatus.FAILED}
)
COMPLETED_NODE_RESULT_STATUS_MEMBERS: frozenset[NodeResultStatus] = frozenset(
    {NodeResultStatus.SUCCESS, NodeResultStatus.SKIPPED}
)
COMPLETED_NODE_RESULT_STATUS_EXCLUSIONS: frozenset[NodeResultStatus] = frozenset(
    {NodeResultStatus.FAILED, NodeResultStatus.WARN}
)
COMPLETED_EXECUTION_STATUSES: frozenset[str] = frozenset(
    status.value for status in COMPLETED_EXECUTION_STATUS_MEMBERS
)
SUCCESS_EXECUTION_STATUS: str = ExecutionStatus.SUCCESS.value
FAILED_EXECUTION_STATUS: str = ExecutionStatus.FAILED.value
CHECK_METADATA_EXCLUDED_KEYS: frozenset[str] = frozenset(
    {"passed", "steps", "expected_results", "assertion_results"}
)
WARNING_CHECK_SEVERITY_MEMBERS: frozenset[PythonCheckSeverity] = frozenset(
    {PythonCheckSeverity.WARN}
)
WARNING_CHECK_SEVERITY_EXCLUSIONS: frozenset[PythonCheckSeverity] = frozenset(
    {PythonCheckSeverity.ERROR}
)
WARNING_CHECK_SEVERITY: str = PythonCheckSeverity.WARN.value
INSUFFICIENT_CHECK_STATUS: str = "insufficient"
