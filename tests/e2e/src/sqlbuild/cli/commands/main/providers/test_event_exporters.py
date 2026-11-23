"""End-to-end coverage for provider-backed lifecycle event exporters."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.providers._test_types import (
    CommandOutputE2ETestCase,
    EventExporterE2ETestCase,
    FilteredSinkFailureE2ETestCase,
    NoExporterCommandE2ETestCase,
    ReadOnlyProviderE2ETestCase,
    SharedSinkProviderE2ETestCase,
    SlowSinkExitE2ETestCase,
    SnowflakeKeyWarningE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb
from tests.unit.src.sqlbuild.python_nodes.helpers import node_module_files


@pytest.mark.parametrize(
    "test_case",
    (
        EventExporterE2ETestCase(
            "redacted provider export",
            "invocation_started",
            {"integration": {"name": "test_orchestrator", "run_id": "external-run-1"}},
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_provider_exporter_when_building_then_receives_redacted_events_before_teardown(
    test_case: EventExporterE2ETestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    output_path: Path = tmp_path / "exported.jsonl"
    import_path: Path = tmp_path / "exporter-imports"
    monkeypatch.setenv("EVENT_EXPORT_PATH", str(output_path))
    monkeypatch.setenv("EXPORTER_IMPORT_PATH", str(import_path))
    monkeypatch.setenv("SQLBUILD_INVOCATION_CONTEXT_JSON", json.dumps(test_case.expected_context))
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="lifecycle_event_sink_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "lifecycle_event_sink_project"
                adapter = "duckdb"

                [connection]
                database = "lifecycle_event_sink_project.duckdb"
                """
            ).strip()
            + "\n",
            "python/providers/event_sink.py": dedent(
                """
                import json
                import os
                from pathlib import Path

                from sqlbuild.observability import lifecycle_event_to_json
                from sqlbuild.providers import Provider

                class EventSink(Provider):
                    '''Test provider EventSink.'''
                    path: Path | None = None

                    def setup(self, ctx):
                        del ctx
                        self.path = Path(os.environ["EVENT_EXPORT_PATH"])
                        self.path.write_text("setup\\n", encoding="utf-8")

                    def write(self, event):
                        assert self.path is not None
                        with self.path.open("a", encoding="utf-8") as stream:
                            stream.write(lifecycle_event_to_json(event) + "\\n")

                    def teardown(self):
                        assert self.path is not None
                        with self.path.open("a", encoding="utf-8") as stream:
                            stream.write("teardown\\n")
                """
            ).strip()
            + "\n",
            "python/event_sinks/events.py": dedent(
                """
                from python.providers.event_sink import EventSink
                from sqlbuild.event_sinks import LifecycleEvent, lifecycle_event_sink
                import os
                from pathlib import Path

                _import_path = Path(os.environ["EXPORTER_IMPORT_PATH"])
                with _import_path.open("a", encoding="utf-8") as _stream:
                    _stream.write("imported\\n")

                @lifecycle_event_sink
                def export_event(event: LifecycleEvent, event_sink: EventSink) -> None:
                    event_sink.write(event)

                @lifecycle_event_sink
                def failing_exporter(event: LifecycleEvent, event_sink: EventSink) -> None:
                    del event, event_sink
                    raise RuntimeError("destination credentials must not be reported")
                """
            ).strip()
            + "\n",
            "sql/models/orders.sql": dedent(
                """
                MODEL (description "Test model orders.", materialized table);

                SELECT 'do-not-export-this-sql' AS secret_value
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    lines: list[str] = output_path.read_text(encoding="utf-8").splitlines()
    assert lines[0] == "setup"
    assert lines[-1] == "teardown"
    events: list[dict[str, object]] = [json.loads(line) for line in lines[1:-1]]
    event_types: list[object] = [event["event_type"] for event in events]
    assert event_types[0] == test_case.expected_first_event
    assert event_types[-1] == "invocation_completed"
    assert "statement_completed" in event_types
    assert sorted(event["invocation_sequence"] for event in events) == list(range(len(events)))
    assert all(event["external_context"] == test_case.expected_context for event in events)
    assert "do-not-export-this-sql" not in "\n".join(lines)
    assert import_path.read_text(encoding="utf-8").splitlines() == ["imported"]


@pytest.mark.parametrize(
    "test_case",
    (
        CommandOutputE2ETestCase(
            description="multiline transcript chunks with opaque integration context",
            expected_context={
                "integration": {"name": "test_orchestrator", "run_id": "external-run-1"}
            },
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_command_output_sink_when_compiling_then_multiline_chunks_reconstruct_cli_output(
    test_case: CommandOutputE2ETestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    output_path: Path = tmp_path / "command-output.jsonl"
    monkeypatch.setenv("COMMAND_OUTPUT_PATH", str(output_path))
    monkeypatch.setenv(
        "SQLBUILD_INVOCATION_CONTEXT_JSON",
        json.dumps(test_case.expected_context),
    )
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="command_output_sink_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "command_output_sink_project"
                adapter = "duckdb"

                [connection]
                database = "command_output_sink_project.duckdb"
                """
            ).strip()
            + "\n",
            "python/event_sinks/output.py": dedent(
                """
                import os
                from pathlib import Path

                from sqlbuild.event_sinks import (
                    CommandOutputRecord,
                    command_output_event_sink,
                    command_output_to_json,
                )

                @command_output_event_sink
                def export_output(record: CommandOutputRecord) -> None:
                    path = Path(os.environ["COMMAND_OUTPUT_PATH"])
                    with path.open("a", encoding="utf-8") as stream:
                        stream.write(command_output_to_json(record) + "\\n")
                """
            ).strip()
            + "\n",
            "sql/models/orders.sql": "MODEL (description 'Test model orders.', materialized view);\n\nSELECT 1 AS order_id\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    records: list[dict[str, object]] = [
        json.loads(line) for line in output_path.read_text(encoding="utf-8").splitlines()
    ]
    stdout_records: list[dict[str, object]] = records
    assert all(record["stream"] == "stdout" for record in stdout_records)
    transcript: str = "".join(str(record["message"]) for record in stdout_records)
    assert result.stdout.endswith(transcript)
    assert len(stdout_records) < len(result.stdout.splitlines())
    assert any(str(record["message"]).count("\n") > 1 for record in stdout_records)
    assert all(record["external_context"] == test_case.expected_context for record in records)


