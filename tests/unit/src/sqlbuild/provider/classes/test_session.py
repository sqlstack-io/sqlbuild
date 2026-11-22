from __future__ import annotations

import re
import time
from concurrent.futures import ThreadPoolExecutor
from threading import Barrier, Event, Thread
from typing import ClassVar

import pytest

from sqlbuild.provider.classes.session import ProviderSession
from sqlbuild.provider.exceptions import (
    ProviderLookupError,
    ProviderSetupError,
    ProviderTeardownError,
)
from sqlbuild.providers import Provider
from tests.unit.src.sqlbuild.provider.classes._test_types import (
    ProviderContainerLookupTestCase,
    ProviderContainerMissingTestCase,
    ProviderSessionErrorTestCase,
    ProviderSessionLeftOpenTestCase,
    ProviderSessionLifecycleTestCase,
)


class RecordingProvider(Provider):
    provider_name: ClassVar[str] = "recording"
    events: ClassVar[list[str]] = []
    label: str

    def setup(self, ctx: object) -> None:
        self.events.append(f"setup:{self.label}:{ctx}")

    def teardown(self) -> None:
        self.events.append(f"teardown:{self.label}")


class FailingSetupProvider(Provider):
    provider_name: ClassVar[str] = "failing_setup"

    def setup(self, ctx: object) -> None:
        raise RuntimeError("setup exploded")


class FailingTeardownProvider(Provider):
    provider_name: ClassVar[str] = "failing_teardown"
    events: ClassVar[list[str]] = []

    def setup(self, ctx: object) -> None:
        self.events.append("setup:failing")

    def teardown(self) -> None:
        self.events.append("teardown:failing")
        raise RuntimeError("teardown exploded")


class NamedFailingTeardownProvider(Provider):
    provider_name: ClassVar[str] = "named_failing_teardown"
    label: str

    def teardown(self) -> None:
        raise RuntimeError(f"{self.label} teardown exploded")


class ConcurrentRecordingProvider(Provider):
    provider_name: ClassVar[str] = "concurrent_recording"
    setup_calls: ClassVar[int] = 0
    teardown_calls: ClassVar[int] = 0
    setup_barrier: ClassVar[Barrier | None] = None

    def setup(self, ctx: object) -> None:
        barrier: Barrier | None = self.setup_barrier
        if barrier is not None:
            barrier.wait(timeout=5)
        self.__class__.setup_calls += 1

    def teardown(self) -> None:
        self.__class__.teardown_calls += 1


