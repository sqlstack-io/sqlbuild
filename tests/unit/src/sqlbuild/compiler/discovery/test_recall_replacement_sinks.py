from pathlib import Path

import pytest

from sqlbuild._native import (
    DiscoveredRuntimeExtensions,
    EventExporterDiscoveryError,
    ProviderDiscoveryError,
)
from sqlbuild.compiler.discovery.main.runtime_extensions import discover_runtime_extensions
from tests.unit.src.sqlbuild.compiler.discovery._test_types import (
    InvalidRecallSinkTestCase,
    RecallSinkDiscoveryTestCase,
)
from tests.unit.src.sqlbuild.compiler.discovery.helpers import (
    RECALL_SINK_IMPORTS,
    recall_sink_names,
    write_recall_sink_project,
)

LIFECYCLE_REPLACEMENT: str = (
    "@lifecycle_event_sink\n"
    "def keep_runs(event: LifecycleEvent, history: RecallWriter) -> None:\n"
    "    history.publish_lifecycle(event)\n\n"
)
OUTPUT_REPLACEMENT: str = (
    "@command_output_event_sink\n"
    "def redact(record: CommandOutputRecord, writer: RecallWriter) -> None:\n"
    "    writer.publish_command_output(record)\n\n"
)
ORDINARY_SINKS: str = (
    "@lifecycle_event_sink\n"
    "def audit_log(event: LifecycleEvent) -> None:\n"
    "    del event\n\n"
    "@command_output_event_sink\n"
    "def output_log(record: CommandOutputRecord) -> None:\n"
    "    del record\n"
)


@pytest.mark.parametrize(
    "test_case",
    (
        RecallSinkDiscoveryTestCase(
            description="no replacement keeps Recall's default for both streams",
            sink_files={"python/event_sinks/logs.py": RECALL_SINK_IMPORTS + ORDINARY_SINKS},
            expected_replacements=((), ()),
            expected_ordinary=(("audit_log",), ("output_log",)),
        ),
        RecallSinkDiscoveryTestCase(
            description="one replacement per stream, by writer type under any parameter name",
            sink_files={
                "python/event_sinks/recall.py": RECALL_SINK_IMPORTS + LIFECYCLE_REPLACEMENT + OUTPUT_REPLACEMENT,
                "python/event_sinks/logs.py": RECALL_SINK_IMPORTS + ORDINARY_SINKS,
            },
            expected_replacements=((("keep_runs", "history"),), (("redact", "writer"),)),
            expected_ordinary=(("audit_log",), ("output_log",)),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_sinks_taking_a_recall_writer_when_discovering_then_they_replace_recall_only(
    tmp_path: Path, test_case: RecallSinkDiscoveryTestCase
) -> None:
    write_recall_sink_project(project_dir=tmp_path, files=test_case.sink_files)

    extensions: DiscoveredRuntimeExtensions = discover_runtime_extensions(project_dir=tmp_path)

    assert recall_sink_names(extensions) == (
        test_case.expected_replacements,
        test_case.expected_ordinary,
    )


@pytest.mark.parametrize(
    "test_case",
    (
        InvalidRecallSinkTestCase(
            description="two replacements for one stream are both named",
            project_files={
                "python/event_sinks/first.py": RECALL_SINK_IMPORTS + OUTPUT_REPLACEMENT,
                "python/event_sinks/second.py": RECALL_SINK_IMPORTS
                + OUTPUT_REPLACEMENT.replace("def redact", "def mask"),
            },
            expected_error=EventExporterDiscoveryError,
            expected_code="D015",
            expected_message=(
                "Only one sink may replace Recall command-output history; found "
                "python/event_sinks/first.py:redact, python/event_sinks/second.py:mask"
            ),
        ),
        InvalidRecallSinkTestCase(
            description="a sink with two writers",
            project_files={
                "python/event_sinks/recall.py": RECALL_SINK_IMPORTS + "@lifecycle_event_sink\n"
                "def both(event: LifecycleEvent, one: RecallWriter, two: RecallWriter) -> None:\n"
                "    del event, one, two\n",
            },
            expected_error=EventExporterDiscoveryError,
            expected_code="D015",
            expected_message="python/event_sinks/recall.py:both declares more than one RecallWriter parameter",
        ),
        InvalidRecallSinkTestCase(
            description="a replacement with a provider parameter",
            project_files={
                "python/providers/orders_api.py": "from sqlbuild.providers import Provider\n\n\n"
                "class OrdersApi(Provider):\n    'Order service settings.'\n\n"
                "    region: str = 'eu'\n",
                "python/event_sinks/recall.py": RECALL_SINK_IMPORTS + "@command_output_event_sink\n"
                "def redact(record: CommandOutputRecord, writer: RecallWriter, orders_api) -> None:\n"
                "    del record, writer, orders_api\n",
            },
            expected_error=EventExporterDiscoveryError,
            expected_code="D015",
            expected_message="python/event_sinks/recall.py:redact must not declare provider parameters",
        ),
        InvalidRecallSinkTestCase(
            description="a provider named recall_writer",
            project_files={
                "python/providers/recall_writer.py": "from sqlbuild.providers import Provider\n\n\n"
                "class RecallWriter(Provider):\n    'Shadows the writer name.'\n\n"
                "    region: str = 'eu'\n",
                "python/event_sinks/logs.py": RECALL_SINK_IMPORTS + ORDINARY_SINKS,
            },
            expected_error=ProviderDiscoveryError,
            expected_code="D012",
            expected_message="Provider name 'recall_writer' in python/providers/recall_writer.py is reserved",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_recall_replacement_sinks_when_discovering_then_a_coded_error_names_them(
    tmp_path: Path, test_case: InvalidRecallSinkTestCase
) -> None:
    write_recall_sink_project(project_dir=tmp_path, files=test_case.project_files)

    with pytest.raises(test_case.expected_error) as raised:
        discover_runtime_extensions(project_dir=tmp_path)

    assert test_case.expected_message in str(raised.value)
    assert getattr(raised.value, "code", None) == test_case.expected_code


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