@pytest.mark.parametrize(
    "test_case",
    (
        ReadOnlyProviderE2ETestCase(
            description="lineage does not initialize configured providers or output sinks",
            command=("lineage", "orders", "--format", "json"),
            expected_exit_code=0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_configured_provider_and_sink_when_lineage_runs_then_skips_extension_initialization(
    test_case: ReadOnlyProviderE2ETestCase,
    tmp_path: Path,
) -> None:
    provider_marker: Path = tmp_path / "provider-initialized"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="read_only_lineage_project",
        repo_files={
            "sqlbuild_project.toml": 'name = "read_only_lineage_project"\nadapter = "duckdb"\n',
            "python/providers/output.py": (
                "from pathlib import Path\n"
                "from sqlbuild.providers import Provider\n"
                "class OutputProvider(Provider):\n"
                "    '''Test provider OutputProvider.'''\n    def setup(self, ctx):\n"
                "        del ctx\n"
                f"        Path({str(provider_marker)!r}).write_text('initialized', encoding='utf-8')\n"
                "    def write(self, record):\n"
                "        del record\n"
                "    def teardown(self):\n"
                "        pass\n"
            ),
            "python/event_sinks/output.py": (
                "from python.providers.output import OutputProvider\n"
                "from sqlbuild.event_sinks import command_output_event_sink\n"
                "@command_output_event_sink\n"
                "def export_output(record, output_provider: OutputProvider):\n"
                "    output_provider.write(record)\n"
            ),
            "sql/models/orders.sql": "MODEL (description 'Test model orders.', materialized view);\n\nSELECT 1 AS order_id\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.command),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert '"id": "model:orders"' in result.stdout
    assert not provider_marker.exists()


@pytest.mark.parametrize(
    "test_case",
    (
        NoExporterCommandE2ETestCase("clean command", ("clean",), 0),
        NoExporterCommandE2ETestCase("skills command", ("skills", "--target", "agents"), 0),
    ),
    ids=lambda case: case.description,
)
def test_given_no_exporters_and_exploding_provider_when_non_provider_command_runs_then_succeeds(
    test_case: NoExporterCommandE2ETestCase,
    tmp_path: Path,
) -> None:
    marker_path: Path = tmp_path / "provider-imported"
    helper_marker_path: Path = tmp_path / "helper-imported"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="no_exporter_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "no_exporter_project"
                adapter = "duckdb"

                [connection]
                database = "no_exporter_project.duckdb"
                """
            ).lstrip(),
            "python/providers/exploding.py": (
                "from pathlib import Path\n"
                f"Path({str(marker_path)!r}).write_text('imported', encoding='utf-8')\n"
                "raise RuntimeError('provider must not import')\n"
            ),
            "python/event_sinks/helpers.py": (
                "from pathlib import Path\n"
                f"Path({str(helper_marker_path)!r}).write_text('imported', encoding='utf-8')\n"
                "def encode(value):\n    return value\n"
            ),
            "sql/models/example.sql": "MODEL (description 'Test model example.', materialized view);\n\nSELECT 1 AS value\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.command),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert not marker_path.exists()
    assert helper_marker_path.exists()
    assert not (project_dir / ".sqlbuild").exists()


@pytest.mark.parametrize(
    "test_case",
    (
        FilteredSinkFailureE2ETestCase(
            description="failed build filters per sink and reports the failing sink",
            command=("build",),
            expected_exit_code=1,
            expected_runs_only=(),
            expected_errors_only=("invocation_failed",),
            expected_warning=(
                "Warning: lifecycle event export was incomplete: 0 of {accepted} event "
                "deliveries dropped, 1 failed (sink 'failing_on_errors'). Check sink health: a "
                "sink call slower than 1s drops that sink's events until it returns; events "
                "still queued at exit need a longer event_sinks.lifecycle.shutdown_timeout."
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_filtered_and_failing_sinks_when_build_fails_then_delivery_and_warning_match_filters(
    test_case: FilteredSinkFailureE2ETestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    output_dir: Path = tmp_path / "filtered"
    output_dir.mkdir(parents=True)
    monkeypatch.setenv("FILTERED_EXPORT_DIR", str(output_dir))
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="filtered_sink_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "filtered_sink_project"
                adapter = "duckdb"

                [connection]
                database = "filtered_sink_project.duckdb"

                [event_sinks.lifecycle.named.errors_only]
                min_severity = "error"
                """
            ).lstrip(),
            "python/event_sinks/filtered.py": dedent(
                """
                import os
                from pathlib import Path

                from sqlbuild.event_sinks import LifecycleEvent, LifecycleEventKind, lifecycle_event_sink


                def _record(sink: str, event: LifecycleEvent) -> None:
                    path = Path(os.environ["FILTERED_EXPORT_DIR"]) / f"{sink}.txt"
                    with path.open("a", encoding="utf-8") as stream:
                        stream.write(event.event_type + "\\n")


                @lifecycle_event_sink(event_kinds={LifecycleEventKind.RUN}, min_severity="info")
                def runs_only(event: LifecycleEvent) -> None:
                    _record("runs_only", event)


                @lifecycle_event_sink(event_kinds={LifecycleEventKind.INVOCATION})
                def errors_only(event: LifecycleEvent) -> None:
                    _record("errors_only", event)


                @lifecycle_event_sink(name="failing_on_errors", min_severity="error")
                def failing(event: LifecycleEvent) -> None:
                    raise RuntimeError("destination detail must not be reported")
                """
            ),
            "sql/models/orders.sql": (
                "MODEL (description 'Test model orders.', materialized table);\n\n"
                "SELECT 1 AS order_id\n"
            ),
            "sql/models/broken_orders.sql": (
                "MODEL (description 'Test model broken orders.', materialized table);\n\n"
                'SELECT missing_column FROM __ref("orders")\n'
            ),
        },
    )
    (output_dir / "runs_only.txt").touch()
    (output_dir / "errors_only.txt").touch()

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.command),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    runs_only: tuple[str, ...] = tuple(
        (output_dir / "runs_only.txt").read_text(encoding="utf-8").splitlines()
    )
    errors_only: tuple[str, ...] = tuple(
        (output_dir / "errors_only.txt").read_text(encoding="utf-8").splitlines()
    )
    accepted: int = len(runs_only) + len(errors_only) + 1
    assert runs_only == test_case.expected_runs_only
    assert errors_only == test_case.expected_errors_only
    assert test_case.expected_warning.format(accepted=accepted) in result.stderr
    assert "destination detail" not in result.stdout + result.stderr


