"""Results of local SQL unit tests that did not reach a comparison."""

from __future__ import annotations

import time

from sqlbuild._native import SqlTestPlanEntry
from sqlbuild.executor.testing._helpers.timeout_result import timed_out_result
from sqlbuild.executor.testing.constants import REFUSAL_DISPLAY_LIMIT
from sqlbuild.executor.testing.models import SqlQuackConversion, SqlTestExecutionResult, StepResult
from sqlbuild.executor.testing.types import SqlTestOutcome
from sqlbuild.runtime.execution_limits.models import ItemTimeoutPolicy, ItemTimeoutReport
from sqlbuild.runtime.execution_limits.types import CancellationOutcome, ItemKind


def refused_message(*, test_entry: SqlTestPlanEntry, conversion: SqlQuackConversion) -> str:
    """Why SQLQuack refused the test, one refusal per line."""

    lines: list[str] = [f"test '{test_entry.name}' was refused by SQLQuack:"]
    lines.extend(
        f"  {refusal.describe()}" for refusal in conversion.refusals[:REFUSAL_DISPLAY_LIMIT]
    )
    hidden: int = len(conversion.refusals) - REFUSAL_DISPLAY_LIMIT
    if hidden > 0:
        lines.append(f"  and {hidden} more")
    lines.extend(f"  note: {note}" for note in conversion.notes)
    return "\n".join(lines)


def single_step_result(
    *,
    test_entry: SqlTestPlanEntry,
    outcome: SqlTestOutcome,
    error_code: str,
    error_message: str,
    assumptions: tuple[str, ...] = (),
    local_sql: str | None = None,
) -> SqlTestExecutionResult:
    """A result whose only step carries the outcome and its error."""

    model_name: str = next(
        (step.model_name for step in test_entry.chain if step.expected_cte_sql is not None),
        test_entry.name,
    )
    return SqlTestExecutionResult(
        test_name=test_entry.name,
        outcome=outcome,
        source_path=test_entry.source_path,
        block_index=test_entry.block_index,
        parent_name=test_entry.parent_name,
        case_name=test_entry.case_name,
        case_index=test_entry.case_index,
        case_fingerprint=test_entry.case_fingerprint,
        parameter_schema=test_entry.parameter_schema,
        parameter_values=test_entry.parameter_values,
        step_results=(
            StepResult(
                model_name=model_name,
                outcome=outcome,
                error_code=error_code,
                error_message=error_message,
            ),
        ),
        error_code=error_code,
        error_message=error_message,
        real_model_count=test_entry.real_model_count,
        assumptions=assumptions,
        local_sql=local_sql,
    )


def remaining_seconds(
    *, policy: ItemTimeoutPolicy | None, started: float, cap: float | None
) -> float | None:
    """What is left of the test's time limit, capped at `cap`; `None` when neither limits it."""

    limits: list[float] = [] if cap is None else [cap]
    if policy is not None:
        limits.append(max(0.0, policy.seconds - (time.monotonic() - started)))
    return min(limits) if limits else None


def local_timed_out_result(
    *, test_entry: SqlTestPlanEntry, policy: ItemTimeoutPolicy, started: float
) -> SqlTestExecutionResult:
    """The test's TIMEOUT result after its limit ran out locally."""

    return timed_out_result(
        test_entry=test_entry,
        report=ItemTimeoutReport(
            kind=ItemKind.TEST,
            name=test_entry.name,
            limit=policy.limit,
            source=policy.source,
            elapsed_seconds=time.monotonic() - started,
            query_ids=(),
            cancellation=CancellationOutcome.CONFIRMED,
            cancellation_detail=None,
        ),
    )
