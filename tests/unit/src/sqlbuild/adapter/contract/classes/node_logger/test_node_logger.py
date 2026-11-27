"""`ctx.log` is a logger: each level records a lifecycle event and logs to the node's logger."""

import logging
from typing import Any, cast

import pytest

from sqlbuild.adapter.contract.classes.node_logger import NodeLogger
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.models import LifeCycleEvent
from sqlbuild.adapter.contract.types import LifeCycleEventKind, LogLevel
from tests.unit.src.sqlbuild.adapter.contract.classes.node_logger._test_types import (
    NodeLoggerCallTestCase,
    NodeLoggerTestCase,
    RecorderForwardingTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        NodeLoggerTestCase("info", LogLevel.INFO, logging.INFO),
        NodeLoggerTestCase("warning", LogLevel.WARNING, logging.WARNING),
        NodeLoggerTestCase("error", LogLevel.ERROR, logging.ERROR),
    ],
    ids=lambda case: case.description,
)
def test_given_a_node_logger_when_logging_at_a_level_then_the_line_keeps_its_level(
    test_case: NodeLoggerTestCase, caplog: pytest.LogCaptureFixture
) -> None:
    recorder: StatementRecorder = StatementRecorder()
    logger: NodeLogger = NodeLogger(recorder=recorder, logger=logging.getLogger("orders.loader"))

    with caplog.at_level(logging.INFO, logger="orders.loader"):
        getattr(logger, test_case.level.value)("loaded 3 orders")

    assert recorder.snapshot() == (
        LifeCycleEvent(
            kind=LifeCycleEventKind.LOG, content="loaded 3 orders", level=test_case.level
        ),
    )
    assert [(record.levelno, record.getMessage()) for record in caplog.records] == [
        (test_case.expected_python_level, "loaded 3 orders")
    ]


@pytest.mark.parametrize(
    "test_case",
    [
        RecorderForwardingTestCase(
            "a ctx.log line goes to on_log as it is kept",
            LogLevel.WARNING,
            ((LogLevel.WARNING, "loaded 3 orders"),),
        ),
        RecorderForwardingTestCase("a line the executor logs itself stays in the node", None, ()),
    ],
    ids=lambda case: case.description,
)
def test_given_a_recorder_with_on_log_when_a_line_is_logged_then_only_ctx_log_lines_are_forwarded(
    test_case: RecorderForwardingTestCase,
) -> None:
    forwarded: list[tuple[LogLevel, str]] = []
    recorder: StatementRecorder = StatementRecorder()
    recorder.on_log = lambda level, message: forwarded.append((level, message))

    recorder.log(message="loaded 3 orders", level=test_case.level)

    assert tuple(forwarded) == test_case.expected_forwarded
    assert [event.content for event in recorder.snapshot()] == ["loaded 3 orders"]


@pytest.mark.parametrize(
    "test_case",
    [NodeLoggerCallTestCase("a message passed to ctx.log itself", "loaded 3 orders", TypeError)],
    ids=lambda case: case.description,
)
def test_given_a_node_logger_when_called_directly_then_it_is_not_callable(
    test_case: NodeLoggerCallTestCase,
) -> None:
    logger: NodeLogger = NodeLogger(recorder=StatementRecorder(), logger=None)

    with pytest.raises(test_case.expected_error):
        cast(Any, logger)(test_case.message)
