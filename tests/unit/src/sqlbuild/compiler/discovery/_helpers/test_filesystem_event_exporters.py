from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import pytest

from sqlbuild._native import (
    DiscoveredEventExporter,
    DiscoveredProvider,
    EventExporterDiscoveryError,
    discover_event_exporter_functions,
    discover_provider_classes,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers._test_types import (
    EventExporterDiscoveryTestCase,
    EventExporterSignatureErrorTestCase,
    EventExporterTypedSignatureTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDiscoveryTestCase("recursive public discovery", "publish"),),
    ids=lambda case: case.description,
)
def test_given_nested_and_private_files_when_discovering_then_returns_public_exporters(
    test_case: EventExporterDiscoveryTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    declaration = """
from sqlbuild.event_sinks import lifecycle_event_sink

@lifecycle_event_sink
def publish(event):
    pass
"""
    write_repo_files(
        tmp_path,
        {
            "python/event_sinks/nested/public.py": declaration,
            "python/event_sinks/_private.py": declaration,
            "python/event_sinks/_private_dir/hidden.py": declaration,
            "python/event_sinks/nested/__init__.py": declaration,
        },
    )

    exporters: tuple[DiscoveredEventExporter, ...] = discover_event_exporter_functions(
        project_dir=tmp_path
    )

    assert tuple(exporter.name for exporter in exporters) == (test_case.expected_name,)
    assert exporters[0].relative_path.as_posix() == "python/event_sinks/nested/public.py"


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDiscoveryTestCase("provider usage", "sink"),),
    ids=lambda case: case.description,
)
def test_given_provider_parameter_when_discovering_then_records_usage(
    test_case: EventExporterDiscoveryTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "python/providers/sink.py": """
from sqlbuild.providers import Provider

class Sink(Provider):
    pass
""",
            "python/event_sinks/publish.py": """
from python.providers.sink import Sink
from sqlbuild.event_sinks import LifecycleEvent, lifecycle_event_sink

@lifecycle_event_sink
def publish(event: LifecycleEvent, sink: Sink) -> None:
    pass
""",
        },
    )
    providers: tuple[DiscoveredProvider, ...] = discover_provider_classes(project_dir=tmp_path)

    exporters: tuple[DiscoveredEventExporter, ...] = discover_event_exporter_functions(
        project_dir=tmp_path, providers=providers
    )

    assert exporters[0].provider_usages[0].provider_name == test_case.expected_name
    assert exporters[0].provider_usages[0].parameter_name == test_case.expected_name


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDiscoveryTestCase("unannotated provider usage", "sink"),),
    ids=lambda case: case.description,
)
def test_given_unannotated_provider_parameter_when_discovering_then_accepts_name_binding(
    test_case: EventExporterDiscoveryTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "python/providers/sink.py": """
from sqlbuild.providers import Provider

class Sink(Provider):
    pass
""",
            "python/event_sinks/publish.py": """
from sqlbuild.event_sinks import lifecycle_event_sink

@lifecycle_event_sink
def publish(event, sink):
    pass
""",
        },
    )
    providers: tuple[DiscoveredProvider, ...] = discover_provider_classes(project_dir=tmp_path)

    exporters: tuple[DiscoveredEventExporter, ...] = discover_event_exporter_functions(
        project_dir=tmp_path, providers=providers
    )

    assert exporters[0].provider_usages[0].provider_name == test_case.expected_name


