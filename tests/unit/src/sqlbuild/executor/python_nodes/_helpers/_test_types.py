from collections.abc import Callable
from dataclasses import dataclass

from sqlbuild.compiler.python_nodes.types import (
    PythonNodeFanInAction,
    PythonNodeKind,
    PythonNodeStatus,
    SkipMode,
)
from sqlbuild.python_nodes.types import PythonCheckSeverity


@dataclass(frozen=True)
class PythonNodeSchedulerTestCase:
    description: str
    node_names: tuple[str, ...]
    upstream_names: dict[str, tuple[str, ...]]
    downstream_names: dict[str, tuple[str, ...]]
    completion_order: tuple[str, ...]
    expected_initial_ready: tuple[str, ...]
    expected_final_ready: tuple[str, ...]
    expected_final_in_degree: dict[str, int]


@dataclass(frozen=True)
class PythonNodeReturnNormalizationTestCase:
    description: str
    kind: PythonNodeKind
    returned: object
    expected_status: PythonNodeStatus
    expected_payload: object | None
    expected_metadata: dict[str, object]
    expected_materialized: bool | None
    expected_skip_mode: SkipMode | None
    expected_skip_reason: str | None


@dataclass(frozen=True)
class PythonNodeFanInPolicyTestCase:
    description: str
    upstream_statuses: tuple[PythonNodeStatus, ...]
    upstream_skip_modes: tuple[SkipMode | None, ...]
    expected_action: PythonNodeFanInAction
    expected_reason: str | None


@dataclass(frozen=True)
class PythonNodeFailureResultTestCase:
    description: str
    error: BaseException
    expected_status: PythonNodeStatus
    expected_error_message: str


@dataclass(frozen=True)
class PythonNodeReturnNormalizationErrorTestCase:
    description: str
    kind: PythonNodeKind
    returned: object
    expected_error_fragment: str


@dataclass(frozen=True)
class PythonNodeContextHelperTestCase:
    description: str
    raw_name: str
    database: str | None
    schema: str | None
    expected_qualified_name: str
    expected_execute_result: str
    expected_query_result: str
    expected_recorded_events: tuple[str, ...]
    expected_logger_name: str
    expected_run_id: str
    expected_target: str | None
    expected_vars: dict[str, object]
    expected_error_fragment: str = ""


@dataclass(frozen=True)
class PublicSkipModeExportTestCase:
    description: str
    expected_task_export: bool
    expected_asset_export: bool


@dataclass(frozen=True)
class PythonNodeSkipModeInputTestCase:
    description: str
    raw_mode: str
    expected_mode: SkipMode


@dataclass(frozen=True)
class PythonNodeRunStateTestCase:
    description: str
    expected_payload: object
    expected_metadata: dict[str, object]
    expected_default: object
    expected_error_fragment: str


@dataclass(frozen=True)
class PythonNodeExecutorTestCase:
    description: str
    expected_names: tuple[str, ...]
    expected_statuses: tuple[PythonNodeStatus, ...]
    expected_payloads: tuple[object | None, ...]
    expected_materialized: tuple[bool | None, ...]
    expected_error_fragments: tuple[str | None, ...]
    skip_function: Callable[..., object] | None = None


@dataclass(frozen=True)
class PythonNodeLifecycleNodeBuildTestCase:
    description: str
    python_graph_case: str
    selected_names: frozenset[str]
    expected_names: tuple[str, ...]
    expected_kinds: tuple[str, ...]
    expected_upstream_names: tuple[tuple[str, ...], ...]
    expected_payload_names: tuple[str, ...]


@dataclass(frozen=True)
class PythonIngressLoaderExecutorTestCase:
    description: str
    selected_names: frozenset[str]
    expected_python_names: tuple[str, ...]
    expected_load_names: tuple[str, ...]
    expected_python_statuses: tuple[PythonNodeStatus, ...]
    expected_load_statuses: tuple[str, ...]
    expected_call_order: tuple[str, ...]


@dataclass(frozen=True)
class IngressSchedulerSkipLifecycleTestCase:
    description: str
    expected_resource_ids: tuple[str, ...]
    expected_event_types: tuple[str, ...]


@dataclass(frozen=True)
class ReadSidePythonTrackerTestCase:
    description: str
    selected_names: frozenset[str]
    completed_sql_names: tuple[str, ...]
    expected_result_names: tuple[str, ...]
    expected_call_order: tuple[str, ...]
    expected_statuses: tuple[PythonNodeStatus, ...]
    expected_skip_reasons: tuple[str | None, ...] = ()
    failed_sql_names: tuple[str, ...] = ()
    scheduled_sql_names: frozenset[str] = frozenset({"stg_orders"})


