"""Convert, run and judge one SQL unit test locally."""

from __future__ import annotations

import re
import subprocess
import time

from sqlbuild._native import SqlTestPlanEntry, render_sql_test_comparison_sql
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.testing._helpers.comparison import plan_row_mismatch
from sqlbuild.executor.testing._helpers.local_converter import (
    convert_statement,
)
from sqlbuild.executor.testing._helpers.local_duckdb import run_converted_statement
from sqlbuild.executor.testing._helpers.local_results import (
    compared_result,
    local_timed_out_result,
    refused_message,
    remaining_seconds,
    single_step_result,
    with_function_assumptions,
    with_function_setup,
)
from sqlbuild.executor.testing._helpers.timeout_policy import sql_test_timeout
from sqlbuild.executor.testing.constants import (
    GUARD_RAISE_PATTERN,
    LOCAL_TEST_ENGINE_CODE,
    LOCAL_TEST_REFUSED_CODE,
    SQL_TEST_EXECUTION_ERROR_CODE,
    SQLQUACK_CONVERT_TIMEOUT_SECONDS,
)
from sqlbuild.executor.testing.models import (
    LocalFunctionUse,
    LocalStatementRun,
    LocalTestEngine,
    SqlQuackConversion,
    SqlTestExecutionResult,
)
from sqlbuild.executor.testing.types import SqlTestOutcome
from sqlbuild.runtime.execution_limits.models import ItemTimeoutPolicy, RunItemTimeouts


def execute_local_sql_test(
    *,
    test_entry: SqlTestPlanEntry,
    adapter: BaseAdapter,
    engine: LocalTestEngine,
    timeouts: RunItemTimeouts,
    functions: LocalFunctionUse | None = None,
) -> SqlTestExecutionResult:
    """Convert the test's comparison statement with SQLQuack and judge its rows on DuckDB."""

    try:
        return _judged(
            test_entry=test_entry,
            adapter=adapter,
            engine=engine,
            timeouts=timeouts,
            functions=functions or LocalFunctionUse(),
        )
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
    functions: LocalFunctionUse,
) -> SqlTestExecutionResult:
    refusal: str | None = engine.session_refusal or functions.refusal
    if refusal is not None:
        return single_step_result(
            test_entry=test_entry,
            outcome=SqlTestOutcome.REFUSED,
            error_code=LOCAL_TEST_REFUSED_CODE,
            error_message=f"test '{test_entry.name}' was refused: {refusal}",
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
            schema=functions.schema,
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
    conversion = with_function_assumptions(conversion=conversion, functions=functions)
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
    remaining: float | None = remaining_seconds(policy=policy, started=started, cap=None)
    if policy is not None and remaining is not None and remaining <= 0:
        return local_timed_out_result(test_entry=test_entry, policy=policy, started=started)
    run: LocalStatementRun = run_converted_statement(
        sql=conversion.sql, timeout_seconds=remaining, setup=functions.setup
    )
    conversion = with_function_setup(conversion=conversion, functions=functions)
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
    return compared_result(
        test_entry=test_entry,
        adapter=adapter,
        engine=engine,
        functions=functions,
        outcome=(run, conversion),
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
