from __future__ import annotations

import time
from datetime import UTC, datetime
from pathlib import Path
from threading import Event, Timer, current_thread

import pytest

from sqlbuild._native import (
    DiscoveredCommandOutputSink,
    DiscoveredEventExporter,
    DiscoveredProvider,
    DiscoveredProviderUsage,
    EventExporterCommandScope,
    EventExporterDispatcher,
    EventExportSummary,
)
from sqlbuild.event_sinks import CommandOutputRecord, CommandOutputStream
from sqlbuild.providers import Provider
from tests.unit.src.sqlbuild.runtime.event_exporting.classes._test_types import (
    EventExporterTeardownTestCase,
    SharedSinkProviderTestCase,
)
from tests.unit.src.sqlbuild.runtime.event_exporting.classes.helpers import (
    BlockingProvider,
    blocking_discovery,
    lifecycle_event,
)


@pytest.mark.parametrize(
    "test_case",
    (
        EventExporterTeardownTestCase("sink returns during the idle wait", 1, 0.1, 0.08, 0.9),
        EventExporterTeardownTestCase("sink still hung after the idle wait", 1, 30.0, 0.9, 3.0),
    ),
    ids=lambda case: case.description,
)
def test_given_live_timed_out_exporter_when_scope_closes_then_provider_tears_down_on_closing_thread(
    test_case: EventExporterTeardownTestCase,
    tmp_path: Path,
) -> None:
    BlockingProvider.reset()
    providers: tuple[DiscoveredProvider, ...]
    exporters: tuple[DiscoveredEventExporter, ...]
    providers, exporters = blocking_discovery()
    dispatcher: EventExporterDispatcher = EventExporterDispatcher(
        shutdown_timeout_seconds=0.02,
        invocation_timeout_seconds=10.0,
    )
    scope: EventExporterCommandScope = EventExporterCommandScope(dispatcher=dispatcher)
    scope.configure_extensions(
        project_dir=tmp_path,
        providers=providers,
        event_exporters=exporters,
    )
    dispatcher.enqueue(lifecycle_event())
    assert BlockingProvider.entered.wait(timeout=0.2)
    release: Timer = Timer(test_case.release_after_seconds, BlockingProvider.release_export.set)
    release.start()
    started: float = time.monotonic()

    summary: EventExportSummary = scope.close()

    elapsed: float = time.monotonic() - started
    release.cancel()
    BlockingProvider.release_export.set()
    assert test_case.min_close_seconds <= elapsed < test_case.max_close_seconds
    assert summary.failed == 1
    assert BlockingProvider.teardown_count == test_case.expected_teardown_count
    assert BlockingProvider.teardown_threads == [current_thread().name]
    _ = scope.close()
    assert BlockingProvider.teardown_count == test_case.expected_teardown_count


@pytest.mark.parametrize(
    "test_case",
    (SharedSinkProviderTestCase("shared lifecycle and output provider", 2),),
    ids=lambda case: case.description,
)
def test_given_both_sink_types_when_delivering_then_they_share_one_provider_instance(
    test_case: SharedSinkProviderTestCase,
    tmp_path: Path,
) -> None:
    delivered_lifecycle: Event = Event()
    provider_ids: list[int] = []

    class Destination(Provider):
        pass

    def publish_lifecycle(*, event: object, destination: Destination) -> None:
        del event
        provider_ids.append(id(destination))
        delivered_lifecycle.set()

    def publish_output(*, record: CommandOutputRecord, destination: Destination) -> None:
        del record
        provider_ids.append(id(destination))

    provider_path: Path = tmp_path / "python/providers/destination.py"
    sink_path: Path = tmp_path / "python/event_sinks/destination.py"
    usage: DiscoveredProviderUsage = DiscoveredProviderUsage(
        provider_name="destination",
        parameter_name="destination",
        annotation_class_name="Destination",
        annotation_module=__name__,
    )
    provider: DiscoveredProvider = DiscoveredProvider(
        file_path=provider_path,
        relative_path=Path("python/providers/destination.py"),
        name="destination",
        provider_class=Destination,
        settings=Destination(),
    )
    lifecycle_sink: DiscoveredEventExporter = DiscoveredEventExporter(
        file_path=sink_path,
        relative_path=Path("python/event_sinks/destination.py"),
        name="publish_lifecycle",
        function=publish_lifecycle,
        provider_usages=(usage,),
    )
    output_sink: DiscoveredCommandOutputSink = DiscoveredCommandOutputSink(
        file_path=sink_path,
        relative_path=Path("python/event_sinks/destination.py"),
        name="publish_output",
        function=publish_output,
        streams=frozenset(CommandOutputStream),
        provider_usages=(usage,),
    )
    dispatcher: EventExporterDispatcher = EventExporterDispatcher()
    scope: EventExporterCommandScope = EventExporterCommandScope(dispatcher=dispatcher)
    scope.configure_extensions(
        project_dir=tmp_path,
        providers=(provider,),
        event_exporters=(lifecycle_sink,),
        command_output_sinks=(output_sink,),
    )

    dispatcher.enqueue(lifecycle_event())
    scope.export_output(
        (
            CommandOutputRecord(
                invocation_id="invocation",
                sequence=0,
                occurred_at=datetime.now(UTC),
                stream=CommandOutputStream.STDOUT,
                message="hello\n",
                external_context={},
            ),
        )
    )
    assert delivered_lifecycle.wait(timeout=0.2)
    _ = scope.close()

    assert len(provider_ids) == test_case.expected_delivery_count
    assert len(set(provider_ids)) == 1
