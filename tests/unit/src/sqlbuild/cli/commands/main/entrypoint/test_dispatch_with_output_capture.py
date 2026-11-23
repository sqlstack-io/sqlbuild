"""Command-scope output capture activation and context tests."""

from __future__ import annotations

import io
import sys

import pytest

from sqlbuild._native import INVOCATION_CONTEXT_ENV, EventExporterCommandScope
from sqlbuild.event_sinks import (
    CommandOutputRecord,
    CommandOutputValidationError,
    command_output_context,
)
from sqlbuild.observability import ExecutionIdentity
from sqlbuild.runtime.output_capture.main.configured_output_capture_scope import (
    configured_output_capture_scope,
)
from tests.unit.src.sqlbuild.cli.commands.main.entrypoint._test_types import (
    OutputCaptureWiringTestCase,
)
from tests.unit.src.sqlbuild.cli.commands.main.entrypoint.helpers import make_event_exporter_scope


@pytest.mark.parametrize(
    "test_case",
    (
        OutputCaptureWiringTestCase(
            description="inert_without_command_output_sink", expected_success=True
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_no_command_output_sink_when_dispatching_then_capture_is_inert(
    test_case: OutputCaptureWiringTestCase, monkeypatch: pytest.MonkeyPatch
) -> None:
    assert test_case.expected_success is True
    sink: io.StringIO = io.StringIO()
    monkeypatch.setattr(sys, "stdout", sink)

    with configured_output_capture_scope(
        exporter_scope=None,
        identity=ExecutionIdentity(invocation_id="invocation-1"),
    ):
        print("terminal only")

    assert sink.getvalue() == "terminal only\n"
    assert sys.stdout is sink


@pytest.mark.parametrize(
    "test_case",
    (
        OutputCaptureWiringTestCase(
            description="configured_command_output_sink", expected_success=True
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_command_output_sink_when_dispatching_then_output_uses_same_provider_scope(
    test_case: OutputCaptureWiringTestCase, monkeypatch: pytest.MonkeyPatch
) -> None:
    assert test_case.expected_success is True
    stdout: io.StringIO = io.StringIO()
    stderr: io.StringIO = io.StringIO()
    records: list[CommandOutputRecord] = []
    exporter_scope: EventExporterCommandScope = make_event_exporter_scope(records=records)
    monkeypatch.setattr(sys, "stdout", stdout)
    monkeypatch.setattr(sys, "stderr", stderr)

    with configured_output_capture_scope(
        exporter_scope=exporter_scope,
        identity=ExecutionIdentity(invocation_id="invocation-1", run_id="run-1"),
    ):
        print("normal")
        print("warning", file=sys.stderr)
    _ = exporter_scope.close()

    assert stdout.getvalue() == "normal\n"
    assert stderr.getvalue() == "warning\n"
    assert tuple(record.message for record in records) == ("normal\n", "warning\n")
    assert tuple(record.sequence for record in records) == (0, 1)
    assert records[0].external_context == {}


@pytest.mark.parametrize(
    "test_case",
    (OutputCaptureWiringTestCase(description="optional_external_context", expected_success=True),),
    ids=lambda case: case.description,
)
def test_given_integration_context_when_output_sink_is_configured_then_context_is_stamped_opaquely(
    test_case: OutputCaptureWiringTestCase,
) -> None:
    assert test_case.expected_success is True
    records: list[CommandOutputRecord] = []
    exporter_scope: EventExporterCommandScope = make_event_exporter_scope(records=records)

    with command_output_context(external_context={"system": "opaque", "external_run": "42"}):
        with configured_output_capture_scope(
            exporter_scope=exporter_scope,
            identity=ExecutionIdentity(invocation_id="invocation-1"),
        ):
            print("context line")
    _ = exporter_scope.close()

    assert records[0].external_context == {"system": "opaque", "external_run": "42"}


@pytest.mark.parametrize(
    "test_case",
    (
        OutputCaptureWiringTestCase(
            description="subprocess_environment_context", expected_success=True
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_environment_context_when_output_sink_is_configured_then_context_crosses_cli_boundary(
    test_case: OutputCaptureWiringTestCase, monkeypatch: pytest.MonkeyPatch
) -> None:
    assert test_case.expected_success is True
    records: list[CommandOutputRecord] = []
    exporter_scope: EventExporterCommandScope = make_event_exporter_scope(records=records)
    monkeypatch.setenv(
        INVOCATION_CONTEXT_ENV,
        '{"integration":{"name":"dagster","run_id":"dagster-run-1"}}',
    )

    with configured_output_capture_scope(
        exporter_scope=exporter_scope,
        identity=ExecutionIdentity(invocation_id="invocation-1"),
    ):
        print("context line")
    _ = exporter_scope.close()

    assert records[0].external_context == {
        "integration": {"name": "dagster", "run_id": "dagster-run-1"}
    }


@pytest.mark.parametrize(
    "test_case",
    (OutputCaptureWiringTestCase(description="invalid_external_context", expected_success=True),),
    ids=lambda case: case.description,
)
def test_given_invalid_integration_context_when_capture_starts_then_reports_once_and_stays_inert(
    test_case: OutputCaptureWiringTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    assert test_case.expected_success is True
    stdout: io.StringIO = io.StringIO()
    records: list[CommandOutputRecord] = []
    failures: list[BaseException] = []
    exporter_scope: EventExporterCommandScope = make_event_exporter_scope(records=records)
    monkeypatch.setattr(sys, "stdout", stdout)

    with command_output_context(external_context={"invalid": object()}):
        with configured_output_capture_scope(
            exporter_scope=exporter_scope,
            identity=ExecutionIdentity(invocation_id="invocation-1"),
            failure_callback=failures.append,
        ):
            print("terminal only")
    _ = exporter_scope.close()

    assert stdout.getvalue() == "terminal only\n"
    assert records == []
    assert len(failures) == 1
    assert isinstance(failures[0], CommandOutputValidationError)


@pytest.mark.parametrize(
    "test_case",
    (OutputCaptureWiringTestCase(description="interrupted_cleanup", expected_success=True),),
    ids=lambda case: case.description,
)
def test_given_interrupted_command_when_cleaning_up_then_stream_restores_and_interrupt_propagates(
    test_case: OutputCaptureWiringTestCase, monkeypatch: pytest.MonkeyPatch
) -> None:
    assert test_case.expected_success is True
    stdout: io.StringIO = io.StringIO()
    records: list[CommandOutputRecord] = []
    exporter_scope: EventExporterCommandScope = make_event_exporter_scope(records=records)
    monkeypatch.setattr(sys, "stdout", stdout)

    with pytest.raises(KeyboardInterrupt):
        with configured_output_capture_scope(
            exporter_scope=exporter_scope,
            identity=ExecutionIdentity(invocation_id="invocation-1"),
        ):
            print("before interrupt")
            raise KeyboardInterrupt
    _ = exporter_scope.close()

    assert stdout.getvalue() == "before interrupt\n"
    assert tuple(record.message for record in records) == ("before interrupt\n",)
    assert sys.stdout is stdout


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
