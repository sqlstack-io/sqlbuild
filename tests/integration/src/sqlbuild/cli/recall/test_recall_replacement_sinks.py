from __future__ import annotations

import subprocess
import time
from pathlib import Path

import pytest

from sqlbuild._native import COMPILE_REUSE_DISABLE_ENV_VAR as REUSE_DISABLE_ENV_VAR
from tests.integration.src.sqlbuild.cli.recall._test_types import (
    FailingReplacementTestCase,
    InterruptedSinkTestCase,
    ReplacementLaunchRecordTestCase,
    ReplacementLifecycleTestCase,
    ReplacementOutputTestCase,
    SlowReplacementTestCase,
)
from tests.integration.src.sqlbuild.cli.recall.helpers import (
    EVENT_LOG_SINK,
    FAILING_SINKS,
    NATIVE_SQB,
    SLOW_LIFECYCLE_SINK,
    environment,
    filtering_lifecycle_sink,
    interrupted_build,
    logged_event_types,
    recall_rows,
    rewriting_output_sink,
    run,
    write_project,
    write_sinks,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ReplacementLifecycleTestCase(
            description="statement events are filtered from Recall but reach the other sink",
            dropped_prefix="statement_",
            expected_filter_active=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_filtering_lifecycle_sink_when_building_then_recall_keeps_what_it_publishes(
    test_case: ReplacementLifecycleTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_sinks(
        write_project(tmp_path / "orders_shop"),
        {
            "recall_events": filtering_lifecycle_sink(test_case.dropped_prefix),
            "event_log": EVENT_LOG_SINK,
        },
    )

    built: subprocess.CompletedProcess[bytes] = run(project_dir, "build")

    logged, kept = logged_event_types(project_dir, test_case.dropped_prefix)
    assert built.returncode == 0, built.stderr
    assert b"Recall" not in built.stderr
    assert len(kept) < len(logged)
    assert recall_rows(project_dir, "SELECT event_type FROM events ORDER BY event_type") == [
        [event] for event in kept
    ]
    assert recall_rows(
        project_dir,
        "SELECT accepted, delivered, filtered, failed FROM recall_delivery WHERE stream = 'events'",
    ) == [[len(logged), len(kept), len(logged) - len(kept), 0]]
    assert recall_rows(project_dir, "SELECT filter_active FROM output_invocations") == [
        [test_case.expected_filter_active]
    ]


@pytest.mark.parametrize(
    "test_case",
    [
        ReplacementOutputTestCase(
            description="a pass-through sink keeps every byte",
            replaced=(b"", b""),
            expected_complete=1,
        ),
        ReplacementOutputTestCase(
            description="a rewriting sink stores the rewritten bytes",
            replaced=(b"binary", b"BINARY"),
            expected_complete=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_output_replacement_sink_when_recalled_then_its_bytes_are_stored_exactly(
    test_case: ReplacementOutputTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_sinks(
        write_project(tmp_path / "orders_shop"),
        {"recall_output": rewriting_output_sink(test_case.replaced)},
    )

    built: subprocess.CompletedProcess[bytes] = run(project_dir, "build")
    stdout: subprocess.CompletedProcess[bytes] = run(
        project_dir, "recall", "output", "last", "--stream", "stdout"
    )
    stderr: subprocess.CompletedProcess[bytes] = run(
        project_dir, "recall", "output", "last", "--stream", "stderr"
    )

    old, new = test_case.replaced
    assert built.returncode == 0, built.stderr
    assert old in built.stdout
    assert stdout.stdout == built.stdout.replace(old, new)
    assert stderr.stdout == built.stderr.replace(old, new)
    assert recall_rows(project_dir, "SELECT complete, filter_active FROM output_invocations") == [
        [test_case.expected_complete, 1]
    ]


@pytest.mark.parametrize(
    "test_case",
    [
        FailingReplacementTestCase(
            description="raising sinks count their records as failed and the build succeeds",
            expected_failed_streams=[["events"], ["output"]],
            expected_notice=b"sqb build completed successfully; only its Recall history is affected",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_raising_replacement_sinks_when_building_then_records_fail_and_the_command_does_not(
    test_case: FailingReplacementTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_sinks(
        write_project(tmp_path / "orders_shop"), {"recall": FAILING_SINKS}
    )

    built: subprocess.CompletedProcess[bytes] = run(project_dir, "build")

    assert built.returncode == 0, built.stderr
    assert b"sink failed" not in built.stdout + built.stderr
    assert b"Recall" not in built.stdout
    assert built.stderr.rstrip().endswith(test_case.expected_notice + b".")
    assert (
        recall_rows(
            project_dir,
            "SELECT stream FROM recall_delivery WHERE failed > 0 AND failed = accepted "
            "AND delivered = 0 ORDER BY stream",
        )
        == test_case.expected_failed_streams
    )
    assert recall_rows(project_dir, "SELECT command FROM recall_invocations") == [["build"]]


@pytest.mark.parametrize(
    "test_case",
    [
        ReplacementLaunchRecordTestCase(
            description="without sinks sqb may replay the compile",
            sinks={},
            expected_records=1,
        ),
        ReplacementLaunchRecordTestCase(
            description="a Recall output replacement alone withholds the launch record",
            sinks={"recall_output": rewriting_output_sink((b"", b""))},
            expected_records=0,
        ),
        ReplacementLaunchRecordTestCase(
            description="a Recall lifecycle replacement alone withholds the launch record",
            sinks={"recall_events": filtering_lifecycle_sink("statement_")},
            expected_records=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_only_replacement_sinks_when_compiling_through_sqb_then_no_launch_record_is_left(
    test_case: ReplacementLaunchRecordTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_sinks(write_project(tmp_path / "orders_shop"), test_case.sinks)

    compiled: subprocess.CompletedProcess[bytes] = subprocess.run(
        [str(NATIVE_SQB), "compile"],
        cwd=project_dir,
        capture_output=True,
        check=False,
        env=environment(**{REUSE_DISABLE_ENV_VAR: "0"}),
    )

    assert compiled.returncode == 0, compiled.stderr
    records: list[Path] = list((project_dir / "target" / "cache" / "launcher-v1").glob("*.json"))
    assert len(records) == test_case.expected_records


@pytest.mark.parametrize(
    "test_case",
    [
        SlowReplacementTestCase(
            description="a sink sleeping past the budget costs the command at most the budget",
            shutdown_seconds=1.0,
            sink_sleep_seconds="30",
            expected_max_seconds=12.0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_lifecycle_sink_slower_than_the_budget_when_building_then_nothing_else_waits(
    test_case: SlowReplacementTestCase, tmp_path: Path
) -> None:
    budget: str = f'\n[recall]\nshutdown_timeout = "{int(test_case.shutdown_seconds * 1000)}ms"\n'
    project_dir: Path = write_sinks(
        write_project(tmp_path / "orders_shop", extra_toml=budget),
        {"recall_events": SLOW_LIFECYCLE_SINK, "event_log": EVENT_LOG_SINK},
    )

    started: float = time.monotonic()
    built: subprocess.CompletedProcess[bytes] = run(
        project_dir, "build", ORDERS_SINK_SLEEP_SECONDS=test_case.sink_sleep_seconds
    )
    elapsed: float = time.monotonic() - started

    logged: list[str] = (project_dir / "events.log").read_text(encoding="utf-8").splitlines()
    assert built.returncode == 0, built.stderr
    assert elapsed < test_case.expected_max_seconds
    assert logged[-1] == "invocation_completed"
    assert b"Recall history for this run is incomplete" in built.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        InterruptedSinkTestCase(
            description="Ctrl-C stops the build while the sink sleeps",
            shutdown_seconds=1.0,
            sink_sleep_seconds="30",
            expected_exit_code=130,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_slow_lifecycle_sink_when_interrupted_then_the_build_stops(
    test_case: InterruptedSinkTestCase, tmp_path: Path
) -> None:
    budget: str = f'\n[recall]\nshutdown_timeout = "{int(test_case.shutdown_seconds * 1000)}ms"\n'
    project_dir: Path = write_sinks(
        write_project(tmp_path / "orders_shop", extra_toml=budget),
        {"recall_events": SLOW_LIFECYCLE_SINK, "event_log": EVENT_LOG_SINK},
    )

    exit_code: int | None = interrupted_build(
        project_dir,
        ORDERS_SINK_SLEEP_SECONDS=test_case.sink_sleep_seconds,
        ORDERS_TASK_SLEEP_SECONDS="60",
    )

    assert exit_code is not None, "the interrupted build hung"
    assert exit_code == test_case.expected_exit_code


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