@pytest.mark.parametrize(
    "test_case",
    (
        EventExporterSignatureErrorTestCase(
            "async callable", "async def publish(event):\n    pass", "must be synchronous"
        ),
        EventExporterSignatureErrorTestCase(
            "missing event", "def publish(value):\n    pass", "must declare event first"
        ),
        EventExporterSignatureErrorTestCase(
            "unknown provider",
            "def publish(event, missing):\n    pass",
            "unknown provider 'missing'",
        ),
        EventExporterSignatureErrorTestCase(
            "variadic parameter",
            "def publish(event, *args):\n    pass",
            "must use named parameters",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_signature_when_discovering_then_rejects_before_execution(
    test_case: EventExporterSignatureErrorTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "python/event_sinks/invalid.py": (
                "from sqlbuild.event_sinks import lifecycle_event_sink\n\n"
                f"@lifecycle_event_sink\n{test_case.body}\n"
            )
        },
    )

    with pytest.raises(EventExporterDiscoveryError, match=test_case.expected_error):
        discover_event_exporter_functions(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDiscoveryTestCase("duplicate names", "Duplicate event exporter name"),),
    ids=lambda case: case.description,
)
def test_given_duplicate_names_when_discovering_then_rejects_deterministically(
    test_case: EventExporterDiscoveryTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    declaration = """
from sqlbuild.event_sinks import lifecycle_event_sink

@lifecycle_event_sink(name="duplicate")
def publish(event):
    pass
"""
    write_repo_files(
        tmp_path,
        {
            "python/event_sinks/first.py": declaration,
            "python/event_sinks/second.py": declaration,
        },
    )

    with pytest.raises(EventExporterDiscoveryError, match=test_case.expected_name):
        discover_event_exporter_functions(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    (
        EventExporterTypedSignatureTestCase(
            "event default",
            "def publish(event=None):\n    pass",
            "parameters must not have defaults",
        ),
        EventExporterTypedSignatureTestCase(
            "provider default",
            "def publish(event, sink=None):\n    pass",
            "parameters must not have defaults",
        ),
        EventExporterTypedSignatureTestCase(
            "string provider annotation",
            "def publish(event, sink: str):\n    pass",
            "must be unannotated or exactly Sink",
        ),
        EventExporterTypedSignatureTestCase(
            "union provider annotation",
            "def publish(event, sink: Sink | None):\n    pass",
            "must be unannotated or exactly Sink",
        ),
        EventExporterTypedSignatureTestCase(
            "provider base annotation",
            "def publish(event, sink: Provider):\n    pass",
            "must be unannotated or exactly Sink",
        ),
        EventExporterTypedSignatureTestCase(
            "different provider annotation",
            "def publish(event, sink: Other):\n    pass",
            "must be unannotated or exactly Sink",
        ),
        EventExporterTypedSignatureTestCase(
            "non-none return",
            "def publish(event, sink: Sink) -> int:\n    return 1",
            "return annotation must be None",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_typed_signature_when_discovering_then_rejects_exact_contract(
    test_case: EventExporterTypedSignatureTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "python/providers/sink.py": """
from sqlbuild.providers import Provider

class Sink(Provider):
    pass
""",
            "python/event_sinks/invalid.py": (
                "from python.providers.sink import Sink\n"
                "from sqlbuild.event_sinks import lifecycle_event_sink\n"
                "from sqlbuild.providers import Provider\n\n"
                "class Other(Provider):\n    pass\n\n"
                f"@lifecycle_event_sink\n{test_case.declaration}\n"
            ),
        },
    )
    providers: tuple[DiscoveredProvider, ...] = discover_provider_classes(project_dir=tmp_path)

    with pytest.raises(EventExporterDiscoveryError, match=test_case.expected_error):
        discover_event_exporter_functions(project_dir=tmp_path, providers=providers)


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDiscoveryTestCase("function alias deduplication", "publish"),),
    ids=lambda case: case.description,
)
def test_given_decorated_function_alias_when_discovering_then_declares_exporter_once(
    test_case: EventExporterDiscoveryTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "python/event_sinks/aliased.py": """
from sqlbuild.event_sinks import lifecycle_event_sink

@lifecycle_event_sink
def publish(event) -> None:
    pass

alias = publish
"""
        },
    )

    exporters: tuple[DiscoveredEventExporter, ...] = discover_event_exporter_functions(
        project_dir=tmp_path
    )

    assert tuple(exporter.name for exporter in exporters) == (test_case.expected_name,)


@pytest.mark.parametrize(
    "test_case",
    (EventExporterDiscoveryTestCase("legacy exporter directory", "replaced by sinks"),),
    ids=lambda case: case.description,
)
def test_given_legacy_exporter_directory_when_discovering_then_reports_migration(
    test_case: EventExporterDiscoveryTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {"event_exporters/publish.py": "def publish(event):\n    del event\n"},
    )

    with pytest.raises(EventExporterDiscoveryError, match=test_case.expected_name):
        discover_event_exporter_functions(project_dir=tmp_path)
