"""Result normalization and fan-in policy helpers for Python DAG nodes."""

from __future__ import annotations

import traceback

from sqlbuild._native import redact_diagnostic_text
from sqlbuild.compiler.python_nodes.types import (
    PythonNodeFanInAction,
    PythonNodeKind,
    PythonNodeStatus,
    SkipMode,
)
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.python_nodes.constants import (
    ERROR_DETAIL_CHARS,
    ERROR_SUMMARY_CHARS,
    ERROR_SUMMARY_CUT,
    TRACEBACK_ELIDED,
    TRACEBACK_HEADER,
)
from sqlbuild.executor.python_nodes.models import (
    PythonCheckResult,
    PythonNodeExecutionResult,
    PythonNodeFanInDecision,
    PythonNodeResult,
    PythonNodeSkipResult,
)
from sqlbuild.python_nodes.types import PythonCheckSeverity


def normalize_python_check_return(
    *,
    returned: object,
    default_severity: PythonCheckSeverity,
) -> PythonCheckResult:
    """Normalize a raw Python check return value into a check result."""

    if isinstance(returned, PythonCheckResult):
        return returned
    if isinstance(returned, bool):
        return PythonCheckResult(
            passed=returned,
            severity=default_severity,
        )
    raise ExecutorInputError("Python checks must return PythonCheckResult, True, or False")


def normalize_python_node_return(
    *,
    node_name: str,
    kind: PythonNodeKind,
    returned: object,
) -> PythonNodeExecutionResult:
    """Normalize a raw Python-node return value into an execution result."""

    if isinstance(returned, PythonNodeSkipResult):
        return PythonNodeExecutionResult(
            node_name=node_name,
            kind=kind,
            status=PythonNodeStatus.SKIPPED,
            metadata=returned.metadata,
            skip_mode=returned.mode,
            skip_reason=returned.reason,
        )
    if isinstance(returned, PythonNodeResult):
        return PythonNodeExecutionResult(
            node_name=node_name,
            kind=kind,
            status=PythonNodeStatus.SUCCESS,
            payload=returned.payload,
            metadata=returned.metadata,
            materialized=returned.materialized,
        )
    return PythonNodeExecutionResult(
        node_name=node_name,
        kind=kind,
        status=PythonNodeStatus.SUCCESS,
        payload=returned,
    )


def build_python_node_failure_result(
    *,
    node_name: str,
    kind: PythonNodeKind,
    error: BaseException,
) -> PythonNodeExecutionResult:
    """Normalize an exception into a failed Python-node execution result."""

    return PythonNodeExecutionResult(
        node_name=node_name,
        kind=kind,
        status=PythonNodeStatus.FAILED,
        error_message=str(error),
        error_detail=traceback_tail(error=error),
    )


def traceback_tail(*, error: BaseException) -> str:
    """Return the error's redacted traceback, cut to its exception and last frames if too long."""

    text: str = redact_diagnostic_text("".join(traceback.format_exception(error)))
    if len(text) <= ERROR_DETAIL_CHARS:
        return text
    raised: str = redact_diagnostic_text("".join(traceback.format_exception_only(error)))
    frames: list[str] = text.removesuffix(raised).splitlines(keepends=True)
    if len(raised) > ERROR_SUMMARY_CHARS:
        raised = raised[: ERROR_SUMMARY_CHARS - len(ERROR_SUMMARY_CUT)] + ERROR_SUMMARY_CUT
    elided: str = TRACEBACK_ELIDED.format(count=len(frames))
    room: int = ERROR_DETAIL_CHARS - len(TRACEBACK_HEADER) - len(elided) - len(raised)
    kept: list[str] = []
    for line in reversed(frames):
        if len(line) > room:
            break
        kept.append(line)
        room -= len(line)
    kept.reverse()
    if len(kept) == len(frames):
        return "".join(kept) + raised
    elided = TRACEBACK_ELIDED.format(count=len(frames) - len(kept))
    return TRACEBACK_HEADER + elided + "".join(kept) + raised


def evaluate_python_node_fan_in(
    *, upstream_results: tuple[PythonNodeExecutionResult, ...]
) -> PythonNodeFanInDecision:
    """Return whether a node can run, should skip, or is blocked by upstream outcomes."""

    if not upstream_results:
        return PythonNodeFanInDecision(action=PythonNodeFanInAction.RUN)
    failed_names: tuple[str, ...] = tuple(
        result.node_name for result in upstream_results if result.status == PythonNodeStatus.FAILED
    )
    if failed_names:
        return PythonNodeFanInDecision(
            action=PythonNodeFanInAction.BLOCK,
            reason=f"Upstream Python node failed: {', '.join(failed_names)}",
        )
    hard_skipped_names: tuple[str, ...] = tuple(
        result.node_name
        for result in upstream_results
        if result.status == PythonNodeStatus.SKIPPED and result.skip_mode == SkipMode.HARD
    )
    if hard_skipped_names:
        return PythonNodeFanInDecision(
            action=PythonNodeFanInAction.SKIP,
            reason=f"Upstream Python node hard-skipped: {', '.join(hard_skipped_names)}",
            skip_mode=SkipMode.HARD,
        )
    if any(result.status == PythonNodeStatus.SUCCESS for result in upstream_results):
        return PythonNodeFanInDecision(action=PythonNodeFanInAction.RUN)
    return PythonNodeFanInDecision(
        action=PythonNodeFanInAction.SKIP,
        reason="All upstream Python nodes were skipped",
        skip_mode=SkipMode.SOFT,
    )