class HeldSetupProvider(Provider):
    provider_name: ClassVar[str] = "held_setup"
    started: ClassVar[Event] = Event()
    release: ClassVar[Event] = Event()
    events: ClassVar[list[str]] = []
    label: str

    def setup(self, ctx: object) -> None:
        self.events.append(f"setup:{self.label}")
        self.started.set()
        _ = self.release.wait(timeout=30)

    def teardown(self) -> None:
        self.events.append(f"teardown:{self.label}")


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderContainerLookupTestCase(
            description="item lookup returns provider and calls setup",
            lookup_name="clock",
            expected_provider_name="clock",
            expected_setup_events=("setup:clock:runtime",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_container_when_item_lookup_provider_then_returns_provider_and_sets_up_once(
    test_case: ProviderContainerLookupTestCase,
) -> None:
    RecordingProvider.events = []
    session: ProviderSession = ProviderSession(
        {
            "clock": RecordingProvider(label="clock"),
            "alerts": RecordingProvider(label="alerts"),
        },
        setup_context="runtime",
    )

    first_provider: Provider = session.providers[test_case.lookup_name]
    second_provider: Provider = session.providers[test_case.lookup_name]

    assert first_provider is second_provider
    assert first_provider.model_dump()["label"] == test_case.expected_provider_name
    assert tuple(RecordingProvider.events) == test_case.expected_setup_events


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderContainerLookupTestCase(
            description="attribute lookup returns provider and calls setup",
            lookup_name="alerts",
            expected_provider_name="alerts",
            expected_setup_events=("setup:alerts:runtime",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_container_when_attribute_lookup_then_returns_provider_and_sets_up_once(
    test_case: ProviderContainerLookupTestCase,
) -> None:
    RecordingProvider.events = []
    session: ProviderSession = ProviderSession(
        {
            "clock": RecordingProvider(label="clock"),
            "alerts": RecordingProvider(label="alerts"),
        },
        setup_context="runtime",
    )

    first_provider: Provider = getattr(session.providers, test_case.lookup_name)
    second_provider: Provider = getattr(session.providers, test_case.lookup_name)

    assert first_provider is second_provider
    assert first_provider.model_dump()["label"] == test_case.expected_provider_name
    assert tuple(RecordingProvider.events) == test_case.expected_setup_events


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderContainerMissingTestCase(
            description="missing item lookup errors clearly",
            lookup_name="missing",
            expected_error_fragment="Provider 'missing' was not found. Available providers: clock",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_container_when_lookup_missing_provider_then_errors_clearly(
    test_case: ProviderContainerMissingTestCase,
) -> None:
    session: ProviderSession = ProviderSession({"clock": RecordingProvider(label="clock")})

    with pytest.raises(ProviderLookupError, match=test_case.expected_error_fragment):
        session.providers[test_case.lookup_name]


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderContainerMissingTestCase(
            description="missing attribute lookup raises attribute error clearly",
            lookup_name="missing",
            expected_error_fragment="Provider 'missing' was not found. Available providers: clock",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_container_when_attribute_lookup_missing_provider_then_errors_clearly(
    test_case: ProviderContainerMissingTestCase,
) -> None:
    session: ProviderSession = ProviderSession({"clock": RecordingProvider(label="clock")})

    with pytest.raises(AttributeError, match=test_case.expected_error_fragment):
        getattr(session.providers, test_case.lookup_name)


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderContainerMissingTestCase(
            description="missing provider with empty container reports none",
            lookup_name="missing",
            expected_error_fragment="Provider 'missing' was not found. Available providers: none",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_empty_provider_container_when_lookup_missing_provider_then_error_reports_none(
    test_case: ProviderContainerMissingTestCase,
) -> None:
    session: ProviderSession = ProviderSession({})

    with pytest.raises(ProviderLookupError, match=test_case.expected_error_fragment):
        session.providers[test_case.lookup_name]


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderContainerMissingTestCase(
            description="access after close errors clearly",
            lookup_name="clock",
            expected_error_fragment="Provider session is closed; cannot access 'clock'",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_session_when_access_after_close_then_errors_clearly(
    test_case: ProviderContainerMissingTestCase,
) -> None:
    session: ProviderSession = ProviderSession({"clock": RecordingProvider(label="clock")})

    session.close()

    with pytest.raises(ProviderLookupError, match=test_case.expected_error_fragment):
        session.providers[test_case.lookup_name]


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderSessionLifecycleTestCase(
            description="container keys membership and iteration do not setup providers",
            access_names=(),
            expected_events=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_container_when_inspecting_keys_membership_and_iterating_then_no_setup_runs(
    test_case: ProviderSessionLifecycleTestCase,
) -> None:
    RecordingProvider.events = []
    session: ProviderSession = ProviderSession(
        {
            "clock": RecordingProvider(label="clock"),
            "alerts": RecordingProvider(label="alerts"),
        }
    )

    assert session.providers.keys() == ("clock", "alerts")
    assert "clock" in session.providers
    assert "missing" not in session.providers
    assert tuple(session.providers) == ("clock", "alerts")
    assert tuple(RecordingProvider.events) == test_case.expected_events


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderSessionLifecycleTestCase(
            description="container items returns providers and sets each up",
            access_names=("clock", "alerts"),
            expected_events=("setup:clock:runtime", "setup:alerts:runtime"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_container_when_iterating_items_then_returns_providers_and_sets_each_up(
    test_case: ProviderSessionLifecycleTestCase,
) -> None:
    RecordingProvider.events = []
    session: ProviderSession = ProviderSession(
        {
            "clock": RecordingProvider(label="clock"),
            "alerts": RecordingProvider(label="alerts"),
        },
        setup_context="runtime",
    )

    items: tuple[tuple[str, Provider], ...] = session.providers.items()

    assert tuple(name for name, _ in items) == test_case.access_names
    assert tuple(provider.model_dump()["label"] for _, provider in items) == test_case.access_names
    assert tuple(RecordingProvider.events) == test_case.expected_events


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderSessionLifecycleTestCase(
            description="teardown runs for accessed providers in reverse access order",
            access_names=("clock", "alerts"),
            expected_events=(
                "setup:clock:runtime",
                "setup:alerts:runtime",
                "teardown:alerts",
                "teardown:clock",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_session_when_closed_then_teardown_runs_in_reverse_setup_order(
    test_case: ProviderSessionLifecycleTestCase,
) -> None:
    RecordingProvider.events = []
    session: ProviderSession = ProviderSession(
        {
            "clock": RecordingProvider(label="clock"),
            "alerts": RecordingProvider(label="alerts"),
            "unused": RecordingProvider(label="unused"),
        },
        setup_context="runtime",
    )

    for name in test_case.access_names:
        session.providers[name]
    session.close()

    assert tuple(RecordingProvider.events) == test_case.expected_events


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderSessionLifecycleTestCase(
            description="close is idempotent and does not teardown twice",
            access_names=("clock",),
            expected_events=("setup:clock:runtime", "teardown:clock"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_session_when_closed_twice_then_teardown_runs_once(
    test_case: ProviderSessionLifecycleTestCase,
) -> None:
    RecordingProvider.events = []
    session: ProviderSession = ProviderSession(
        {"clock": RecordingProvider(label="clock")},
        setup_context="runtime",
    )

    for name in test_case.access_names:
        session.providers[name]
    session.close()
    session.close()

    assert tuple(RecordingProvider.events) == test_case.expected_events


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderSessionLifecycleTestCase(
            description="concurrent same-session access sets up provider once",
            access_names=("clock",),
            expected_events=(),
            expected_setup_calls=1,
            expected_teardown_calls=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_session_when_concurrent_get_same_provider_then_setup_runs_once(
    test_case: ProviderSessionLifecycleTestCase,
) -> None:
    ConcurrentRecordingProvider.setup_calls = 0
    ConcurrentRecordingProvider.teardown_calls = 0
    ConcurrentRecordingProvider.setup_barrier = Barrier(1)
    session: ProviderSession = ProviderSession(
        {"clock": ConcurrentRecordingProvider()},
        setup_context="runtime",
    )

    with ThreadPoolExecutor(max_workers=8) as executor:
        providers: tuple[Provider, ...] = tuple(
            executor.map(lambda _: session.providers[test_case.access_names[0]], range(16))
        )

    assert len({id(provider) for provider in providers}) == 1
    assert ConcurrentRecordingProvider.setup_calls == test_case.expected_setup_calls
    assert ConcurrentRecordingProvider.teardown_calls == test_case.expected_teardown_calls


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderSessionLifecycleTestCase(
            description="concurrent close teardowns provider once",
            access_names=("clock",),
            expected_events=(),
            expected_setup_calls=1,
            expected_teardown_calls=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_session_when_closed_concurrently_then_teardown_runs_once(
    test_case: ProviderSessionLifecycleTestCase,
) -> None:
    ConcurrentRecordingProvider.setup_calls = 0
    ConcurrentRecordingProvider.teardown_calls = 0
    ConcurrentRecordingProvider.setup_barrier = None
    session: ProviderSession = ProviderSession({"clock": ConcurrentRecordingProvider()})
    session.providers[test_case.access_names[0]]

    with ThreadPoolExecutor(max_workers=8) as executor:
        tuple(executor.map(lambda _: session.close(), range(16)))

    assert ConcurrentRecordingProvider.setup_calls == test_case.expected_setup_calls
    assert ConcurrentRecordingProvider.teardown_calls == test_case.expected_teardown_calls


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderSessionLifecycleTestCase(
            description="teardown runs when context exits with original error",
            access_names=("clock",),
            expected_events=("setup:clock:runtime", "teardown:clock"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_session_when_execution_fails_then_teardown_runs_and_original_error_surfaces(
    test_case: ProviderSessionLifecycleTestCase,
) -> None:
    RecordingProvider.events = []
    session: ProviderSession = ProviderSession(
        {"clock": RecordingProvider(label="clock")},
        setup_context="runtime",
    )

    with pytest.raises(RuntimeError, match="original failure"):
        with session:
            for name in test_case.access_names:
                session.providers[name]
            raise RuntimeError("original failure")

    assert tuple(RecordingProvider.events) == test_case.expected_events


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderSessionErrorTestCase(
            description="setup failure identifies provider",
            provider_name="failing_setup",
            expected_error_fragment=(
                "Provider 'failing_setup' (FailingSetupProvider) failed during setup: "
                "setup exploded"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_session_when_setup_fails_then_errors_clearly(
    test_case: ProviderSessionErrorTestCase,
) -> None:
    session: ProviderSession = ProviderSession({"failing_setup": FailingSetupProvider()})

    with pytest.raises(ProviderSetupError, match=re.escape(test_case.expected_error_fragment)):
        session.providers[test_case.provider_name]


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderSessionErrorTestCase(
            description="teardown failure identifies provider",
            provider_name="failing_teardown",
            expected_error_fragment=(
                "Provider 'failing_teardown' (FailingTeardownProvider) failed during teardown: "
                "teardown exploded"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_session_when_teardown_fails_then_errors_clearly(
    test_case: ProviderSessionErrorTestCase,
) -> None:
    session: ProviderSession = ProviderSession({"failing_teardown": FailingTeardownProvider()})

    session.providers[test_case.provider_name]
    with pytest.raises(
        ProviderTeardownError,
        match=re.escape(test_case.expected_error_fragment),
    ):
        session.close()


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderSessionErrorTestCase(
            description="multiple teardown failures are all reported",
            provider_name="first",
            expected_error_fragment="first teardown exploded",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_session_when_multiple_teardowns_fail_then_all_failures_are_reported(
    test_case: ProviderSessionErrorTestCase,
) -> None:
    session: ProviderSession = ProviderSession(
        {
            "first": NamedFailingTeardownProvider(label="first"),
            "second": NamedFailingTeardownProvider(label="second"),
        }
    )

    session.providers[test_case.provider_name]
    session.providers["second"]
    with pytest.raises(ProviderTeardownError) as exc_info:
        session.close()

    error_message: str = str(exc_info.value)
    assert test_case.expected_error_fragment in error_message
    assert "second teardown exploded" in error_message


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderSessionErrorTestCase(
            description="teardown failure during original failure is recorded",
            provider_name="failing_teardown",
            expected_error_fragment="teardown exploded",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_teardown_failure_when_execution_already_failed_then_original_error_is_not_hidden(
    test_case: ProviderSessionErrorTestCase,
) -> None:
    session: ProviderSession = ProviderSession({"failing_teardown": FailingTeardownProvider()})

    with pytest.raises(RuntimeError, match="original failure"):
        with session:
            session.providers[test_case.provider_name]
            raise RuntimeError("original failure")

    assert session.teardown_error is not None
    assert test_case.expected_error_fragment in str(session.teardown_error)


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderSessionLeftOpenTestCase(
            description="a setup still running when providers are left open and closed",
            within_seconds=1,
            expected_events=("setup:gateway",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_running_setup_when_left_open_and_closed_then_neither_waits_and_no_setup_follows(
    test_case: ProviderSessionLeftOpenTestCase,
) -> None:
    HeldSetupProvider.started = Event()
    HeldSetupProvider.release = Event()
    HeldSetupProvider.events = []
    session: ProviderSession = ProviderSession(
        {
            "gateway": HeldSetupProvider(label="gateway"),
            "archive": HeldSetupProvider(label="archive"),
        }
    )
    lookups: list[str] = []

    def look_up(name: str) -> None:
        try:
            _ = session.providers[name]
            lookups.append(f"{name}:found")
        except ProviderLookupError:
            lookups.append(f"{name}:closed")

    setting_up: Thread = Thread(target=look_up, args=("gateway",), daemon=True)
    setting_up.start()
    assert HeldSetupProvider.started.wait(timeout=5)
    waiting: Thread = Thread(target=look_up, args=("archive",), daemon=True)
    waiting.start()

    began: float = time.monotonic()
    session.providers.leave_open()
    session.close()
    took: float = time.monotonic() - began
    HeldSetupProvider.release.set()
    setting_up.join(timeout=5)
    waiting.join(timeout=5)

    assert took < test_case.within_seconds
    assert tuple(HeldSetupProvider.events) == test_case.expected_events
    assert sorted(lookups) == ["archive:closed", "gateway:found"]
