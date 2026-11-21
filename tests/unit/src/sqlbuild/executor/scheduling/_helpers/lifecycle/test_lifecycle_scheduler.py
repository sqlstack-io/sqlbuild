"""Tests for the generic lifecycle scheduler."""

from __future__ import annotations

import pytest

from sqlbuild.compiler.python_nodes.types import SkipMode
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.scheduling.main._run_lifecycle import run_lifecycle_scheduler
from sqlbuild.executor.scheduling.models import (
    LifecycleExecutionNode,
    LifecycleNodeResult,
    LifecycleSchedulerResult,
)
from sqlbuild.executor.scheduling.types import LifecycleNodeStatus
from tests.unit.src.sqlbuild.executor.scheduling._helpers.lifecycle._test_types import (
    LifecycleSchedulerErrorTestCase,
    LifecycleSchedulerFanInTestCase,
    LifecycleSchedulerTestCase,
)
from tests.unit.src.sqlbuild.executor.scheduling._helpers.lifecycle.helpers import (
    lifecycle_soft_skip,
    lifecycle_success,
    record_lifecycle_hard_skip_a_else_success,
    record_lifecycle_maybe_failure,
    record_lifecycle_soft_skip_a_else_success,
    record_lifecycle_success,
)


@pytest.mark.parametrize(
    "test_case",
    [
        LifecycleSchedulerTestCase(
            description="runs mixed nodes in topological order",
            expected_order=("load_pages", "summarize_pages", "load_orders"),
            expected_statuses=(
                LifecycleNodeStatus.SUCCESS,
                LifecycleNodeStatus.SUCCESS,
                LifecycleNodeStatus.SUCCESS,
            ),
            expected_skip_reasons=(None, None, None),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_mixed_lifecycle_nodes_when_running_scheduler_then_runs_in_topological_order(
    test_case: LifecycleSchedulerTestCase,
) -> None:
    calls: list[str] = []
    nodes: tuple[LifecycleExecutionNode, ...] = (
        LifecycleExecutionNode(
            name="load_orders", kind="loader", upstream_names=("summarize_pages",)
        ),
        LifecycleExecutionNode(name="load_pages", kind="loader"),
        LifecycleExecutionNode(
            name="summarize_pages", kind="asset", upstream_names=("load_pages",)
        ),
    )

    result: LifecycleSchedulerResult = run_lifecycle_scheduler(
        nodes=nodes,
        handler=lambda node: record_lifecycle_success(node=node, calls=calls),
    )

    assert calls == list(test_case.expected_order)
    assert tuple(node_result.name for node_result in result.results) == test_case.expected_order
    assert (
        tuple(node_result.status for node_result in result.results) == test_case.expected_statuses
    )
    assert tuple(node_result.skip_reason for node_result in result.results) == (
        test_case.expected_skip_reasons
    )


@pytest.mark.parametrize(
    "test_case",
    [
        LifecycleSchedulerTestCase(
            description="skips downstream after failed upstream",
            expected_order=("load_pages", "summarize_pages", "load_orders"),
            expected_statuses=(
                LifecycleNodeStatus.SUCCESS,
                LifecycleNodeStatus.FAILED,
                LifecycleNodeStatus.SKIPPED,
            ),
            expected_skip_reasons=(None, None, "Upstream node did not succeed: summarize_pages"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_failed_lifecycle_node_when_running_scheduler_then_skips_downstream(
    test_case: LifecycleSchedulerTestCase,
) -> None:
    calls: list[str] = []
    nodes: tuple[LifecycleExecutionNode, ...] = (
        LifecycleExecutionNode(name="load_pages", kind="loader"),
        LifecycleExecutionNode(
            name="summarize_pages", kind="asset", upstream_names=("load_pages",)
        ),
        LifecycleExecutionNode(
            name="load_orders", kind="loader", upstream_names=("summarize_pages",)
        ),
    )

    result: LifecycleSchedulerResult = run_lifecycle_scheduler(
        nodes=nodes,
        handler=lambda node: record_lifecycle_maybe_failure(node=node, calls=calls),
    )

    assert calls == ["load_pages", "summarize_pages"]
    assert tuple(node_result.name for node_result in result.results) == test_case.expected_order
    assert (
        tuple(node_result.status for node_result in result.results) == test_case.expected_statuses
    )
    assert tuple(node_result.skip_reason for node_result in result.results) == (
        test_case.expected_skip_reasons
    )


@pytest.mark.parametrize(
    "test_case",
    [
        LifecycleSchedulerFanInTestCase(
            description="runs downstream with soft-skipped upstream and successful sibling",
            expected_calls=["A", "X", "C"],
            expected_statuses=(
                LifecycleNodeStatus.SKIPPED,
                LifecycleNodeStatus.SUCCESS,
                LifecycleNodeStatus.SUCCESS,
            ),
            expected_downstream_skip_reason=None,
            expected_downstream_skip_mode=None,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_soft_skip_and_successful_sibling_when_running_scheduler_then_runs_downstream(
    test_case: LifecycleSchedulerFanInTestCase,
) -> None:
    calls: list[str] = []
    nodes: tuple[LifecycleExecutionNode, ...] = (
        LifecycleExecutionNode(name="A", kind="asset"),
        LifecycleExecutionNode(name="X", kind="asset"),
        LifecycleExecutionNode(name="C", kind="asset", upstream_names=("A", "X")),
    )

    result: LifecycleSchedulerResult = run_lifecycle_scheduler(
        nodes=nodes,
        handler=lambda node: record_lifecycle_soft_skip_a_else_success(node=node, calls=calls),
    )

    assert calls == test_case.expected_calls
    assert (
        tuple(node_result.status for node_result in result.results) == test_case.expected_statuses
    )
    assert result.results[-1].skip_reason == test_case.expected_downstream_skip_reason
    assert result.results[-1].skip_mode == test_case.expected_downstream_skip_mode


@pytest.mark.parametrize(
    "test_case",
    [
        LifecycleSchedulerFanInTestCase(
            description="soft-skips downstream when all upstreams soft-skipped",
            expected_calls=["A", "B"],
            expected_statuses=(
                LifecycleNodeStatus.SKIPPED,
                LifecycleNodeStatus.SKIPPED,
                LifecycleNodeStatus.SKIPPED,
            ),
            expected_downstream_skip_reason="All upstream nodes were skipped",
            expected_downstream_skip_mode=SkipMode.SOFT,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_all_upstreams_soft_skipped_when_running_scheduler_then_soft_skips_downstream(
    test_case: LifecycleSchedulerFanInTestCase,
) -> None:
    calls: list[str] = []
    nodes: tuple[LifecycleExecutionNode, ...] = (
        LifecycleExecutionNode(name="A", kind="asset"),
        LifecycleExecutionNode(name="B", kind="asset"),
        LifecycleExecutionNode(name="C", kind="asset", upstream_names=("A", "B")),
    )

    def handler(node: LifecycleExecutionNode) -> LifecycleNodeResult:
        calls.append(node.name)
        return lifecycle_soft_skip(node)

    result: LifecycleSchedulerResult = run_lifecycle_scheduler(nodes=nodes, handler=handler)

    assert calls == test_case.expected_calls
    assert (
        tuple(node_result.status for node_result in result.results) == test_case.expected_statuses
    )
    assert result.results[-1].skip_reason == test_case.expected_downstream_skip_reason
    assert result.results[-1].skip_mode == test_case.expected_downstream_skip_mode


@pytest.mark.parametrize(
    "test_case",
    [
        LifecycleSchedulerFanInTestCase(
            description="skips downstream with hard-skipped upstream and successful sibling",
            expected_calls=["A", "X"],
            expected_statuses=(
                LifecycleNodeStatus.SKIPPED,
                LifecycleNodeStatus.SUCCESS,
                LifecycleNodeStatus.SKIPPED,
            ),
            expected_downstream_skip_reason="Upstream node hard-skipped: A",
            expected_downstream_skip_mode=SkipMode.HARD,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_hard_skip_and_successful_sibling_when_running_scheduler_then_skips_downstream(
    test_case: LifecycleSchedulerFanInTestCase,
) -> None:
    calls: list[str] = []
    nodes: tuple[LifecycleExecutionNode, ...] = (
        LifecycleExecutionNode(name="A", kind="asset"),
        LifecycleExecutionNode(name="X", kind="asset"),
        LifecycleExecutionNode(name="C", kind="asset", upstream_names=("A", "X")),
    )

    result: LifecycleSchedulerResult = run_lifecycle_scheduler(
        nodes=nodes,
        handler=lambda node: record_lifecycle_hard_skip_a_else_success(node=node, calls=calls),
    )

    assert calls == test_case.expected_calls
    assert (
        tuple(node_result.status for node_result in result.results) == test_case.expected_statuses
    )
    assert result.results[-1].skip_reason == test_case.expected_downstream_skip_reason
    assert result.results[-1].skip_mode == test_case.expected_downstream_skip_mode


@pytest.mark.parametrize(
    "test_case",
    [
        LifecycleSchedulerErrorTestCase(
            description="raises for unknown dependency",
            expected_error_fragment="depends on unknown node 'missing'",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unknown_dependency_when_running_scheduler_then_raises_input_error(
    test_case: LifecycleSchedulerErrorTestCase,
) -> None:
    nodes: tuple[LifecycleExecutionNode, ...] = (
        LifecycleExecutionNode(name="summarize_pages", kind="asset", upstream_names=("missing",)),
    )

    with pytest.raises(ExecutorInputError) as exc_info:
        run_lifecycle_scheduler(nodes=nodes, handler=lifecycle_success)

    assert test_case.expected_error_fragment in str(exc_info.value)
