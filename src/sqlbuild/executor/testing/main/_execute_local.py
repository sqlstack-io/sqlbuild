"""Convert, run and judge one SQL unit test locally."""

from __future__ import annotations

import re
import time

from sqlbuild._native import SqlTestPlanEntry, render_sql_test_comparison_sql
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.testing._helpers.comparison import build_step_results
from sqlbuild.executor.testing._helpers.local_converter import convert_statement
from sqlbuild.executor.testing._helpers.local_duckdb import run_converted_statement
from sqlbuild.executor.testing._helpers.local_results import refused_message, single_step_result
from sqlbuild.executor.testing._helpers.timeout_policy import sql_test_timeout
from sqlbuild.executor.testing._helpers.timeout_result import timed_out_result
from sqlbuild.executor.testing.constants import (
    GUARD_RAISE_PATTERN,
    LOCAL_TEST_REFUSED_CODE,
    SQL_TEST_ASSERTION_FAILED_CODE,
    SQL_TEST_EXECUTION_ERROR_CODE,
)
from sqlbuild.executor.testing.models import (
    LocalStatementRun,
    LocalTestEngine,
    SqlQuackConversion,
    SqlTestExecutionResult,
    StepResult,
)
from sqlbuild.executor.testing.types import SqlTestOutcome
from sqlbuild.runtime.execution_limits.models import (
    ItemTimeoutPolicy,
    ItemTimeoutReport,
    RunItemTimeouts,
)
from sqlbuild.runtime.execution_limits.types import CancellationOutcome, ItemKind


def execute_local_sql_test(
    *,
    test_entry: SqlTestPlanEntry,
    adapter: BaseAdapter,
    engine: LocalTestEngine,
    timeouts: RunItemTimeouts,
) -> SqlTestExecutionResult:
    """Convert the test's comparison statement with SQLQuack and judge its rows on DuckDB."""

    (comparison_sql,) = render_sql_test_comparison_sql(
        (test_entry,),
        set_difference_operator=adapter.render_set_difference_operator(),
        sql_analysis_dialect=adapter.sql_analysis_dialect(),
    )
    try:
        conversion: SqlQuackConversion = convert_statement(engine=engine, sql=comparison_sql)
    except ExecutorInputError as error:
        return single_step_result(
            test_entry=test_entry,
            outcome=SqlTestOutcome.ERROR,
            error_code=error.code,
            error_message=f"test '{test_entry.name}' could not be converted: {error.message}",
        )
    if not conversion.converted:
        return single_step_result(
            test_entry=test_entry,
            outcome=SqlTestOutcome.REFUSED,
            error_code=LOCAL_TEST_REFUSED_CODE,
            error_message=refused_message(test_entry=test_entry, conversion=conversion),
            assumptions=conversion.assumptions,
        )
    policy: ItemTimeoutPolicy | None = sql_test_timeout(entry=test_entry, timeouts=timeouts)
    started: float = time.monotonic()
    run: LocalStatementRun = run_converted_statement(
        sql=conversion.sql, timeout_seconds=policy.seconds if policy is not None else None
    )
    if run.error is not None:
        return _failed_run_result(
            test_entry=test_entry,
            run=run,
            conversion=conversion,
            policy=policy,
            elapsed_seconds=time.monotonic() - started,
        )
    step_results: list[StepResult] = build_step_results(list(run.rows))
    failed: list[str] = [
        step.model_name for step in step_results if step.outcome == SqlTestOutcome.FAIL
    ]
    return SqlTestExecutionResult(
        test_name=test_entry.name,
        outcome=SqlTestOutcome.FAIL if failed else SqlTestOutcome.PASS,
        source_path=test_entry.source_path,
        block_index=test_entry.block_index,
        parent_name=test_entry.parent_name,
        case_name=test_entry.case_name,
        case_index=test_entry.case_index,
        case_fingerprint=test_entry.case_fingerprint,
        parameter_schema=test_entry.parameter_schema,
        parameter_values=test_entry.parameter_values,
        step_results=tuple(step_results),
        error_code=SQL_TEST_ASSERTION_FAILED_CODE if failed else None,
        error_message=(
            f"test '{test_entry.name}' failed for models: {', '.join(failed)}" if failed else None
        ),
        real_model_count=test_entry.real_model_count,
        assumptions=conversion.assumptions,
    )


def _failed_run_result(
    *,
    test_entry: SqlTestPlanEntry,
    run: LocalStatementRun,
    conversion: SqlQuackConversion,
    policy: ItemTimeoutPolicy | None,
    elapsed_seconds: float,
) -> SqlTestExecutionResult:
    error: str = run.error or ""
    if run.timed_out and policy is not None:
        return timed_out_result(
            test_entry=test_entry,
            report=ItemTimeoutReport(
                kind=ItemKind.TEST,
                name=test_entry.name,
                limit=policy.limit,
                source=policy.source,
                elapsed_seconds=elapsed_seconds,
                query_ids=(),
                cancellation=CancellationOutcome.CONFIRMED,
                cancellation_detail=None,
            ),
        )
    guard: re.Match[str] | None = GUARD_RAISE_PATTERN.search(error)
    if guard is not None:
        return single_step_result(
            test_entry=test_entry,
            outcome=SqlTestOutcome.REFUSED,
            error_code=LOCAL_TEST_REFUSED_CODE,
            error_message=(
                f"test '{test_entry.name}' was refused at run time by SQLQuack's "
                f"`{guard.group(1)}` guard: {guard.group(2).strip()}"
            ),
            assumptions=conversion.assumptions,
        )
    return single_step_result(
        test_entry=test_entry,
        outcome=SqlTestOutcome.ERROR,
        error_code=SQL_TEST_EXECUTION_ERROR_CODE,
        error_message=(
            f"test '{test_entry.name}' encountered an execution error on local DuckDB: {error}"
        ),
        assumptions=conversion.assumptions,
    )
