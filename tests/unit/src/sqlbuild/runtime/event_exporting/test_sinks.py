from __future__ import annotations

from collections.abc import Callable
from typing import cast

import pytest

from sqlbuild._native import EventExportSeverity
from sqlbuild.runtime.event_exporting.exceptions import EventExporterInputError
from sqlbuild.event_sinks import (
    CommandOutputEventSinkDefinition,
    CommandOutputStream,
    LifecycleEventKind,
    LifecycleEventSinkDefinition,
    command_output_event_sink,
    get_command_output_event_sink_definition,
    get_lifecycle_event_sink_definition,
    lifecycle_event_sink,
)
from tests.unit.src.sqlbuild.runtime.event_exporting._test_types import SinkApiTestCase


@pytest.mark.parametrize(
    "test_case",
    (SinkApiTestCase("direct lifecycle decorator", "publish"),),
    ids=lambda case: case.description,
)
def test_given_direct_lifecycle_decorator_when_reading_then_uses_function_name(
    test_case: SinkApiTestCase,
) -> None:
    @lifecycle_event_sink
    def publish(event: object) -> None:
        del event

    definition: LifecycleEventSinkDefinition | None = get_lifecycle_event_sink_definition(
        cast(Callable[..., object], publish)
    )

    assert definition is not None
    assert definition.name == test_case.expected_name


@pytest.mark.parametrize(
    "test_case",
    (SinkApiTestCase("configured lifecycle decorator", "warehouse_events"),),
    ids=lambda case: case.description,
)
def test_given_lifecycle_filters_when_decorating_then_options_are_frozen(
    test_case: SinkApiTestCase,
) -> None:
    @lifecycle_event_sink(
        name="warehouse_events",
        event_kinds={"run", "statement"},
        min_severity="info",
    )
    def publish(event: object) -> None:
        del event

    definition: LifecycleEventSinkDefinition | None = get_lifecycle_event_sink_definition(
        cast(Callable[..., object], publish)
    )

    assert definition is not None
    assert definition.name == test_case.expected_name
    assert definition.event_kinds == frozenset({"run", "statement"})
    assert definition.min_severity is EventExportSeverity.INFO


@pytest.mark.parametrize(
    "test_case",
    (SinkApiTestCase("typed lifecycle kinds", "publish"),),
    ids=lambda case: case.description,
)
def test_given_typed_lifecycle_kinds_when_decorating_then_definition_stores_string_values(
    test_case: SinkApiTestCase,
) -> None:
    @lifecycle_event_sink(event_kinds={LifecycleEventKind.AUDIT, LifecycleEventKind.RUN})
    def publish(event: object) -> None:
        del event

    definition: LifecycleEventSinkDefinition | None = get_lifecycle_event_sink_definition(
        cast(Callable[..., object], publish)
    )

    assert definition is not None
    assert definition.name == test_case.expected_name
    assert definition.event_kinds == frozenset({"audit", "run"})


@pytest.mark.parametrize(
    "test_case",
    (
        SinkApiTestCase(
            "invalid lifecycle severity",
            "min_severity must be one of: debug, info, warning, error",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_lifecycle_severity_when_decorating_then_lists_allowed_values(
    test_case: SinkApiTestCase,
) -> None:
    assert test_case.expected_name is not None
    with pytest.raises(
        EventExporterInputError,
        match=test_case.expected_name,
    ):

        @lifecycle_event_sink(min_severity="fatal")
        def publish(event: object) -> None:
            del event


@pytest.mark.parametrize(
    "test_case",
    (SinkApiTestCase("typed command-output streams", "stdout_only"),),
    ids=lambda case: case.description,
)
def test_given_command_output_decorator_when_reading_then_streams_are_typed(
    test_case: SinkApiTestCase,
) -> None:
    @command_output_event_sink(name="stdout_only", streams={"stdout"})
    def publish(record: object) -> None:
        del record

    definition: CommandOutputEventSinkDefinition | None = get_command_output_event_sink_definition(
        cast(Callable[..., object], publish)
    )

    assert definition is not None
    assert definition.name == test_case.expected_name
    assert definition.streams == frozenset({CommandOutputStream.STDOUT})
    assert get_lifecycle_event_sink_definition(cast(Callable[..., object], publish)) is None


@pytest.mark.parametrize(
    "test_case",
    (SinkApiTestCase("typed definitions remain separate"),),
    ids=lambda case: case.description,
)
def test_given_lifecycle_sink_when_reading_command_output_definition_then_returns_none(
    test_case: SinkApiTestCase,
) -> None:
    @lifecycle_event_sink
    def publish(event: object) -> None:
        del event

    assert test_case.expected_name is None
    assert get_command_output_event_sink_definition(cast(Callable[..., object], publish)) is None


@pytest.mark.parametrize(
    "test_case",
    (SinkApiTestCase("invalid sink name", "lower snake_case"),),
    ids=lambda case: case.description,
)
def test_given_invalid_sink_name_when_decorating_then_rejects_declaration(
    test_case: SinkApiTestCase,
) -> None:
    assert test_case.expected_name is not None
    with pytest.raises(EventExporterInputError, match=test_case.expected_name):

        @command_output_event_sink(name="Bad-Name")
        def publish(record: object) -> None:
            del record


@pytest.mark.parametrize(
    "test_case",
    (SinkApiTestCase("invalid command-output stream", "stdout or stderr"),),
    ids=lambda case: case.description,
)
def test_given_invalid_command_output_stream_when_decorating_then_rejects_declaration(
    test_case: SinkApiTestCase,
) -> None:
    assert test_case.expected_name is not None
    with pytest.raises(EventExporterInputError, match=test_case.expected_name):

        @command_output_event_sink(streams={"diagnostic"})
        def publish(record: object) -> None:
            del record


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