@dataclass(frozen=True)
class PythonNodeRetryExecutorTestCase:
    description: str
    expected_status: PythonNodeStatus
    expected_payload: object | None
    expected_error_fragment: str | None
    expected_attempts: int
    expected_sleeps: tuple[float, ...]


@dataclass(frozen=True)
class MalformedPythonOperationTestCase:
    description: str
    operation_name: str
    expected_error_fragment: str


@dataclass(frozen=True)
class PythonPostCallLifecycleTestCase:
    description: str
    expected_event_types: tuple[str, ...]


@dataclass(frozen=True)
class PythonCheckLifecycleTestCase:
    description: str
    passed: bool
    severity: PythonCheckSeverity
    expected_event_types: tuple[str, ...]


@dataclass(frozen=True)
class PythonCheckReturnNormalizationTestCase:
    description: str
    returned: object
    default_severity: PythonCheckSeverity
    expected_passed: bool
    expected_message: str | None
    expected_metadata: dict[str, object]
    expected_severity: PythonCheckSeverity | None


@dataclass(frozen=True)
class PythonCheckReturnNormalizationErrorTestCase:
    description: str
    returned: object
    default_severity: PythonCheckSeverity
    expected_error_fragment: str


@dataclass(frozen=True)
class PythonCheckContextResultTestCase:
    description: str
    message: str | None
    metadata: dict[str, object]
    expected_passed: tuple[bool, bool, bool]
    expected_messages: tuple[str | None, str | None, str | None]
    expected_metadata: tuple[dict[str, object], dict[str, object], dict[str, object]]
    expected_severities: tuple[
        PythonCheckSeverity | None, PythonCheckSeverity | None, PythonCheckSeverity | None
    ]


@dataclass(frozen=True)
class PythonCheckExecutorTestCase:
    description: str
    expected_passed: bool
    expected_severity: PythonCheckSeverity
    expected_message: str | None
    upstream_skip_mode: SkipMode | None
    expected_error_fragment: str | None = None
    upstream_status: PythonNodeStatus = PythonNodeStatus.SUCCESS
    upstream_skip_reason: str | None = None


@dataclass(frozen=True)
class BlockedPythonCheckLifecycleTestCase:
    description: str
    upstream_status: PythonNodeStatus
    upstream_skip_mode: SkipMode | None
    expected_terminal: str
    expected_check_status: str
    expected_summary: dict[str, int]


@dataclass(frozen=True)
class PythonIdentityFingerprintWriteTestCase:
    description: str
    schema: str | None
    expected_sql_count: int
    expected_fragments: tuple[str, ...]
    unexpected_fragments: tuple[str, ...] = ()


@dataclass(frozen=True)
class IngressWarehouseFailureTestCase:
    """An ingress node whose job warehouse the session cannot use, and what the run records."""

    description: str
    missing_warehouse_node: str
    expected_python: tuple[tuple[str, PythonNodeStatus, str | None], ...]
    expected_loads: tuple[tuple[str, str, str | None], ...]
    expected_call_order: tuple[str, ...]


@dataclass(frozen=True)
class CheckWarehouseFailureTestCase:
    """A check on a job warehouse the session cannot use, after an upstream of some status."""

    description: str
    upstream_status: PythonNodeStatus
    expected_error_message: str | None
    expected_switch_attempts: int


@dataclass(frozen=True)
class TracebackTailTestCase:
    description: str
    depth: int
    expected_start: str
    expected_end: str
    expected_elided: bool


@dataclass(frozen=True)
class TracebackRedactionTestCase:
    description: str
    message: str
    expected_kept: str
    expected_absent: str


@dataclass(frozen=True)
class DiscoveredItemNodeTestCase:
    description: str
    discovered: dict[str, object] | None
    expected_args: dict[str, object] | None
    expected_item: str | None
    expected_failure: str | None


@dataclass(frozen=True)
class PythonNodeLiveLogTestCase:
    """An asset that logs, and what its runtime's `on_log` sees and its result counts."""

    description: str
    name: str
    function: Callable[..., object]
    expected_forwarded: tuple[tuple[str, str, str], ...]
    expected_forwarded_count: int
    expected_dropped: int
