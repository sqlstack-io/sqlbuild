"""Integration tests for the maintained model lifecycle: create, alter and keep."""

from __future__ import annotations

from typing import Any

import pytest

from sqlbuild._native import PlanAction
from sqlbuild.executor.run.main._execute import execute_maintained_entry
from sqlbuild.executor.run.models import ModelExecutionResult
from sqlbuild.executor.scheduling.types import ExecutionStatus
from tests.integration.src.sqlbuild.executor.run.maintained._test_types import (
    MaintainedFailureTestCase,
    MaintainedSuccessTestCase,
)
from tests.integration.src.sqlbuild.executor.run.maintained.helpers import (
    MaintainedDuckDbAdapter,
    listed_types,
    maintained_context,
)

_BUILT: tuple[str, ...] = ("CREATE TABLE staging.daily_totals AS SELECT 1 AS orders",)


@pytest.mark.parametrize(
    "test_case",
    [
        MaintainedSuccessTestCase(
            description="a first run creates on the session warehouse",
            action=PlanAction.CREATE_MAINTAINED,
            setup_sql=(),
            refresh_warehouse=None,
            expected_created=(("'1 hour'", "incremental", "SESSION_WH"),),
            expected_altered=(),
            expected_rows=((3,),),
        ),
        MaintainedSuccessTestCase(
            description="a declared refresh warehouse is used over the session's",
            action=PlanAction.CREATE_MAINTAINED,
            setup_sql=(),
            refresh_warehouse="transform_wh",
            expected_created=(("'1 hour'", "incremental", "transform_wh"),),
            expected_altered=(),
            expected_rows=((3,),),
        ),
        MaintainedSuccessTestCase(
            description="a view the model was before is dropped and replaced",
            action=PlanAction.CREATE_MAINTAINED,
            setup_sql=("CREATE VIEW staging.daily_totals AS SELECT 1 AS orders",),
            refresh_warehouse=None,
            expected_created=(("'1 hour'", "incremental", "SESSION_WH"),),
            expected_altered=(),
            expected_rows=((3,),),
        ),
        MaintainedSuccessTestCase(
            description="drifted settings alter in place and keep the data",
            action=PlanAction.ALTER_MAINTAINED,
            setup_sql=_BUILT,
            refresh_warehouse="transform_wh",
            expected_created=(),
            expected_altered=(("'1 hour'", "transform_wh"),),
            expected_rows=((1,),),
        ),
        MaintainedSuccessTestCase(
            description="a kept model runs no refresh statement",
            action=PlanAction.KEEP_MAINTAINED,
            setup_sql=_BUILT,
            refresh_warehouse=None,
            expected_created=(),
            expected_altered=(),
            expected_rows=((1,),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_maintained_entry_when_executing_then_settings_reach_the_warehouse(
    test_case: MaintainedSuccessTestCase, connection: Any
) -> None:
    adapter: MaintainedDuckDbAdapter = MaintainedDuckDbAdapter(session_warehouse="SESSION_WH")
    for sql in test_case.setup_sql:
        connection.execute(sql)

    result: ModelExecutionResult = execute_maintained_entry(
        context=maintained_context(
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
        MaintainedFailureTestCase(
            description="no declared or session warehouse fails before any statement",
            setup_sql=(),
            session_warehouse=None,
            listed_settings=None,
            expected_error_code="A717",
            expected_listed_types=(),
        ),
        MaintainedFailureTestCase(
            description="a failed create never drops a relation listed as a dynamic table",
            setup_sql=("CREATE VIEW staging.daily_totals AS SELECT 1 AS orders",),
            session_warehouse="SESSION_WH",
            listed_settings=("1 hour", "SESSION_WH"),
            expected_error_code="R002",
            expected_listed_types=(("VIEW",),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unbuildable_maintained_entry_when_executing_then_fails_keeping_the_relation(
    test_case: MaintainedFailureTestCase, connection: Any
) -> None:
    adapter: MaintainedDuckDbAdapter = MaintainedDuckDbAdapter(
        session_warehouse=test_case.session_warehouse, listed_settings=test_case.listed_settings
    )
    for sql in test_case.setup_sql:
        connection.execute(sql)

    result: ModelExecutionResult = execute_maintained_entry(
        context=maintained_context(
            adapter=adapter,
            connection=connection,
            action=PlanAction.CREATE_MAINTAINED,
            refresh_warehouse=None,
        )
    )

    assert result.status == ExecutionStatus.FAILED
    assert result.error_code == test_case.expected_error_code
    assert listed_types(connection=connection) == list(test_case.expected_listed_types)
