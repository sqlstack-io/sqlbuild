"""Helpers for Python-node scheduler tests."""

from __future__ import annotations

import logging
import time
from collections.abc import Callable, Iterator
from dataclasses import replace
from datetime import UTC, datetime
from itertools import chain, repeat
from pathlib import Path
from typing import Any, ClassVar, cast

import pytest

from sqlbuild._native import (
    CostContext,
    DiscoveredAssetFunction,
    DiscoveredCheckFunction,
    DiscoveredLoaderFunction,
    DiscoveredProjectInputs,
    DiscoveredPythonNode,
    LocalConfig,
    ProjectConfig,
    PythonNodeGraph,
    PythonSqlRunLifecyclePlan,
    PythonSqlRunSelection,
    SourceEntry,
    build_python_node_graph,
    build_python_sql_run_lifecycle,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.assets import AssetContext, asset
from sqlbuild.checks import CheckContext
from sqlbuild.compiler.python_nodes.types import SkipMode
from sqlbuild.cost.models import CostResourceContext
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.node_results.main._direct_store import build_direct_node_result_store
from sqlbuild.executor.node_results.models import NodeResultEnvelope, NodeResultRecord
from sqlbuild.executor.python_nodes._helpers.execution import execute_ready_python_node
from sqlbuild.executor.python_nodes.constants import MISSING_DEFAULT
from sqlbuild.executor.python_nodes.models import (
    BasePythonNodeContext,
    PythonNodeExecutionResult,
    PythonNodeRunState,
    PythonNodeRuntime,
)
from sqlbuild.executor.python_nodes.types import ExecutablePythonNode
from sqlbuild.executor.scheduling.classes.session_warehouses import SessionWarehouses
from sqlbuild.executor.scheduling.models import LifecycleExecutionNode
from sqlbuild.observability import LifecycleEvent
from sqlbuild.providers import Provider
from sqlbuild.python_nodes.types import PythonCheckSeverity
from sqlbuild.refs import model
from tests.unit.src.sqlbuild.compiler.python_nodes._helpers.helpers import (
    build_intermediate_loader_asset_dependency_python_node_graph,
    build_orders_python_node_graph,
)


def execute_ordered_test_nodes(
    *,
    nodes: tuple[ExecutablePythonNode, ...],
    runtime: PythonNodeRuntime,
    statement_recorder: StatementRecorder,
    sleep: Callable[[float], None] = time.sleep,
    monotonic: Callable[[], float] = time.monotonic,
) -> tuple[PythonNodeExecutionResult, ...]:
    """Exercise the live ready-node entry point in fixture order, without a second scheduler."""

    run_state: PythonNodeRunState = PythonNodeRunState()
    runtime = replace(
        runtime,
        result_store=build_direct_node_result_store(
            adapter=runtime.adapter,
            connection=runtime.connection,
            database=runtime.default_database,
            schema=runtime.default_schema,
        ),
    )
    results: dict[object, PythonNodeExecutionResult] = {}
    node: ExecutablePythonNode
    for node in nodes:
        result: PythonNodeExecutionResult = execute_ready_python_node(
            node=node,
            upstream_results=tuple(results[dependency] for dependency in node.depends_on),
            runtime=runtime,
            statement_recorder=statement_recorder,
            run_state=run_state,
            sleep=sleep,
            monotonic=monotonic,
        )
        run_state.record_result(result=result)
        results[node.function] = result
    return tuple(results.values())


class ExecutionSlackProvider(Provider):
    provider_name: ClassVar[str] = "slack_provider"
    label: str = "slack"


class PythonNodeContextTestAdapter(BaseAdapter):
    """Adapter that records SQL and returns deterministic values."""

    adapter_name: ClassVar[str] = "python-node-context-test"

    def __init__(self) -> None:
        self.executed_sql: list[str] = []

    def connect(self, config: dict[str, object]) -> object:
        del config
        return object()

    def close(self, connection: object) -> None:
        del connection

    def _execute(self, connection: Any, sql: str) -> object:
        del connection
        self.executed_sql.append(sql)
        return f"result:{sql}"


class PythonNodeContextTestResultStore:
    """In-memory stand-in for persisted node result reads."""

    def __init__(self, results: dict[tuple[str, str], tuple[NodeResultEnvelope, ...]]) -> None:
        self._results: dict[tuple[str, str], tuple[NodeResultEnvelope, ...]] = results
        self.written_records: list[NodeResultRecord] = []
        self.connection: object = object()
        self.database: str | None = None
        self.schema: str | None = "default_schema"

    def write(self, record: NodeResultRecord) -> None:
        self.written_records.append(record)

    def result_of(
        self,
        *,
        node_type: str,
        node_name: str,
        run_id: str | None = None,
        default: object,
    ) -> NodeResultEnvelope | object:
        del run_id
        results: tuple[NodeResultEnvelope, ...] = self._results.get((node_type, node_name), ())
        strategy: Callable[..., object] = _RESULT_READ_STRATEGIES[
            (bool(results), default is MISSING_DEFAULT)
        ]
        return strategy(results=results, default=default, node_name=node_name)

    def results_of(
        self,
        *,
        node_type: str,
        node_name: str,
        limit: int,
    ) -> tuple[NodeResultEnvelope, ...]:
        return self._results.get((node_type, node_name), ())[:limit]


def provider_task(ctx: AssetContext, slack_provider: ExecutionSlackProvider) -> dict[str, object]:
    return {"target": ctx.target, "provider": slack_provider.label}


def provider_asset(ctx: AssetContext, slack_provider: ExecutionSlackProvider) -> dict[str, object]:
    return {"target": ctx.target, "provider": slack_provider.label}


def provider_check(ctx: CheckContext, slack_provider: ExecutionSlackProvider) -> bool:
    return ctx.target == "dev" and slack_provider.label == "slack"


def context_provider_task(ctx: AssetContext) -> dict[str, object]:
    return {
        "attr": cast(ExecutionSlackProvider, ctx.providers.slack_provider).label,
        "item": cast(ExecutionSlackProvider, ctx.providers["slack_provider"]).label,
    }


def context_provider_asset(ctx: AssetContext) -> dict[str, object]:
    return {
        "attr": cast(ExecutionSlackProvider, ctx.providers.slack_provider).label,
        "item": cast(ExecutionSlackProvider, ctx.providers["slack_provider"]).label,
    }


def context_provider_check(ctx: CheckContext) -> bool:
    return (
        cast(ExecutionSlackProvider, ctx.providers.slack_provider).label == "slack"
        and cast(ExecutionSlackProvider, ctx.providers["slack_provider"]).label == "slack"
    )


def missing_context_provider_task(ctx: AssetContext) -> object:
    return ctx.providers["slack_provider"]


def build_task_context(
    *,
    adapter: PythonNodeContextTestAdapter,
    statement_recorder: StatementRecorder,
    logger_name: str,
    run_state: PythonNodeRunState | None = None,
    result_store: object | None = None,
) -> AssetContext:
    return AssetContext(
        adapter=adapter,
        connection_config={"warehouse": "dev"},
        connection=object(),
        run_id="test_run",
        target="dev",
        vars={"batch": "hourly"},
        is_reload=False,
        logger=logging.getLogger(logger_name),
        statement_recorder=statement_recorder,
        run_state=run_state,
        result_store=result_store,
        default_database="default_db",
        default_schema="default_schema",
    )


def build_asset_context(
    *,
    adapter: PythonNodeContextTestAdapter,
    statement_recorder: StatementRecorder,
    logger_name: str,
    run_state: PythonNodeRunState | None = None,
    result_store: object | None = None,
) -> AssetContext:
    return AssetContext(
        adapter=adapter,
        connection_config={"warehouse": "dev"},
        connection=object(),
        run_id="test_run",
        target="dev",
        vars={"batch": "hourly"},
        is_reload=False,
        logger=logging.getLogger(logger_name),
        statement_recorder=statement_recorder,
        run_state=run_state,
        result_store=result_store,
        default_database="default_db",
        default_schema="default_schema",
    )


def build_check_context(
    *,
    adapter: PythonNodeContextTestAdapter,
    statement_recorder: StatementRecorder,
    logger_name: str,
    run_state: PythonNodeRunState | None = None,
    result_store: object | None = None,
) -> CheckContext:
    return CheckContext(
        adapter=adapter,
        connection_config={"warehouse": "dev"},
        connection=object(),
        run_id="test_run",
        target="dev",
        vars={"batch": "hourly"},
        is_reload=False,
        logger=logging.getLogger(logger_name),
        statement_recorder=statement_recorder,
        run_state=run_state,
        result_store=result_store,
        default_database="default_db",
        default_schema="default_schema",
    )


@asset
def upstream_task(_ctx: object) -> object:
    return None


@asset
def skipped_upstream_task(_ctx: object) -> object:
    return None


@asset
def fetch_orders(ctx: AssetContext) -> object:
    return ctx.result(
        payload={"file": "orders.json"},
        metadata={"row_count": 3},
    )


@asset(depends_on=(fetch_orders,))
def export_orders(ctx: AssetContext) -> object:
    upstream_result: NodeResultEnvelope = cast(
        NodeResultEnvelope, ctx.result_of(node_function=fetch_orders)
    )
    payload_dict: dict[str, object] = cast(dict[str, object], upstream_result.payload)
    metadata_dict: dict[str, object] = upstream_result.metadata
    file_name: str = cast(str, payload_dict["file"])
    return ctx.result(
        payload={"uri": f"s3://exports/{file_name}"},
        metadata=metadata_dict,
        materialized=True,
    )


def skip_empty_orders(ctx: AssetContext) -> object:
    return ctx.skip(reason="No new orders")


def hard_skip_empty_orders(ctx: AssetContext) -> object:
    return ctx.skip(reason="No new orders", mode=SkipMode.HARD)


def export_after_skip(ctx: AssetContext) -> object:
    return ctx.result(payload={"uri": "should-not-run"}, materialized=True)


def successful_sibling(ctx: AssetContext) -> object:
    return ctx.result(payload={"status": "ready"})


def export_after_mixed_skip(ctx: AssetContext) -> object:
    return ctx.result(payload={"uri": "s3://exports/orders.json"}, materialized=True)


def fail_orders(_ctx: AssetContext) -> object:
    raise RuntimeError("API unavailable")


def cursor_window(ctx: AssetContext) -> object:
    return ctx.result(
        payload={
            "start_cursor_ts": ctx.start_cursor_ts,
            "end_cursor_ts": ctx.end_cursor_ts,
            "start_cursor_int": ctx.start_cursor_int,
            "end_cursor_int": ctx.end_cursor_int,
        }
    )


def export_after_failure(ctx: AssetContext) -> object:
    return ctx.result(payload={"uri": "should-not-run"}, materialized=True)


EXPECTED_START_CURSOR_TS: datetime = datetime(2026, 1, 1, tzinfo=UTC)
EXPECTED_END_CURSOR_TS: datetime = datetime(2026, 1, 2, tzinfo=UTC)


def python_operation_events(events: list[LifecycleEvent]) -> tuple[LifecycleEvent, ...]:
    return tuple(filter(lambda event: event.event_type.startswith("operation_"), events))


class FlakyTask:
    def __init__(self, failures_before_success: int, exception_type: type[Exception]) -> None:
        self.failures_before_success: int = failures_before_success
        self.exception_type: type[Exception] = exception_type
        self.attempts: int = 0
        self.cost_contexts: list[CostResourceContext | None] = []
        failures: Iterator[Callable[[AssetContext], object]] = (
            self._raise_transient_failure for _attempt in range(self.failures_before_success)
        )
        self._strategies = iter(chain(failures, repeat(self._return_success)))

    def __call__(self, ctx: AssetContext) -> object:
        self.attempts += 1
        cost_context: CostResourceContext | None = CostContext.current()
        self.cost_contexts.append(cost_context)
        return next(self._strategies)(ctx)

    def _raise_transient_failure(self, ctx: AssetContext) -> object:
        del ctx
        raise self.exception_type("transient failure")

    def _return_success(self, ctx: AssetContext) -> object:
        return ctx.result(payload={"attempts": self.attempts})


def loader_only_attribute_names() -> tuple[str, ...]:
    return (
        "destination",
        "destination_database",
        "destination_schema",
        "destination_name",
        "current_cursor_value",
        "loader",
        "source",
        "qualify_in_destination_schema",
    )


def assert_base_context_fields(context: BasePythonNodeContext) -> None:
    assert context.run_id == "test_run"
    assert context.target == "dev"
    assert context.vars == {"batch": "hourly"}
    assert context.is_reload is False
    assert context.connection_config == {"warehouse": "dev"}


def build_lifecycle_plan_for_selected_python_names(
    *, graph: PythonNodeGraph, selected_names: frozenset[str]
) -> PythonSqlRunLifecyclePlan:
    return build_python_sql_run_lifecycle(
        selection=PythonSqlRunSelection(sql_keys=frozenset(), python_node_names=selected_names),
        python_graph=graph,
    )


def lifecycle_node_payload_name(node: LifecycleExecutionNode) -> str:
    payload: object | None = node.payload
    assert isinstance(payload, DiscoveredPythonNode)
    return payload.name


def python_graph_for_lifecycle_case(case_name: str) -> PythonNodeGraph:
    return {
        "orders": build_orders_python_node_graph,
        "intermediate_loader_asset_dependency": (
            build_intermediate_loader_asset_dependency_python_node_graph
        ),
    }[case_name]()


INGRESS_CALLS: list[str] = []


def reset_ingress_calls() -> None:
    INGRESS_CALLS.clear()


def ingress_calls() -> tuple[str, ...]:
    return tuple(INGRESS_CALLS)


@asset
def prepare_ingress_orders(ctx: AssetContext) -> object:
    INGRESS_CALLS.append("prepare_ingress_orders")
    return ctx.result(payload={"prepared": True})


@asset(depends_on=(prepare_ingress_orders,))
def publish_ingress_orders(ctx: AssetContext) -> object:
    INGRESS_CALLS.append("publish_ingress_orders")
    return ctx.result(payload={"published": True}, materialized=True)


def load_ingress_orders(_ctx: object) -> object:
    INGRESS_CALLS.append("load_ingress_orders")
    return None


def scheduler_bypassed_task() -> object:
    raise AssertionError("scheduler-bypassed task must not run")


def build_ingress_task_loader_graph() -> PythonNodeGraph:
    return build_python_node_graph(
        discovered_inputs=DiscoveredProjectInputs(
            project_config=ProjectConfig(name="demo", adapter="duckdb"),
            local_config=LocalConfig(),
            loader_functions=(ingress_loader_function(),),
            asset_functions=(ingress_task_function(),),
        )
    )


def build_ingress_task_asset_graph() -> PythonNodeGraph:
    return build_python_node_graph(
        discovered_inputs=DiscoveredProjectInputs(
            project_config=ProjectConfig(name="demo", adapter="duckdb"),
            local_config=LocalConfig(),
            asset_functions=(
                ingress_task_function(),
                DiscoveredAssetFunction(
                    file_path=Path("/project/assets/orders.py"),
                    relative_path=Path("python/nodes/assets/orders.py"),
                    name="publish_ingress_orders",
                    function=publish_ingress_orders,
                    depends_on=(prepare_ingress_orders,),
                ),
            ),
        )
    )


def ingress_task_function() -> DiscoveredAssetFunction:
    return DiscoveredAssetFunction(
        file_path=Path("/project/tasks/orders.py"),
        relative_path=Path("python/nodes/tasks/orders.py"),
        name="prepare_ingress_orders",
        function=prepare_ingress_orders,
    )


def ingress_loader_function() -> DiscoveredLoaderFunction:
    return DiscoveredLoaderFunction(
        file_path=Path("/project/loaders/orders.py"),
        relative_path=Path("python/nodes/loaders/orders.py"),
        name="load_ingress_orders",
        function=load_ingress_orders,
        depends_on=(prepare_ingress_orders,),
    )


def ingress_source_map() -> dict[str, SourceEntry]:
    return {
        "raw_orders": SourceEntry(
            name="raw_orders",
            loader="load_ingress_orders",
        )
    }


READ_SIDE_CALLS: list[str] = []


def reset_read_side_calls() -> None:
    READ_SIDE_CALLS.clear()


def read_side_calls() -> tuple[str, ...]:
    return tuple(READ_SIDE_CALLS)


@asset(depends_on=(model("stg_orders"),))
def profile_stg_orders(ctx: AssetContext) -> object:
    READ_SIDE_CALLS.append("profile_stg_orders")
    return ctx.result(payload={"profiled": True})


@asset(depends_on=(profile_stg_orders,))
def export_stg_profile(ctx: AssetContext) -> object:
    READ_SIDE_CALLS.append("export_stg_profile")
    return ctx.result(
        payload=cast(NodeResultEnvelope, ctx.result_of(node_function=profile_stg_orders)).payload,
        materialized=False,
    )


def build_read_side_sql_task_asset_graph() -> PythonNodeGraph:
    return build_python_node_graph(
        discovered_inputs=DiscoveredProjectInputs(
            project_config=ProjectConfig(name="demo", adapter="duckdb"),
            local_config=LocalConfig(),
            asset_functions=(
                DiscoveredAssetFunction(
                    file_path=Path("/project/tasks/orders.py"),
                    relative_path=Path("python/nodes/tasks/orders.py"),
                    name="profile_stg_orders",
                    function=profile_stg_orders,
                    depends_on=(model("stg_orders"),),
                ),
                DiscoveredAssetFunction(
                    file_path=Path("/project/assets/orders.py"),
                    relative_path=Path("python/nodes/assets/orders.py"),
                    name="export_stg_profile",
                    function=export_stg_profile,
                    depends_on=(profile_stg_orders,),
                ),
            ),
        )
    )


@asset(name="upstream_task")
def check_upstream_task(_ctx: object) -> object:
    return {"rows": 3}


def passing_python_check(ctx: CheckContext) -> object:
    metadata: dict[str, object] = cast(
        NodeResultEnvelope, ctx.result_of(node_function=check_upstream_task)
    ).metadata
    return ctx.pass_(message="passed", metadata={"rows": metadata["rows"]})


def warning_python_check(ctx: CheckContext) -> object:
    return ctx.warn(message="warned")


def false_python_check(_ctx: CheckContext) -> object:
    return False


def exception_python_check(_ctx: CheckContext) -> object:
    raise RuntimeError("check exploded")


def python_check_function_for_case(description: str) -> DiscoveredCheckFunction:
    function: Callable[..., object] = {
        "preserves explicit warning result": warning_python_check,
        "normalizes false result as error failure": false_python_check,
        "normalizes check exception as error failure": exception_python_check,
    }.get(description, passing_python_check)
    return DiscoveredCheckFunction(
        file_path=Path("/project/checks/orders.py"),
        relative_path=Path("python/nodes/checks/orders.py"),
        name="check_upstream_task",
        function=function,
        depends_on=(check_upstream_task,),
        severity=PythonCheckSeverity.ERROR,
    )


def build_python_check_graph(*, check_function: DiscoveredCheckFunction) -> PythonNodeGraph:
    return build_python_node_graph(
        discovered_inputs=DiscoveredProjectInputs(
            project_config=ProjectConfig(name="demo", adapter="duckdb"),
            local_config=LocalConfig(),
            asset_functions=(
                DiscoveredAssetFunction(
                    file_path=Path("/project/tasks/orders.py"),
                    relative_path=Path("python/nodes/tasks/orders.py"),
                    name="upstream_task",
                    function=check_upstream_task,
                ),
            ),
            check_functions=(check_function,),
        )
    )


def _read_existing_result(
    *, results: tuple[NodeResultEnvelope, ...], default: object, node_name: str
) -> NodeResultEnvelope:
    del default, node_name
    return results[0]


def _read_default_result(
    *, results: tuple[NodeResultEnvelope, ...], default: object, node_name: str
) -> object:
    del results, node_name
    return default


def _raise_missing_result(
    *, results: tuple[NodeResultEnvelope, ...], default: object, node_name: str
) -> object:
    del results, default
    raise ExecutorInputError(f"No persisted result found for Python node '{node_name}'")


_RESULT_READ_STRATEGIES: dict[tuple[bool, bool], Callable[..., object]] = {
    (True, True): _read_existing_result,
    (True, False): _read_existing_result,
    (False, False): _read_default_result,
    (False, True): _raise_missing_result,
}


MISSING_WAREHOUSE_FAILURE: str = (
    "cannot run on warehouse MISSING_WH: Object does not exist: warehouse MISSING_WH"
)


def missing_warehouse_runtime(*, adapter: Any, node: str) -> PythonNodeRuntime:
    """A runtime putting `node` on MISSING_WH through `adapter`'s session warehouses."""

    return PythonNodeRuntime(
        adapter=PythonNodeContextTestAdapter(),
        connection_config={},
        connection=object(),
        run_id="warehouse-run",
        target="dev",
        vars={},
        is_reload=False,
        node_warehouses={node: "MISSING_WH"},
        session_warehouses=SessionWarehouses(adapter=cast(BaseAdapter, adapter), home="BUILD_WH"),
    )


def nested_key_error(*, depth: int) -> BaseException:
    """Return the KeyError a lookup raises through `depth` distinct calls, with its traceback."""

    source: str = "def step_0():\n    return {'customer_id': 1}['customer_segment']\n" + "".join(
        f"def step_{n}():\n    return step_{n - 1}()\n" for n in range(1, depth + 1)
    )
    steps: dict[str, Any] = {}
    exec(compile(source, "steps.py", "exec"), steps)  # noqa: S102
    with pytest.raises(KeyError) as raised:
        steps[f"step_{depth}"]()
    return raised.value


def raised_value_error(*, message: str) -> BaseException:
    """Return a ValueError with `message`, raised so it has a traceback."""

    with pytest.raises(ValueError) as raised:
        raise ValueError(message)
    return raised.value


def discovered_step(ctx: object, n: int = 1) -> None:
    del ctx, n


@asset
def log_two_order_steps(ctx: AssetContext) -> object:
    """Log a step at info and one at warning."""

    ctx.log.info("loaded 3 orders")
    ctx.log.warning("1 order had no customer")
    return ctx.result(payload={"orders": 3})


@asset
def log_past_the_cap(ctx: AssetContext) -> object:
    """Log five lines more than a node keeps."""

    for line in range(205):
        ctx.log.info(f"order {line}")
    return ctx.result(payload={"orders": 205})


def logging_runtime(on_log: Callable[[str, Any, str], None]) -> PythonNodeRuntime:
    """A runtime whose `on_log` receives each `ctx.log` line as it is logged."""

    adapter: PythonNodeContextTestAdapter = PythonNodeContextTestAdapter()
    return PythonNodeRuntime(
        adapter=adapter,
        connection_config={},
        connection=object(),
        run_id="log_run",
        target="dev",
        vars={},
        is_reload=False,
        persist_node_results=False,
        on_log=on_log,
    )
