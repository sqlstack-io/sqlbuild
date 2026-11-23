"""CLI output capture wiring test helpers."""

from __future__ import annotations

from sqlbuild._native import EventExporterCommandScope, EventExporterDispatcher
from sqlbuild.event_sinks import CommandOutputStream
from sqlbuild.runtime.output_capture.models import (
    BoundCommandOutputSink,
    CommandOutputRecord,
)


def make_event_exporter_scope(*, records: list[CommandOutputRecord]) -> EventExporterCommandScope:
    """Build the command-scoped sink infrastructure around a fake destination."""

    def export(*, record: CommandOutputRecord) -> None:
        records.append(record)

    dispatcher: EventExporterDispatcher = EventExporterDispatcher(exporters=())
    dispatcher.bind_command_output_sinks(
        (
            BoundCommandOutputSink(
                name="fake_kafka",
                function=export,
                provider_arguments={},
                streams=frozenset(CommandOutputStream),
            ),
        )
    )
    return EventExporterCommandScope(dispatcher=dispatcher)