@pytest.mark.parametrize(
    "test_case",
    (
        SlowSinkExitE2ETestCase(
            description="sink call in flight when the shutdown budget ends",
            command=("audit",),
            expected_marker_entries=("teardown started", "teardown finished"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_sink_call_in_flight_at_shutdown_when_process_exits_then_teardown_completes_without_abort(
    test_case: SlowSinkExitE2ETestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    marker_path: Path = tmp_path / "teardown.txt"
    monkeypatch.setenv("TEARDOWN_MARKER_PATH", str(marker_path))
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="slow_sink_exit_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "slow_sink_exit_project"
                adapter = "duckdb"

                [connection]
                database = "slow_sink_exit_project.duckdb"

                [event_sinks.lifecycle]
                shutdown_timeout = "0s"
                """
            ).lstrip(),
            "python/providers/destination.py": dedent(
                """
                import os
                import sys
                import time
                import types
                from pathlib import Path

                from sqlbuild.providers import Provider


                class _SlowInterpreterExit:
                    def __del__(self, _time=time):
                        _time.sleep(1.0)


                _holder = types.ModuleType("slow_interpreter_exit")
                _holder.finalizer = _SlowInterpreterExit()
                sys.modules["slow_interpreter_exit"] = _holder


                class Destination(Provider):
                    '''Destination whose publishes and flush are slow.'''

                    def publish(self) -> None:
                        time.sleep(0.05)

                    def teardown(self) -> None:
                        path = Path(os.environ["TEARDOWN_MARKER_PATH"])
                        with path.open("a", encoding="utf-8") as stream:
                            stream.write("teardown started\\n")
                        time.sleep(0.5)
                        with path.open("a", encoding="utf-8") as stream:
                            stream.write("teardown finished\\n")
                """
            ).lstrip(),
            "python/event_sinks/publish.py": dedent(
                """
                from python.providers.destination import Destination
                from sqlbuild.event_sinks import LifecycleEvent, lifecycle_event_sink


                @lifecycle_event_sink
                def publish_lifecycle(event: LifecycleEvent, destination: Destination) -> None:
                    del event
                    destination.publish()
                """
            ).lstrip(),
            "sql/models/orders.sql": dedent(
                """
                MODEL (
                  description "Test model orders.",
                  materialized table,
                  columns (order_id (audits [not_null, unique]))
                );

                SELECT range AS order_id FROM range(10)
                """
            ).lstrip(),
        },
    )
    built: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert built.returncode == 0, built.stdout + built.stderr
    marker_path.unlink()

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.command), project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    assert (
        tuple(marker_path.read_text(encoding="utf-8").splitlines())
        == test_case.expected_marker_entries
    )


SHARED_PROVIDER: str = dedent(
    """
    import os
    from pathlib import Path

    from sqlbuild.providers import Provider


    class {class_name}(Provider):
        '''Test provider {class_name}.'''

        def setup(self, ctx):
            del ctx
            with Path(os.environ["SETUP_LOG_PATH"]).open("a", encoding="utf-8") as log:
                log.write("{name}\\n")
    """
).strip()


@pytest.mark.parametrize(
    "test_case",
    (
        SharedSinkProviderE2ETestCase(
            description="the shared provider sorts after one the sink does not use",
            unused_provider_name="a_first",
            expected_setups=("z_shared",),
        ),
        SharedSinkProviderE2ETestCase(
            description="the shared provider sorts before one the sink does not use",
            unused_provider_name="zz_last",
            expected_setups=("z_shared",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_sink_and_task_sharing_a_provider_when_building_then_it_is_set_up_once(
    test_case: SharedSinkProviderE2ETestCase,
    tmp_path: Path,
) -> None:
    setup_log: Path = tmp_path / "setups.log"
    setup_log.write_text("", encoding="utf-8")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shared_sink_provider_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "shared_sink_provider_project"\nadapter = "duckdb"\n\n'
                '[connection]\ndatabase = "shared_sink_provider_project.duckdb"\n'
            ),
            f"python/providers/{test_case.unused_provider_name}.py": SHARED_PROVIDER.format(
                class_name="UnusedProvider", name=test_case.unused_provider_name
            )
            + "\n",
            "python/providers/z_shared.py": SHARED_PROVIDER.format(
                class_name="SharedProvider", name="z_shared"
            )
            + "\n",
            "python/event_sinks/events.py": dedent(
                """
                from python.providers.z_shared import SharedProvider
                from sqlbuild.event_sinks import LifecycleEvent, lifecycle_event_sink


                @lifecycle_event_sink
                def export_event(event: LifecycleEvent, shared_provider: SharedProvider) -> None:
                    del event, shared_provider
                """
            ).strip()
            + "\n",
            **node_module_files(
                "python/nodes/tasks/uses_shared.py",
                dedent(
                    """
                from python.providers.z_shared import SharedProvider
                from sqlbuild.assets import asset


                @asset
                def uses_shared(ctx, shared_provider: SharedProvider):
                    '''Test task uses_shared.'''
                    del ctx, shared_provider
                """
                ).strip()
                + "\n",
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--select", "uses_shared"),
        project_dir=project_dir,
        env={"SETUP_LOG_PATH": str(setup_log)},
    )

    assert result.returncode == 0, result.stdout + result.stderr
    assert tuple(setup_log.read_text(encoding="utf-8").splitlines()) == test_case.expected_setups


@pytest.mark.parametrize(
    "test_case",
    (
        SnowflakeKeyWarningE2ETestCase(description="build", command=("build",)),
        SnowflakeKeyWarningE2ETestCase(description="query", command=("query", "SELECT 1")),
    ),
    ids=lambda case: case.description,
)
def test_given_snowflake_keys_and_a_sink_provider_when_running_then_warns_once(
    test_case: SnowflakeKeyWarningE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snowflake_keys_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "snowflake_keys_project"\nadapter = "duckdb"\n\n'
                '[connection]\ndatabase = "snowflake_keys_project.duckdb"\nwarehouse = "orders_wh"\n'
            ),
            "sql/models/orders.sql": (
                "MODEL (description 'Test model orders.', materialized table);\n\n"
                "SELECT 1 AS order_id\n"
            ),
            "python/providers/probe.py": dedent(
                """
                from sqlbuild.providers import Provider


                class Probe(Provider):
                    '''Test provider Probe.'''

                    def setup(self, ctx):
                        del ctx
                """
            ).strip()
            + "\n",
            "python/event_sinks/events.py": dedent(
                """
                from python.providers.probe import Probe
                from sqlbuild.event_sinks import LifecycleEvent, lifecycle_event_sink


                @lifecycle_event_sink
                def export_event(event: LifecycleEvent, probe: Probe) -> None:
                    del event, probe
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.command), project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    # Startup and the command both resolve the connection; the warning still prints once.
    assert (result.stdout + result.stderr).count(
        "Snowflake-like keys: warehouse."
    ) == test_case.expected_warnings, result.stderr
