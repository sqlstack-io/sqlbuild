"""A node's bookkeeping writes escape cancels and terminations, but not its deadline."""

from __future__ import annotations

import contextlib
import importlib

import pytest

from sqlbuild.runtime.execution_limits.classes.active_connections import ActiveConnections
from sqlbuild.runtime.execution_limits.classes.item_deadline import ItemDeadline
from sqlbuild.runtime.execution_limits.main.bookkeeping_statements import bookkeeping_statements
from sqlbuild.runtime.execution_limits.main.cancel_running_statements import (
    cancel_running_statements,
)
from sqlbuild.runtime.execution_limits.main.item_deadline_scope import item_deadline_scope
from sqlbuild.runtime.execution_limits.main.statement_activity import statement_activity
from sqlbuild.runtime.execution_limits.main.tracked_statement_connection import (
    tracked_statement_connection,
)
from sqlbuild.runtime.execution_limits.models import ItemTimeoutPolicy
from sqlbuild.runtime.execution_limits.types import ItemKind
from tests.unit.src.sqlbuild.runtime.execution_limits._test_types import (
    BookkeepingActivityTestCase,
    BookkeepingCancelTestCase,
    BookkeepingDeadlineTestCase,
)
from tests.unit.src.sqlbuild.runtime.execution_limits.helpers import (
    FakeCancellingAdapter,
    run_statement_until_cancelled,
)

_ONE_SECOND: ItemTimeoutPolicy = ItemTimeoutPolicy(limit="1s", seconds=1, source="test")
# Every module that reads the process-wide registry, so a test can give them a fresh one.
_REGISTRY_READERS: tuple[str, ...] = (
    "sqlbuild.runtime.execution_limits.main.tracked_statement_connection",
    "sqlbuild.runtime.execution_limits.main.cancel_running_statements",
    "sqlbuild.runtime.execution_limits.main.statement_activity",
)


@pytest.mark.parametrize(
    "test_case",
    [
        BookkeepingCancelTestCase(
            description="a running step's statement is cancelled",
            marking=contextlib.nullcontext,
            expected_cancelled=True,
        ),
        BookkeepingCancelTestCase(
            description="a finished step's result or identity write is not",
            marking=bookkeeping_statements,
            expected_cancelled=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_running_statement_when_running_statements_are_cancelled_then_bookkeeping_is_left(
    test_case: BookkeepingCancelTestCase,
) -> None:
    adapter: FakeCancellingAdapter = FakeCancellingAdapter(result=True, asynchronous=False)
    with test_case.marking(), tracked_statement_connection(adapter=adapter, connection=object()):
        cancel_running_statements()

    assert adapter.interrupted.is_set() == test_case.expected_cancelled


@pytest.mark.parametrize(
    "test_case",
    [
        BookkeepingDeadlineTestCase(
            description="a result write past its item's limit",
            expected_expired=True,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_bookkeeping_statement_when_its_item_deadline_expires_then_it_is_still_cancelled(
    test_case: BookkeepingDeadlineTestCase,
) -> None:
    adapter: FakeCancellingAdapter = FakeCancellingAdapter(result=True, asynchronous=False)

    with (
        item_deadline_scope(
            kind=ItemKind.TEST,
            name="orders_rollup",
            policy=_ONE_SECOND,
            adapter=adapter,
            connection=None,
        ) as deadline,
        bookkeeping_statements(),
    ):
        run_statement_until_cancelled(adapter=adapter, finished_query_id=None)

    assert isinstance(deadline, ItemDeadline)
    assert deadline.expired == test_case.expected_expired


@pytest.mark.parametrize(
    "test_case",
    [
        BookkeepingActivityTestCase(
            description="a finished step's result or identity write counts as run, not as running",
            marking=bookkeeping_statements,
            expected_activity_during=(True, False),
            expected_activity_after=(True, False),
            expected_cancelled=False,
        ),
        BookkeepingActivityTestCase(
            description="a running step's statement counts as run and as running",
            marking=contextlib.nullcontext,
            expected_activity_during=(True, True),
            expected_activity_after=(True, False),
            expected_cancelled=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_only_a_bookkeeping_write_when_ctrl_c_reads_activity_then_a_statement_has_run(
    test_case: BookkeepingActivityTestCase, monkeypatch: pytest.MonkeyPatch
) -> None:
    registry: ActiveConnections = ActiveConnections()
    for module in _REGISTRY_READERS:
        monkeypatch.setattr(importlib.import_module(module), "running_statements", registry)
    adapter: FakeCancellingAdapter = FakeCancellingAdapter(result=True, asynchronous=False)
    before: tuple[bool, bool] = statement_activity()

    with test_case.marking(), tracked_statement_connection(adapter=adapter, connection=object()):
        during: tuple[bool, bool] = statement_activity()
        cancel_running_statements()

    assert (before, during, statement_activity(), adapter.interrupted.is_set()) == (
        (False, False),
        test_case.expected_activity_during,
        test_case.expected_activity_after,
        test_case.expected_cancelled,
    )
