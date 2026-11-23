"""Integration tests for single-model view execution lifecycle."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import DiscoveredHookFunction, PythonHookEntry, SqlHookEntry
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.run.models import ModelExecutionResult
from sqlbuild.executor.run.types import ExecutionPhase
from tests.integration.src.sqlbuild.executor.run.view._test_types import (
    ViewFailureTestCase,
    ViewSuccessTestCase,
)
from tests.integration.src.sqlbuild.executor.run.view.helpers import (
    create_python_view_hook_data,
    create_python_view_order_step,
    fail_python_view_hook,
    run_view_failure_test,
    run_view_success_test,
    verify_view_failure_state,
    verify_view_success_state,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ViewSuccessTestCase(
            description="view creates target from query",
            setup_sql=(),
            model_sql="SELECT 1 AS id, 'alice' AS name",
            target_schema="test_schema",
            target_name="dim_customers",
            expected_row_count=1,
        ),
        ViewSuccessTestCase(
            description="view replaces existing view",
            setup_sql=("CREATE VIEW test_schema.dim_customers AS SELECT 1 AS id, 'old' AS name",),
            model_sql="SELECT 2 AS id, 'new' AS name",
            target_schema="test_schema",
            target_name="dim_customers",
            expected_row_count=1,
        ),
        ViewSuccessTestCase(
            description="view with passing audit succeeds",
            setup_sql=(),
            model_sql="SELECT 1 AS id, 'alice' AS name",
            target_schema="test_schema",
            target_name="dim_customers",
            audit_sql='SELECT id FROM __ref("dim_view") WHERE id IS NULL',
            audit_severity="error",
            expected_row_count=1,
            expected_audit_count=1,
        ),
        ViewSuccessTestCase(
            description="view with failing warn audit still succeeds",
            setup_sql=(),
            model_sql="SELECT NULL AS id, 'alice' AS name",
            target_schema="test_schema",
            target_name="dim_customers",
            audit_sql='SELECT id FROM __ref("dim_view") WHERE id IS NULL',
            audit_severity="warn",
            expected_row_count=1,
            expected_audit_count=1,
        ),
        ViewSuccessTestCase(
            description="view with pre_hook runs hook before creation",
            setup_sql=(),
            model_sql="SELECT * FROM test_schema.hook_data",
            target_schema="test_schema",
            target_name="dim_customers",
            pre_hook=[
                SqlHookEntry(statement="CREATE TABLE test_schema.hook_data AS SELECT 42 AS val")
            ],
            expected_row_count=1,
        ),
        ViewSuccessTestCase(
            description="view with python pre_hook runs before creation",
            setup_sql=(),
            model_sql="SELECT * FROM test_schema.python_view_data",
            target_schema="test_schema",
            target_name="dim_customers",
            pre_hook=[PythonHookEntry(name="create_view_data", kwargs={"value": 42})],
            hook_functions=(
                DiscoveredHookFunction(
                    file_path=Path(__file__),
                    relative_path=Path("sql/hooks/python/view.py"),
                    name="create_view_data",
                    function=create_python_view_hook_data,
                ),
            ),
            expected_row_count=1,
            expected_lifecycle_event_fragments=(
                "CREATE TABLE test_schema.python_view_data AS SELECT 42 AS val",
                "python view pre-hook created data for dim_view",
            ),
        ),
        ViewSuccessTestCase(
            description="view with mixed pre_hooks preserves authored order",
            setup_sql=(),
            model_sql="SELECT * FROM test_schema.view_step_3",
            target_schema="test_schema",
            target_name="dim_customers",
            pre_hook=[
                SqlHookEntry(statement="CREATE TABLE test_schema.view_step_1 AS SELECT 40 AS val"),
                PythonHookEntry(
                    name="create_order_step",
                    kwargs={"source": "view_step_1", "target": "view_step_2"},
                ),
                SqlHookEntry(
                    statement=(
                        "CREATE TABLE test_schema.view_step_3 AS "
                        "SELECT val + 1 AS val FROM test_schema.view_step_2"
                    )
                ),
            ],
            hook_functions=(
                DiscoveredHookFunction(
                    file_path=Path(__file__),
                    relative_path=Path("sql/hooks/python/view.py"),
                    name="create_order_step",
                    function=create_python_view_order_step,
                ),
            ),
            expected_row_count=1,
            expected_lifecycle_event_fragments=(
                "CREATE TABLE test_schema.view_step_1 AS SELECT 40 AS val",
                "CREATE TABLE test_schema.view_step_2 AS SELECT val + 1 AS val "
                "FROM test_schema.view_step_1",
                "CREATE TABLE test_schema.view_step_3 AS SELECT val + 1 AS val "
                "FROM test_schema.view_step_2",
            ),
        ),
        ViewSuccessTestCase(
            description="view with post_hook runs hook after creation",
            setup_sql=(),
            model_sql="SELECT 1 AS id",
            target_schema="test_schema",
            target_name="dim_customers",
            post_hook=[
                SqlHookEntry(
                    statement="CREATE TABLE test_schema.post_hook_ran AS SELECT 1 AS marker"
                )
            ],
            expected_row_count=1,
        ),
        ViewSuccessTestCase(
            description="missing target schema warns when query tracking is enabled",
            setup_sql=(),
            model_sql="SELECT 1 AS id",
            target_schema=None,
            target_name="dim_customers",
            expected_row_count=1,
            expected_warning_fragment="target schema is missing",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_view_when_executing_then_succeeds(
    test_case: ViewSuccessTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    result: ModelExecutionResult = run_view_success_test(
        test_case=test_case, adapter=adapter, connection=connection
    )

    assert len(result.audit_results) == test_case.expected_audit_count
    verify_view_success_state(result=result, test_case=test_case, connection=connection)


@pytest.mark.parametrize(
    "test_case",
    [
        ViewFailureTestCase(
            description="view creation failure on bad SQL returns STAGING phase",
            setup_sql=(),
            model_sql="SELECT * FROM test_schema.nonexistent_table",
            target_schema="test_schema",
            target_name="dim_customers",
            expected_failed_phase=ExecutionPhase.STAGING,
        ),
        ViewFailureTestCase(
            description="view with failing error audit reports view already created",
            setup_sql=(),
            model_sql="SELECT NULL AS id, 'alice' AS name",
            target_schema="test_schema",
            target_name="dim_customers",
            audit_sql='SELECT id FROM __ref("dim_view") WHERE id IS NULL',
            audit_severity="error",
            expected_failed_phase=ExecutionPhase.AUDIT,
            expected_error_fragment="final audit for 'dim_view' failed after view creation",
            expected_promoted_relation="test_schema.dim_customers",
            expected_audit_count=1,
        ),
        ViewFailureTestCase(
            description="pre_hook failure blocks view creation",
            setup_sql=(),
            model_sql="SELECT 1 AS id",
            target_schema="test_schema",
            target_name="dim_customers",
            pre_hook=[SqlHookEntry(statement="THIS IS NOT VALID SQL")],
            expected_failed_phase=ExecutionPhase.PRE_HOOK,
        ),
        ViewFailureTestCase(
            description="python pre_hook failure blocks view creation",
            setup_sql=(),
            model_sql="SELECT 1 AS id",
            target_schema="test_schema",
            target_name="dim_customers",
            pre_hook=[PythonHookEntry(name="fail_hook", kwargs={"message": "pre boom"})],
            hook_functions=(
                DiscoveredHookFunction(
                    file_path=Path(__file__),
                    relative_path=Path("sql/hooks/python/view.py"),
                    name="fail_hook",
                    function=fail_python_view_hook,
                ),
            ),
            expected_failed_phase=ExecutionPhase.PRE_HOOK,
            expected_error_fragment='pre_hooks[0] python("fail_hook") failed: pre boom',
        ),
        ViewFailureTestCase(
            description="post_hook failure marks view failed with promoted relation",
            setup_sql=(),
            model_sql="SELECT 1 AS id",
            target_schema="test_schema",
            target_name="dim_customers",
            post_hook=[SqlHookEntry(statement="THIS IS NOT VALID SQL")],
            expected_failed_phase=ExecutionPhase.POST_HOOK,
            expected_promoted_relation="test_schema.dim_customers",
        ),
        ViewFailureTestCase(
            description="python post_hook failure marks view failed with promoted relation",
            setup_sql=(),
            model_sql="SELECT 1 AS id",
            target_schema="test_schema",
            target_name="dim_customers",
            post_hook=[PythonHookEntry(name="fail_hook", kwargs={"message": "post boom"})],
            hook_functions=(
                DiscoveredHookFunction(
                    file_path=Path(__file__),
                    relative_path=Path("sql/hooks/python/view.py"),
                    name="fail_hook",
                    function=fail_python_view_hook,
                ),
            ),
            expected_failed_phase=ExecutionPhase.POST_HOOK,
            expected_error_fragment='post_hooks[0] python("fail_hook") failed: post boom',
            expected_promoted_relation="test_schema.dim_customers",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_view_when_executing_then_fails(
    test_case: ViewFailureTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    result: ModelExecutionResult = run_view_failure_test(
        test_case=test_case, adapter=adapter, connection=connection
    )

    assert result.failed_phase == test_case.expected_failed_phase
    verify_view_failure_state(result=result, test_case=test_case)
