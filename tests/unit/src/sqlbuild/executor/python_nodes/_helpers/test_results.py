"""Tests for Python-node result normalization and fan-in policies."""

from __future__ import annotations

import pytest

from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.compiler.python_nodes.types import (
    PythonNodeFanInAction,
    PythonNodeKind,
    PythonNodeStatus,
    SkipMode,
)
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.python_nodes._helpers.results import (
    build_python_node_failure_result,
    evaluate_python_node_fan_in,
    normalize_python_check_return,
    normalize_python_node_return,
    traceback_tail,
)
from sqlbuild.executor.python_nodes.models import (
    CheckContext,
    PythonCheckResult,
    PythonNodeExecutionResult,
    PythonNodeFanInDecision,
    PythonNodeResult,
    PythonNodeSkipResult,
)
from sqlbuild.python_nodes.types import PythonCheckSeverity
from tests.unit.src.sqlbuild.executor.python_nodes._helpers._test_types import (
    PythonCheckContextResultTestCase,
    PythonCheckReturnNormalizationErrorTestCase,
    PythonCheckReturnNormalizationTestCase,
    PythonNodeFailureResultTestCase,
    PythonNodeFanInPolicyTestCase,
    PythonNodeReturnNormalizationTestCase,
    TracebackRedactionTestCase,
    TracebackTailTestCase,
)
from tests.unit.src.sqlbuild.executor.python_nodes._helpers.helpers import (
    PythonNodeContextTestAdapter,
    build_check_context,
    nested_key_error,
    raised_value_error,
)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonCheckReturnNormalizationTestCase(
            description="preserves explicit check result metadata",
            returned=PythonCheckResult(
                passed=False,
                message="Export file is missing",
                metadata={"uri": "s3://exports/customers.parquet"},
                severity=PythonCheckSeverity.WARN,
            ),
            default_severity=PythonCheckSeverity.ERROR,
            expected_passed=False,
            expected_message="Export file is missing",
            expected_metadata={"uri": "s3://exports/customers.parquet"},
            expected_severity=PythonCheckSeverity.WARN,
        ),
        PythonCheckReturnNormalizationTestCase(
            description="normalizes true shorthand as pass",
            returned=True,
            default_severity=PythonCheckSeverity.ERROR,
            expected_passed=True,
            expected_message=None,
            expected_metadata={},
            expected_severity=PythonCheckSeverity.ERROR,
        ),
        PythonCheckReturnNormalizationTestCase(
            description="normalizes false shorthand as failure",
            returned=False,
            default_severity=PythonCheckSeverity.WARN,
            expected_passed=False,
            expected_message=None,
            expected_metadata={},
            expected_severity=PythonCheckSeverity.WARN,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_check_return_when_normalizing_then_returns_check_result(
    test_case: PythonCheckReturnNormalizationTestCase,
) -> None:
    result: PythonCheckResult = normalize_python_check_return(
        returned=test_case.returned,
        default_severity=test_case.default_severity,
    )

    assert result.passed == test_case.expected_passed
    assert result.message == test_case.expected_message
    assert result.metadata == test_case.expected_metadata
    assert result.severity == test_case.expected_severity


@pytest.mark.parametrize(
    "test_case",
    [
        PythonCheckReturnNormalizationErrorTestCase(
            description="rejects none check return",
            returned=None,
            default_severity=PythonCheckSeverity.ERROR,
            expected_error_fragment="Python checks must return",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_python_check_return_when_normalizing_then_raises(
    test_case: PythonCheckReturnNormalizationErrorTestCase,
) -> None:
    with pytest.raises(ExecutorInputError, match=test_case.expected_error_fragment):
        normalize_python_check_return(
            returned=test_case.returned,
            default_severity=test_case.default_severity,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        PythonCheckContextResultTestCase(
            description="builds pass fail and warn check results",
            message="Export row count below threshold",
            metadata={"row_count": 3, "threshold": 10},
            expected_passed=(True, False, False),
            expected_messages=(
                "Export row count below threshold",
                "Export row count below threshold",
                "Export row count below threshold",
            ),
            expected_metadata=(
                {"row_count": 3, "threshold": 10},
                {"row_count": 3, "threshold": 10},
                {"row_count": 3, "threshold": 10},
            ),
            expected_severities=(None, None, PythonCheckSeverity.WARN),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_check_context_when_building_results_then_returns_check_results(
    test_case: PythonCheckContextResultTestCase,
) -> None:
    context: CheckContext = build_check_context(
        adapter=PythonNodeContextTestAdapter(),
        statement_recorder=StatementRecorder(),
        logger_name="sqlbuild.check.export_customers_exists",
    )

    pass_result: PythonCheckResult = context.pass_(
        message=test_case.message, metadata=test_case.metadata
    )
    fail_result: PythonCheckResult = context.fail(
        message=test_case.message or "failed",
        metadata=test_case.metadata,
    )
    warn_result: PythonCheckResult = context.warn(
        message=test_case.message or "warned",
        metadata=test_case.metadata,
    )

    assert (pass_result.passed, fail_result.passed, warn_result.passed) == (
        test_case.expected_passed
    )
    assert (pass_result.message, fail_result.message, warn_result.message) == (
        test_case.expected_messages
    )
    assert (pass_result.metadata, fail_result.metadata, warn_result.metadata) == (
        test_case.expected_metadata
    )
    assert (pass_result.severity, fail_result.severity, warn_result.severity) == (
        test_case.expected_severities
    )


@pytest.mark.parametrize(
    "test_case",
    [
        PythonNodeReturnNormalizationTestCase(
            description="normalizes plain return as successful payload",
            kind=PythonNodeKind.ASSET,
            returned={"rows": 3},
            expected_status=PythonNodeStatus.SUCCESS,
            expected_payload={"rows": 3},
            expected_metadata={},
            expected_materialized=None,
            expected_skip_mode=None,
            expected_skip_reason=None,
        ),
        PythonNodeReturnNormalizationTestCase(
            description="normalizes none return as empty success",
            kind=PythonNodeKind.ASSET,
            returned=None,
            expected_status=PythonNodeStatus.SUCCESS,
            expected_payload=None,
            expected_metadata={},
            expected_materialized=None,
            expected_skip_mode=None,
            expected_skip_reason=None,
        ),
        PythonNodeReturnNormalizationTestCase(
            description="normalizes explicit asset result with materialized flag",
            kind=PythonNodeKind.ASSET,
            returned=PythonNodeResult(
                payload={"uri": "s3://exports/customers.parquet"},
                metadata={"format": "parquet"},
                materialized=False,
            ),
            expected_status=PythonNodeStatus.SUCCESS,
            expected_payload={"uri": "s3://exports/customers.parquet"},
            expected_metadata={"format": "parquet"},
            expected_materialized=False,
            expected_skip_mode=None,
            expected_skip_reason=None,
        ),
        PythonNodeReturnNormalizationTestCase(
            description="normalizes downstream skip signal",
            kind=PythonNodeKind.ASSET,
            returned=PythonNodeSkipResult(
                reason="No new source rows",
                mode=SkipMode.HARD,
                metadata={"cursor": "2026-05-30"},
            ),
            expected_status=PythonNodeStatus.SKIPPED,
            expected_payload=None,
            expected_metadata={"cursor": "2026-05-30"},
            expected_materialized=None,
            expected_skip_mode=SkipMode.HARD,
            expected_skip_reason="No new source rows",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_node_return_when_normalizing_then_returns_execution_result(
    test_case: PythonNodeReturnNormalizationTestCase,
) -> None:
    result: PythonNodeExecutionResult = normalize_python_node_return(
        node_name="fetch_orders",
        kind=test_case.kind,
        returned=test_case.returned,
    )

    assert result.node_name == "fetch_orders"
    assert result.kind == test_case.kind
    assert result.status == test_case.expected_status
    assert result.payload == test_case.expected_payload
    assert result.metadata == test_case.expected_metadata
    assert result.materialized == test_case.expected_materialized
    assert result.skip_mode == test_case.expected_skip_mode
    assert result.skip_reason == test_case.expected_skip_reason


@pytest.mark.parametrize(
    "test_case",
    [
        PythonNodeFailureResultTestCase(
            description="records failed status and error message",
            error=RuntimeError("API unavailable"),
            expected_status=PythonNodeStatus.FAILED,
            expected_error_message="API unavailable",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_node_exception_when_building_failure_result_then_records_message(
    test_case: PythonNodeFailureResultTestCase,
) -> None:
    result: PythonNodeExecutionResult = build_python_node_failure_result(
        node_name="fetch_orders",
        kind=PythonNodeKind.ASSET,
        error=test_case.error,
    )

    assert result.status == test_case.expected_status
    assert result.error_message == test_case.expected_error_message


@pytest.mark.parametrize(
    "test_case",
    [
        PythonNodeFanInPolicyTestCase(
            description="runs when there are no upstream nodes",
            upstream_statuses=(),
            upstream_skip_modes=(),
            expected_action=PythonNodeFanInAction.RUN,
            expected_reason=None,
        ),
        PythonNodeFanInPolicyTestCase(
            description="blocks when any upstream failed",
            upstream_statuses=(PythonNodeStatus.SUCCESS, PythonNodeStatus.FAILED),
            upstream_skip_modes=(None, None),
            expected_action=PythonNodeFanInAction.BLOCK,
            expected_reason="Upstream Python node failed: upstream_1",
        ),
        PythonNodeFanInPolicyTestCase(
            description="skips when any upstream hard-skipped",
            upstream_statuses=(PythonNodeStatus.SUCCESS, PythonNodeStatus.SKIPPED),
            upstream_skip_modes=(None, SkipMode.HARD),
            expected_action=PythonNodeFanInAction.SKIP,
            expected_reason="Upstream Python node hard-skipped: upstream_1",
        ),
        PythonNodeFanInPolicyTestCase(
            description="runs with successful sibling when another upstream soft-skipped",
            upstream_statuses=(PythonNodeStatus.SUCCESS, PythonNodeStatus.SKIPPED),
            upstream_skip_modes=(None, SkipMode.SOFT),
            expected_action=PythonNodeFanInAction.RUN,
            expected_reason=None,
        ),
        PythonNodeFanInPolicyTestCase(
            description="skips when all upstream nodes soft-skipped",
            upstream_statuses=(PythonNodeStatus.SKIPPED, PythonNodeStatus.SKIPPED),
            upstream_skip_modes=(SkipMode.SOFT, SkipMode.SOFT),
            expected_action=PythonNodeFanInAction.SKIP,
            expected_reason="All upstream Python nodes were skipped",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_upstream_python_node_results_when_evaluating_fan_in_then_returns_decision(
    test_case: PythonNodeFanInPolicyTestCase,
) -> None:
    upstream_results: tuple[PythonNodeExecutionResult, ...] = tuple(
        PythonNodeExecutionResult(
            node_name=f"upstream_{index}",
            kind=PythonNodeKind.ASSET,
            status=status,
            skip_mode=test_case.upstream_skip_modes[index],
        )
        for index, status in enumerate(test_case.upstream_statuses)
    )

    decision: PythonNodeFanInDecision = evaluate_python_node_fan_in(
        upstream_results=upstream_results,
    )

    assert decision.action == test_case.expected_action
    assert decision.reason == test_case.expected_reason


@pytest.mark.parametrize(
    "test_case",
    [
        TracebackTailTestCase(
            description="a short traceback is kept whole",
            depth=1,
            expected_start="Traceback (most recent call last):\n  File ",
            expected_end="KeyError: 'customer_segment'\n",
            expected_elided=False,
        ),
        TracebackTailTestCase(
            description="a long traceback keeps its header, then its end with the exception",
            depth=80,
            expected_start="Traceback (most recent call last):\n  ... ",
            expected_end="KeyError: 'customer_segment'\n",
            expected_elided=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_python_failure_when_its_traceback_is_kept_then_its_end_fits_the_run_error(
    test_case: TracebackTailTestCase,
) -> None:
    error: BaseException = nested_key_error(depth=test_case.depth)
    result: PythonNodeExecutionResult = build_python_node_failure_result(
        node_name="score_orders", kind=PythonNodeKind.ASSET, error=error
    )
    detail: str = result.error_detail or ""
    assert (
        detail.startswith(test_case.expected_start),
        detail.endswith(test_case.expected_end),
        "earlier lines" in detail,
        len(detail) <= 1_900,
        detail == traceback_tail(error=error),
        result.error_message,
    ) == (True, True, test_case.expected_elided, True, True, "'customer_segment'"), (
        test_case.description,
        detail,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        TracebackRedactionTestCase(
            description="a secret whose key is on a line the cut drops",
            message="password=" + "\n" * 2_000 + "example-value",
            expected_kept="ValueError: password=",
            expected_absent="example-value",
        ),
        TracebackRedactionTestCase(
            description="a secret whose key is cut from one long line",
            message="refused "
            + "x" * 100
            + " password="
            + "y" * 400
            + "example-value"
            + " z" * 900,
            expected_kept="ValueError: refused",
            expected_absent="example-value",
        ),
        TracebackRedactionTestCase(
            description="a message of many lines keeps the line naming the exception",
            message="order rows refused" + "\n  refused row" * 400,
            expected_kept="ValueError: order rows refused",
            expected_absent="password",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_long_error_when_its_traceback_is_cut_then_it_is_redacted_first_and_keeps_the_exception(
    test_case: TracebackRedactionTestCase,
) -> None:
    detail: str = traceback_tail(error=raised_value_error(message=test_case.message))

    assert (
        test_case.expected_kept in detail,
        test_case.expected_absent not in detail,
        len(detail) <= 1_900,
    ) == (True, True, True), (test_case.description, detail)
