from datetime import UTC, datetime
from pathlib import Path
from threading import Event, current_thread
from typing import ClassVar

from sqlbuild._native import (
    BoundEventExporter,
    DiscoveredEventExporter,
    DiscoveredProvider,
    DiscoveredProviderUsage,
    EventExporterDispatcher,
)
from sqlbuild.observability import LifecycleEvent
from sqlbuild.providers import Provider


class BlockingProvider(Provider):
    entered: ClassVar[Event] = Event()
    release_export: ClassVar[Event] = Event()
    teardown_finished: ClassVar[Event] = Event()
    teardown_count: ClassVar[int] = 0
    teardown_threads: ClassVar[list[str]] = []

    @classmethod
    def reset(cls) -> None:
        cls.entered.clear()
        cls.release_export.clear()
        cls.teardown_finished.clear()
        cls.teardown_count = 0
        cls.teardown_threads = []

    def export(self) -> None:
        self.entered.set()
        self.release_export.wait()

    def teardown(self) -> None:
        type(self).teardown_count += 1
        type(self).teardown_threads.append(current_thread().name)
        self.teardown_finished.set()


def blocking_exporter(*, event: LifecycleEvent, blocking_provider: BlockingProvider) -> None:
    del event
    blocking_provider.export()


def blocking_discovery() -> tuple[
    tuple[DiscoveredProvider, ...], tuple[DiscoveredEventExporter, ...]
]:
    provider_path: Path = Path("python/providers/blocking.py")
    exporter_path: Path = Path("python/event_sinks/blocking.py")
    provider: DiscoveredProvider = DiscoveredProvider(
        file_path=provider_path,
        relative_path=provider_path,
        name="blocking_provider",
        provider_class=BlockingProvider,
        settings=BlockingProvider(),
    )
    exporter: DiscoveredEventExporter = DiscoveredEventExporter(
        file_path=exporter_path,
        relative_path=exporter_path,
        name="blocking_exporter",
        function=blocking_exporter,
        provider_usages=(
            DiscoveredProviderUsage(
                provider_name="blocking_provider",
                parameter_name="blocking_provider",
                annotation_class_name="BlockingProvider",
                annotation_module=__name__,
            ),
        ),
    )
    return (provider,), (exporter,)


def lifecycle_event(index: int = 1, event_type: str = "invocation_started") -> LifecycleEvent:
    return LifecycleEvent(
        event_id=f"event-{index}",
        event_type=event_type,
        schema_version=1,
        producer="test",
        producer_version="1",
        occurred_at=datetime.now(UTC),
        invocation_id="invocation",
        payload={"command": "build"},
    )


def correlated_event(index: int, event_type: str) -> LifecycleEvent:
    return LifecycleEvent(
        event_id=f"event-{index}",
        event_type=event_type,
        schema_version=2,
        producer="test",
        producer_version="1",
        occurred_at=datetime.now(UTC),
        invocation_id="invocation",
        invocation_sequence=index,
        run_id="run",
        resource_id="resource",
        resource_attempt_id="attempt",
        operation_id="operation",
        statement_id="statement",
    )


class BlockedDispatcher:
    """A dispatcher whose only sink holds its first event until released."""

    def __init__(self, *, queue_capacity: int) -> None:
        self.delivered: list[str] = []
        self.entered: Event = Event()
        self.release: Event = Event()

        def record(*, event: LifecycleEvent) -> None:
            self.delivered.append(event.event_id)
            self.entered.set()
            _ = self.release.wait()

        self.dispatcher: EventExporterDispatcher = EventExporterDispatcher(
            exporters=(BoundEventExporter("recorder", record, {}),),
            queue_capacity=queue_capacity,
            invocation_timeout_seconds=5.0,
        )
        self.dispatcher.enqueue(correlated_event(0, "statement_submitted"))
        assert self.entered.wait(timeout=1.0)

    def drain(self, event_types: tuple[str, ...]) -> tuple[tuple[str, ...], int]:
        """Queue events behind the held one, release it, and return delivery order and drops."""

        for index, event_type in enumerate(event_types, start=1):
            self.dispatcher.enqueue(correlated_event(index, event_type))
        self.release.set()
        dropped: int = self.dispatcher.shutdown().dropped
        return tuple(self.delivered[1:]), dropped
