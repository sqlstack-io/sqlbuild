"""Convert, run and judge one SQL unit test locally."""

from __future__ import annotations

import re
import subprocess
import time

from sqlbuild._native import SqlTestPlanEntry, render_sql_test_comparison_sql
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.testing._helpers.comparison import build_step_results, plan_row_mismatch
from sqlbuild.executor.testing._helpers.difference_samples import add_difference_samples
from sqlbuild.executor.testing._helpers.local_converter import (
    convert_statement,
    run_converted_query,
)
from sqlbuild.executor.testing._helpers.local_duckdb import run_converted_statement
from sqlbuild.executor.testing._helpers.local_results import (
    local_timed_out_result,
    refused_message,
    remaining_seconds,
    single_step_result,
)
from sqlbuild.executor.testing._helpers.timeout_policy import sql_test_timeout
from sqlbuild.executor.testing.constants import (
    GUARD_RAISE_PATTERN,
    LOCAL_SAMPLE_TIMEOUT_SECONDS,
    LOCAL_TEST_ENGINE_CODE,
    LOCAL_TEST_REFUSED_CODE,
    SQL_TEST_ASSERTION_FAILED_CODE,
    SQL_TEST_EXECUTION_ERROR_CODE,
    SQLQUACK_CONVERT_TIMEOUT_SECONDS,
)
from sqlbuild.executor.testing.models import (
    LocalStatementRun,
    LocalTestEngine,
    SqlQuackConversion,
    SqlTestExecutionResult,
    StepResult,
)
from sqlbuild.executor.testing.types import SqlTestOutcome
from sqlbuild.runtime.execution_limits.models import ItemTimeoutPolicy, RunItemTimeouts


def execute_local_sql_test(
    *,
    test_entry: SqlTestPlanEntry,
    adapter: BaseAdapter,
    engine: LocalTestEngine,
    timeouts: RunItemTimeouts,
) -> SqlTestExecutionResult:
    """Convert the test's comparison statement with SQLQuack and judge its rows on DuckDB."""

    try:
        return _judged(test_entry=test_entry, adapter=adapter, engine=engine, timeouts=timeouts)
    except Exception as error:  # noqa: BLE001 - one test's failure never aborts the run
        return single_step_result(
            test_entry=test_entry,
            outcome=SqlTestOutcome.ERROR,
            error_code=LOCAL_TEST_ENGINE_CODE,
            error_message=(
                f"test '{test_entry.name}' could not run locally: {type(error).__name__}: {error}"
            ),
        )


def _judged(
    *,
    test_entry: SqlTestPlanEntry,
    adapter: BaseAdapter,
    engine: LocalTestEngine,
    timeouts: RunItemTimeouts,
) -> SqlTestExecutionResult:
    if engine.session_refusal is not None:
        return single_step_result(
            test_entry=test_entry,
            outcome=SqlTestOutcome.REFUSED,
            error_code=LOCAL_TEST_REFUSED_CODE,
            error_message=f"test '{test_entry.name}' was refused: {engine.session_refusal}",
        )
    (comparison_sql,) = render_sql_test_comparison_sql(
        (test_entry,),
        set_difference_operator=adapter.render_set_difference_operator(),
        sql_analysis_dialect=adapter.sql_analysis_dialect(),
    )
    policy: ItemTimeoutPolicy | None = sql_test_timeout(entry=test_entry, timeouts=timeouts)
    started: float = time.monotonic()
    try:
        conversion: SqlQuackConversion = convert_statement(
            engine=engine,
            sql=comparison_sql,
            timeout_seconds=remaining_seconds(
                policy=policy, started=started, cap=SQLQUACK_CONVERT_TIMEOUT_SECONDS
            ),
        )
    except subprocess.TimeoutExpired:
        if policy is not None and time.monotonic() - started >= policy.seconds:
            return local_timed_out_result(test_entry=test_entry, policy=policy, started=started)
        return single_step_result(
            test_entry=test_entry,
            outcome=SqlTestOutcome.ERROR,
            error_code=LOCAL_TEST_ENGINE_CODE,
            error_message=(
                f"test '{test_entry.name}' could not be converted: SQLQuack did not finish "
                f"within {SQLQUACK_CONVERT_TIMEOUT_SECONDS}s"
            ),
        )
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
        )
    if engine.strict and conversion.assumptions:
        return single_step_result(
            test_entry=test_entry,
            outcome=SqlTestOutcome.REFUSED,
            error_code=LOCAL_TEST_REFUSED_CODE,
            error_message=(
                f"test '{test_entry.name}' was refused under --strict: its conversion relies on "
                f"{', '.join(conversion.assumptions)}"
            ),
        )
    run: LocalStatementRun = run_converted_statement(
        sql=conversion.sql,
        timeout_seconds=remaining_seconds(policy=policy, started=started, cap=None),
    )
    if run.error is not None:
        return _failed_run_result(
            test_entry=test_entry, run=run, conversion=conversion, policy=policy, started=started
        )
    mismatch: str | None = plan_row_mismatch(rows=run.rows, test_entry=test_entry)
    if mismatch is not None:
        return single_step_result(
            test_entry=test_entry,
            outcome=SqlTestOutcome.ERROR,
            error_code=SQL_TEST_EXECUTION_ERROR_CODE,
            error_message=f"test '{test_entry.name}' returned an unusable local result: {mismatch}",
            assumptions=conversion.assumptions,
            local_sql=conversion.sql,
        )
    step_results: list[StepResult] = add_difference_samples(
        step_results=build_step_results(list(run.rows)),
        test_entry=test_entry,
        adapter=adapter,
        connection=None,
        execute=lambda sql: run_converted_query(
            engine=engine, sql=sql, timeout_seconds=LOCAL_SAMPLE_TIMEOUT_SECONDS
        ),
    )
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
        local_sql=conversion.sql,
    )


def _failed_run_result(
    *,
    test_entry: SqlTestPlanEntry,
    run: LocalStatementRun,
    conversion: SqlQuackConversion,
    policy: ItemTimeoutPolicy | None,
    started: float,
) -> SqlTestExecutionResult:
    error: str = run.error or ""
    if run.timed_out and policy is not None:
        return local_timed_out_result(test_entry=test_entry, policy=policy, started=started)
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
            local_sql=conversion.sql,
        )
    return single_step_result(
        test_entry=test_entry,
        outcome=SqlTestOutcome.ERROR,
        error_code=SQL_TEST_EXECUTION_ERROR_CODE,
        error_message=(
            f"test '{test_entry.name}' encountered an execution error on local DuckDB: {error}"
        ),
        assumptions=conversion.assumptions,
        local_sql=conversion.sql,
    )
