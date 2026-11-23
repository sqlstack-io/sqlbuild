from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import pytest

from sqlbuild._native import (
    DiscoveredCommandOutputSink,
    DiscoveredProvider,
    EventExporterDiscoveryError,
    discover_provider_classes,
)
from sqlbuild.compiler.discovery._helpers.filesystem.command_output_sinks import (
    discover_command_output_sink_functions,
)
from sqlbuild.event_sinks import CommandOutputStream
from tests.unit.src.sqlbuild.compiler.discovery._helpers._test_types import (
    CommandOutputSinkDiscoveryTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    (CommandOutputSinkDiscoveryTestCase("typed stderr declaration", "publish_output"),),
    ids=lambda case: case.description,
)
def test_given_explicit_command_output_sink_when_discovering_then_returns_typed_declaration(
    test_case: CommandOutputSinkDiscoveryTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "python/event_sinks/output.py": """
from sqlbuild.event_sinks import CommandOutputRecord, command_output_event_sink

@command_output_event_sink(streams={"stderr"})
def publish_output(record: CommandOutputRecord) -> None:
    del record
""",
        },
    )

    sinks: tuple[DiscoveredCommandOutputSink, ...] = discover_command_output_sink_functions(
        project_dir=tmp_path
    )

    assert tuple(sink.name for sink in sinks) == (test_case.expected_name,)
    assert sinks[0].streams == frozenset({CommandOutputStream.STDERR})


@pytest.mark.parametrize(
    "test_case",
    (CommandOutputSinkDiscoveryTestCase("lifecycle declaration is excluded"),),
    ids=lambda case: case.description,
)
def test_given_only_lifecycle_sink_when_discovering_output_then_returns_empty(
    test_case: CommandOutputSinkDiscoveryTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "python/event_sinks/lifecycle.py": """
from sqlbuild.event_sinks import lifecycle_event_sink

@lifecycle_event_sink
def publish_lifecycle(event):
    del event
""",
        },
    )

    assert test_case.expected_name is None
    assert discover_command_output_sink_functions(project_dir=tmp_path) == ()


@pytest.mark.parametrize(
    "test_case",
    (CommandOutputSinkDiscoveryTestCase("provider usage", "destination"),),
    ids=lambda case: case.description,
)
def test_given_provider_parameter_when_discovering_output_then_records_shared_usage(
    test_case: CommandOutputSinkDiscoveryTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "python/providers/destination.py": """
from sqlbuild.providers import Provider

class Destination(Provider):
    pass
""",
            "python/event_sinks/output.py": """
from python.providers.destination import Destination
from sqlbuild.event_sinks import command_output_event_sink

@command_output_event_sink
def publish_output(record, destination: Destination):
    del record, destination
""",
        },
    )
    providers: tuple[DiscoveredProvider, ...] = discover_provider_classes(project_dir=tmp_path)

    sinks: tuple[DiscoveredCommandOutputSink, ...] = discover_command_output_sink_functions(
        project_dir=tmp_path,
        providers=providers,
    )

    assert sinks[0].provider_usages[0].provider_name == test_case.expected_name


@pytest.mark.parametrize(
    "test_case",
    (CommandOutputSinkDiscoveryTestCase("wrong first parameter", "declare record first"),),
    ids=lambda case: case.description,
)
def test_given_wrong_first_parameter_when_discovering_output_then_rejects_signature(
    test_case: CommandOutputSinkDiscoveryTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "python/event_sinks/output.py": """
from sqlbuild.event_sinks import command_output_event_sink

@command_output_event_sink
def publish_output(event):
    del event
""",
        },
    )

    assert test_case.expected_name is not None
    with pytest.raises(EventExporterDiscoveryError, match=test_case.expected_name):
        discover_command_output_sink_functions(project_dir=tmp_path)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
