"""Integration tests for the dynamic model lifecycle: create, alter and keep."""

from __future__ import annotations

from typing import Any

import pytest

from sqlbuild._native import PlanAction
from sqlbuild.adapter.contract.types import DynamicListing
from sqlbuild.executor.run.main._execute import execute_dynamic_entry
from sqlbuild.executor.run.models import ModelExecutionResult
from sqlbuild.executor.scheduling.types import ExecutionStatus
from tests.integration.src.sqlbuild.executor.run.dynamic._test_types import (
    DynamicFailureTestCase,
    DynamicSuccessTestCase,
    FailedRestoreTestCase,
)
from tests.integration.src.sqlbuild.executor.run.dynamic.helpers import (
    DynamicDuckDbAdapter,
    dynamic_context,
    listed_types,
)

_BUILT: tuple[str, ...] = ("CREATE TABLE staging.daily_totals AS SELECT 1 AS orders",)


@pytest.mark.parametrize(
    "test_case",
    [
        DynamicSuccessTestCase(
            description="a first run creates on the session warehouse",
            action=PlanAction.CREATE_DYNAMIC,
            setup_sql=(),
            refresh_warehouse=None,
            expected_created=(("'1 hour'", "incremental", "SESSION_WH"),),
            expected_altered=(),
            expected_rows=((3,),),
        ),
        DynamicSuccessTestCase(
            description="a declared refresh warehouse is used over the session's",
            action=PlanAction.CREATE_DYNAMIC,
            setup_sql=(),
            refresh_warehouse="transform_wh",
            expected_created=(("'1 hour'", "incremental", "transform_wh"),),
            expected_altered=(),
            expected_rows=((3,),),
        ),
        DynamicSuccessTestCase(
            description="a view the model was before is dropped and replaced",
            action=PlanAction.CREATE_DYNAMIC,
            setup_sql=("CREATE VIEW staging.daily_totals AS SELECT 1 AS orders",),
            refresh_warehouse=None,
            expected_created=(("'1 hour'", "incremental", "SESSION_WH"),),
            expected_altered=(),
            expected_rows=((3,),),
        ),
        DynamicSuccessTestCase(
            description="drifted settings alter in place and keep the data",
            action=PlanAction.ALTER_DYNAMIC,
            setup_sql=_BUILT,
            refresh_warehouse="transform_wh",
            expected_created=(),
            expected_altered=(("'1 hour'", "transform_wh"),),
            expected_rows=((1,),),
        ),
        DynamicSuccessTestCase(
            description="a kept model runs no refresh statement",
            action=PlanAction.KEEP_DYNAMIC,
            setup_sql=_BUILT,
            refresh_warehouse=None,
            expected_created=(),
            expected_altered=(),
            expected_rows=((1,),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dynamic_entry_when_executing_then_settings_reach_the_warehouse(
    test_case: DynamicSuccessTestCase, connection: Any
) -> None:
    adapter: DynamicDuckDbAdapter = DynamicDuckDbAdapter(session_warehouse="SESSION_WH")
    for sql in test_case.setup_sql:
        connection.execute(sql)

    result: ModelExecutionResult = execute_dynamic_entry(
        context=dynamic_context(
            adapter=adapter,
            connection=connection,
            action=test_case.action,
            refresh_warehouse=test_case.refresh_warehouse,
        )
    )

    assert result.status == ExecutionStatus.SUCCESS, result.error_message
    assert tuple(adapter.created) == test_case.expected_created
    assert tuple(adapter.altered) == test_case.expected_altered
    assert connection.execute("SELECT orders FROM staging.daily_totals").fetchall() == list(
        test_case.expected_rows
    )
    assert connection.execute(
        "SELECT node_name FROM staging._sqlbuild_fingerprints"
    ).fetchall() == [("daily_totals",)]


@pytest.mark.parametrize(
    "test_case",
    [
        DynamicFailureTestCase(
            description="no declared or session warehouse fails before any statement",
            setup_sql=(),
            session_warehouse=None,
            listed_settings=None,
            failing_query=None,
            expected_error_code="A717",
            expected_listed_types=(),
        ),
        DynamicFailureTestCase(
            description="a failed create never drops a relation listed as a dynamic table",
            setup_sql=("CREATE VIEW staging.daily_totals AS SELECT 1 AS orders",),
            session_warehouse="SESSION_WH",
            listed_settings=DynamicListing("1 hour", "SESSION_WH", "ACTIVE"),
            failing_query=None,
            expected_error_code="R002",
            expected_listed_types=(("VIEW",),),
        ),
        DynamicFailureTestCase(
            description="a table the model was before is restored when the create keeps failing",
            setup_sql=("CREATE TABLE staging.daily_totals AS SELECT 1 AS orders",),
            session_warehouse="SESSION_WH",
            listed_settings=None,
            failing_query="SELECT missing_column FROM staging.no_such_table",
            expected_error_code="R002",
            expected_listed_types=(("BASE TABLE",),),
        ),
        DynamicFailureTestCase(
            description="a view the model was before is restored when the create keeps failing",
            setup_sql=("CREATE VIEW staging.daily_totals AS SELECT 1 AS orders",),
            session_warehouse="SESSION_WH",
            listed_settings=None,
            failing_query="SELECT missing_column FROM staging.no_such_table",
            expected_error_code="R002",
            expected_listed_types=(("VIEW",),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unbuildable_dynamic_entry_when_executing_then_fails_keeping_the_relation(
    test_case: DynamicFailureTestCase, connection: Any
) -> None:
    adapter: DynamicDuckDbAdapter = DynamicDuckDbAdapter(
        session_warehouse=test_case.session_warehouse,
        listed_settings=test_case.listed_settings,
        failing_query=test_case.failing_query,
    )
    for sql in test_case.setup_sql:
        connection.execute(sql)

    result: ModelExecutionResult = execute_dynamic_entry(
        context=dynamic_context(
            adapter=adapter,
            connection=connection,
            action=PlanAction.CREATE_DYNAMIC,
            refresh_warehouse=None,
        )
    )

    assert result.status == ExecutionStatus.FAILED
    assert result.error_code == test_case.expected_error_code
    assert listed_types(connection=connection) == list(test_case.expected_listed_types)


@pytest.mark.parametrize(
    "test_case",
    [
        FailedRestoreTestCase(
            description="a failed create and a failed restore are both reported",
            setup_sql=("CREATE TABLE staging.daily_totals AS SELECT 1 AS orders",),
            failing_query="SELECT missing_column FROM staging.no_such_table",
            failing_renames={
                "staging.daily_totals__replaced": "SELECT restore_column FROM staging.no_restore"
            },
            expected_error_code="R002",
            expected_fragments=("no_such_table", "no_restore", "daily_totals__replaced"),
            expected_relations=(("daily_totals__replaced", "BASE TABLE"),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failed_create_and_restore_when_executing_then_reports_both_errors(
    test_case: FailedRestoreTestCase, connection: Any
) -> None:
    adapter: DynamicDuckDbAdapter = DynamicDuckDbAdapter(
        session_warehouse="SESSION_WH",
        failing_query=test_case.failing_query,
        failing_renames=test_case.failing_renames,
    )
    for sql in test_case.setup_sql:
        connection.execute(sql)

    result: ModelExecutionResult = execute_dynamic_entry(
        context=dynamic_context(
            adapter=adapter,
            connection=connection,
            action=PlanAction.CREATE_DYNAMIC,
            refresh_warehouse=None,
        )
    )

    assert result.status == ExecutionStatus.FAILED
    assert result.error_code == test_case.expected_error_code
    message: str = str(result.error_message)
    assert [fragment in message for fragment in test_case.expected_fragments] == [True] * len(
        test_case.expected_fragments
    )
    assert connection.execute(
        "SELECT table_name, table_type FROM information_schema.tables "
        "WHERE table_schema = 'staging' AND table_name LIKE 'daily_totals%' ORDER BY 1"
    ).fetchall() == list(test_case.expected_relations)
