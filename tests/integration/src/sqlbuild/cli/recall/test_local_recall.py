import json
import os
import statistics
import subprocess
from functools import partial
from pathlib import Path

import jsonschema
import pytest

from tests.integration.src.sqlbuild.cli.recall._test_types import (
    AbandonedRunTestCase,
    CaptureLimitTestCase,
    CombinedOutputTestCase,
    ConcurrentBuildsTestCase,
    ConcurrentWritersTestCase,
    CorruptStoreRaceTestCase,
    CorruptStoreTestCase,
    DisabledRecallTestCase,
    FailingStderrTestCase,
    HeldLockTestCase,
    InvalidOutputLimitTestCase,
    JsonQueryTestCase,
    ReadOnlyStoreTestCase,
    RecallOutputTestCase,
    RecallRejectionTestCase,
    RecordedCommandTestCase,
    RecordedFactsTestCase,
    RepeatedBuildTestCase,
    TextOutputTestCase,
    TruncationNoteTestCase,
    UndecodableArgumentTestCase,
)
from tests.integration.src.sqlbuild.cli.recall.helpers import (
    EMITTED_STDERR,
    EMITTED_STDOUT,
    EMITTED_TAIL,
    MEBIBYTE,
    build_while_store_locked,
    build_with_failing_recall_notices,
    build_with_read_only_logs,
    compute_log_streams,
    concurrent_builds,
    killed_build,
    numbered_lines,
    prepared_project,
    recall_rows,
    run,
    simultaneous_builds,
    timed_build,
    write_corrupt_store,
    write_project,
)

RECALL_QUERY_SCHEMA: dict[str, object] = {
    "type": "object",
    "required": [
        "backend",
        "target",
        "store",
        "columns",
        "rows",
        "row_count",
        "truncated",
        "limit",
        "statistics",
    ],
    "additionalProperties": False,
    "properties": {
        "backend": {"const": "logs"},
        "target": {"type": ["string", "null"]},
        "store": {
            "type": "object",
            "required": ["adapter", "database"],
            "properties": {"adapter": {"const": "sqlite"}, "database": {"type": "string"}},
        },
        "columns": {"type": "array", "items": {"type": "string"}},
        "rows": {"type": "array", "items": {"type": "array"}},
        "row_count": {"type": "integer", "minimum": 0},
        "truncated": {"type": "boolean"},
        "limit": {"type": ["integer", "null"]},
        "statistics": {
            "type": "object",
            "required": ["elapsed_ms", "rows_read", "bytes_read", "query_id"],
            "properties": {
                "elapsed_ms": {"type": "number"},
                "rows_read": {"type": "null"},
                "bytes_read": {"type": "null"},
                "query_id": {"type": "null"},
            },
        },
    },
}
DISABLED_TARGET: str = "\n[targets.dev.recall]\nenabled = false\n"
POSIX_PERMISSIONS: bool = os.name == "posix" and os.geteuid() != 0


@pytest.mark.parametrize(
    "test_case",
    [
        RecallOutputTestCase(
            description="stdout", args=("--stream", "stdout"), expected_stream="stdout"
        ),
        RecallOutputTestCase(
            description="stderr", args=("--stream", "stderr"), expected_stream="stderr"
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_build_with_binary_output_when_recalled_then_each_stream_is_byte_exact(
    test_case: RecallOutputTestCase,
    built_project: tuple[Path, subprocess.CompletedProcess[bytes]],
) -> None:
    project_dir, built = built_project

    recalled: subprocess.CompletedProcess[bytes] = run(
        project_dir, "recall", "output", "last", *test_case.args
    )

    assert recalled.returncode == 0, recalled.stderr
    assert recalled.stdout == getattr(built, test_case.expected_stream)
    assert recalled.stderr == b""


@pytest.mark.parametrize(
    "test_case",
    [
        CombinedOutputTestCase(
            description="stdout, then the stderr warning, then more stdout",
            expected_order=(EMITTED_STDOUT, EMITTED_STDERR, EMITTED_TAIL),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_interleaved_writes_when_recalled_combined_then_order_is_preserved(
    test_case: CombinedOutputTestCase,
    built_project: tuple[Path, subprocess.CompletedProcess[bytes]],
) -> None:
    project_dir, built = built_project

    combined: bytes = run(project_dir, "recall", "output", "last").stdout
    transcript: list[list[object]] = recall_rows(
        project_dir,
        "SELECT hex(stdout), hex(stderr) FROM output_transcripts t "
        "JOIN output_invocations o USING (invocation_id) WHERE o.complete = 1",
    )

    positions: list[int] = [combined.index(piece) for piece in test_case.expected_order]
    assert positions == sorted(positions)
    assert len(combined) == len(built.stdout) + len(built.stderr)
    assert transcript == [[built.stdout.hex().upper(), built.stderr.hex().upper()]]


@pytest.mark.parametrize(
    "test_case",
    [
        RecordedFactsTestCase(
            description="the invocation carries its command, target and adapter",
            sql="SELECT status, command, target, adapter, exit_code FROM invocations",
            expected_rows=[["completed", "build", "dev", "duckdb", 0]],
        ),
        RecordedFactsTestCase(
            description="every statement completed and kept its SQL",
            sql="SELECT COUNT(*) > 0, SUM(outcome = 'completed') = COUNT(*), "
            "SUM(sql_available) = COUNT(*) FROM statements",
            expected_rows=[[1, 1, 1]],
        ),
        RecordedFactsTestCase(
            description="natively encoded events keep payloads, promotions and UTC timestamps",
            sql="SELECT COUNT(*) > 0, "
            "SUM(payload = '{}' OR statement_kind IS NULL OR intent IS NULL), "
            "SUM(occurred_at NOT GLOB "
            "'[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9].[0-9][0-9][0-9][0-9][0-9][0-9]Z'), "
            "SUM(invocation_sequence IS NULL) "
            "FROM events WHERE event_type LIKE 'statement_%'",
            expected_rows=[[1, 0, 0, 0]],
        ),
        RecordedFactsTestCase(
            description="the started event names target and adapter",
            sql="SELECT json_extract(payload, '$.target'), json_extract(payload, '$.adapter') "
            "FROM events WHERE event_type = 'invocation_started'",
            expected_rows=[["dev", "duckdb"]],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_build_when_recalled_then_invocation_statements_and_events_are_queryable(
    test_case: RecordedFactsTestCase,
    built_project: tuple[Path, subprocess.CompletedProcess[bytes]],
) -> None:
    project_dir, _ = built_project

    rows: list[list[object]] = recall_rows(project_dir, test_case.sql)

    assert rows == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        JsonQueryTestCase(
            description="a limited query reports truncation",
            args=(
                "--limit",
                "1",
                "SELECT invocation_id, data FROM recall_output ORDER BY sequence",
            ),
            expected_row_count=1,
            expected_truncated=True,
            expected_columns=["invocation_id", "data"],
        ),
        JsonQueryTestCase(
            description="an unlimited query is complete",
            args=("--no-limit", "SELECT COUNT(*) AS total FROM invocations"),
            expected_row_count=1,
            expected_truncated=False,
            expected_columns=["total"],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_recall_query_when_rendered_as_json_then_it_matches_the_schema(
    test_case: JsonQueryTestCase,
    built_project: tuple[Path, subprocess.CompletedProcess[bytes]],
) -> None:
    project_dir, _ = built_project

    queried: subprocess.CompletedProcess[bytes] = run(
        project_dir, "recall", "query", "--json", *test_case.args
    )

    document: dict[str, object] = json.loads(queried.stdout)
    jsonschema.validate(document, RECALL_QUERY_SCHEMA)
    assert document["row_count"] == test_case.expected_row_count
    assert document["truncated"] is test_case.expected_truncated
    assert document["columns"] == test_case.expected_columns


@pytest.mark.parametrize(
    "test_case",
    [
        TextOutputTestCase(
            description="long cells are truncated with the remaining byte count",
            args=(
                "query",
                "--max-cell-bytes",
                "8",
                "SELECT 'first line' || char(10) || 'second line' AS note",
            ),
            expected_fragments=("│ first li…[+14 bytes] │", "1 row"),
        ),
        TextOutputTestCase(
            description="the schema lists views and join keys",
            args=("schema",),
            expected_fragments=(
                "output_transcripts (view)",
                "invocation_id TEXT not null  join key",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_recall_text_output_when_rendered_then_cells_and_schema_are_readable(
    test_case: TextOutputTestCase,
    built_project: tuple[Path, subprocess.CompletedProcess[bytes]],
) -> None:
    project_dir, _ = built_project

    rendered: subprocess.CompletedProcess[bytes] = run(project_dir, "recall", *test_case.args)

    stdout: str = rendered.stdout.decode()
    assert rendered.returncode == 0
    assert [fragment in stdout for fragment in test_case.expected_fragments] == [
        True for _ in test_case.expected_fragments
    ]


@pytest.mark.parametrize(
    "test_case",
    [
        RecallRejectionTestCase(
            description="disabled target",
            project_suffix=DISABLED_TARGET,
            args=("recall", "query", "SELECT 1"),
            expected_code="C970",
        ),
        RecallRejectionTestCase(
            description="unknown invocation",
            project_suffix="",
            args=("recall", "output", "no-such-invocation"),
            expected_code="C972",
        ),
        RecallRejectionTestCase(
            description="write statement",
            project_suffix="",
            args=("recall", "query", "DELETE FROM recall_output"),
            expected_code="C974",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_recall_requests_when_run_then_coded_errors_are_reported(
    test_case: RecallRejectionTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop", extra_toml=test_case.project_suffix)
    _ = run(project_dir, "build")

    rejected: subprocess.CompletedProcess[bytes] = run(project_dir, *test_case.args)

    assert rejected.returncode == 1
    assert f"[{test_case.expected_code}]" in rejected.stderr.decode()


@pytest.mark.parametrize(
    "test_case",
    [
        DisabledRecallTestCase(
            description="a disabled target writes no store",
            project_suffix=DISABLED_TARGET,
            expected_store_exists=False,
        ),
        DisabledRecallTestCase(
            description="the default writes the local store",
            project_suffix="",
            expected_store_exists=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_recall_settings_when_building_then_the_local_store_follows_them(
    test_case: DisabledRecallTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop", extra_toml=test_case.project_suffix)

    built: subprocess.CompletedProcess[bytes] = run(project_dir, "build")

    assert built.returncode == 0
    assert (project_dir / "logs" / "recall.sqlite3").exists() is test_case.expected_store_exists


@pytest.mark.skipif(not POSIX_PERMISSIONS, reason="needs POSIX permissions as a non-root user")
@pytest.mark.parametrize(
    "test_case",
    [
        ReadOnlyStoreTestCase(
            description="the notice carries the DDL once and the build still succeeds",
            expected_fragments=(
                "Recall history is not being written for target `dev`",
                'CREATE TABLE IF NOT EXISTS "recall_output"',
                'CREATE VIEW IF NOT EXISTS "invocations"',
            ),
            expected_repeat_notice=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_read_only_logs_directory_when_building_then_ddl_is_printed_and_exit_is_zero(
    test_case: ReadOnlyStoreTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop")

    built, repeated = build_with_read_only_logs(project_dir)

    stderr: str = built.stderr.decode(errors="replace")
    assert (built.returncode, repeated.returncode) == (0, 0)
    assert [fragment in stderr for fragment in test_case.expected_fragments] == [
        True for _ in test_case.expected_fragments
    ]
    assert "Completed successfully" in built.stdout.decode(errors="replace")
    assert (
        "Recall history is not being written" in repeated.stderr.decode(errors="replace")
    ) is test_case.expected_repeat_notice


@pytest.mark.parametrize(
    "test_case",
    [
        AbandonedRunTestCase(
            description="a SIGKILLed build is abandoned with incomplete output",
            heartbeat_seconds="0.3",
            expected_status="abandoned",
            expected_output_complete=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_killed_build_when_heartbeats_stop_then_the_invocation_is_abandoned(
    test_case: AbandonedRunTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop")

    running_id, recorded = killed_build(project_dir, heartbeat_seconds=test_case.heartbeat_seconds)

    assert recorded == [
        [running_id, test_case.expected_status, 1, 1, test_case.expected_output_complete]
    ]


@pytest.mark.parametrize(
    "test_case",
    [
        ConcurrentBuildsTestCase(
            description="four builds and a polling reader share one store",
            targets=("dev", "dev_1", "dev_2", "dev_3"),
            expected_status="completed",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_four_concurrent_builds_and_a_reader_when_run_then_every_run_is_recorded(
    test_case: ConcurrentBuildsTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop", targets=len(test_case.targets))

    exit_codes, reads, unexpected_failures = concurrent_builds(project_dir, test_case.targets)
    recorded: list[list[object]] = recall_rows(
        project_dir,
        "SELECT i.target, i.status, o.complete FROM invocations i "
        "JOIN output_invocations o USING (invocation_id) ORDER BY i.target",
    )

    assert exit_codes == [0 for _ in test_case.targets]
    assert reads > 0
    assert unexpected_failures == 0
    assert recorded == [
        [target, test_case.expected_status, 1] for target in sorted(test_case.targets)
    ]


@pytest.mark.parametrize(
    "test_case",
    [
        RepeatedBuildTestCase(
            description="repeated SQL is shared, never a conflict",
            builds=2,
            sql="SELECT (SELECT COUNT(*) FROM conflicts), "
            "(SELECT SUM(conflicts) FROM recall_delivery)",
            expected_rows=[[0, 0]],
        ),
        RepeatedBuildTestCase(
            description="every stream of every build delivers exactly what it accepted",
            builds=2,
            sql="SELECT COUNT(DISTINCT invocation_id), "
            "SUM(accepted != delivered + filtered + dropped + failed), "
            "SUM(dropped + failed), MIN(flush_complete) FROM recall_delivery",
            expected_rows=[[2, 0, 0, 1]],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_identical_builds_when_recorded_then_artifacts_and_delivery_counts_stay_exact(
    test_case: RepeatedBuildTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop")

    exit_codes: list[int] = [run(project_dir, "build").returncode for _ in range(test_case.builds)]
    rows: list[list[object]] = recall_rows(project_dir, test_case.sql)

    assert exit_codes == [0 for _ in range(test_case.builds)]
    assert rows == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        RecordedCommandTestCase(
            description="compile",
            args=("compile",),
            expected_recorded=[["compile", "completed"]],
        ),
        RecordedCommandTestCase(
            description="build",
            args=("build",),
            expected_recorded=[["build", "completed"]],
        ),
        RecordedCommandTestCase(
            description="plan",
            args=("plan",),
            expected_recorded=[["plan", "completed"]],
        ),
        RecordedCommandTestCase(
            description="dag",
            args=("dag",),
            expected_recorded=[["dag", "completed"]],
        ),
        RecordedCommandTestCase(
            description="check",
            args=("check",),
            expected_recorded=[["check", "completed"]],
        ),
        RecordedCommandTestCase(
            description="test",
            args=("test",),
            expected_recorded=[["test", "completed"]],
        ),
        RecordedCommandTestCase(
            description="lineage orders",
            args=("lineage", "orders"),
            expected_recorded=[["lineage", "completed"]],
        ),
        RecordedCommandTestCase(
            description="debug",
            args=("debug",),
            expected_recorded=[["debug", "completed"]],
        ),
        RecordedCommandTestCase(
            description="query SELECT 1 AS one",
            args=("query", "SELECT 1 AS one"),
            expected_recorded=[["query", "completed"]],
        ),
        RecordedCommandTestCase(
            description="a raw-query diff is not observed, so it is not recorded",
            args=(
                "diff",
                "--unkeyed",
                "--left-query",
                "SELECT 1 AS one",
                "--right-query",
                "SELECT 1 AS one",
            ),
            expected_recorded=[],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_successful_command_when_recorded_then_it_ends_completed(
    test_case: RecordedCommandTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop")
    _ = run(project_dir, "build")

    completed: subprocess.CompletedProcess[bytes] = run(project_dir, *test_case.args)
    recorded: list[list[object]] = recall_rows(
        project_dir,
        "SELECT command, status FROM invocations "
        f"WHERE command = '{test_case.args[0]}' ORDER BY invocation_started_at DESC LIMIT 1",
    )

    assert completed.returncode == 0, completed.stderr.decode(errors="replace")
    assert recorded == test_case.expected_recorded


@pytest.mark.parametrize(
    "test_case",
    [
        HeldLockTestCase(
            description="a store locked by another process costs at most the budget",
            shutdown_seconds=1.0,
            builds_before_lock=1,
            expected_exit_code=0,
        ),
        HeldLockTestCase(
            description="a store another process holds before setup never delays the start",
            shutdown_seconds=1.0,
            builds_before_lock=0,
            expected_exit_code=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_locked_store_when_building_then_start_and_exit_each_add_at_most_the_budget(
    test_case: HeldLockTestCase, tmp_path: Path
) -> None:
    budget: str = f'\n[recall]\nshutdown_timeout = "{int(test_case.shutdown_seconds * 1000)}ms"\n'
    baseline_dir: Path = prepared_project(tmp_path / "baseline", budget, builds=1)
    project_dir: Path = prepared_project(
        tmp_path / "locked", budget, builds=test_case.builds_before_lock
    )
    baseline_build: partial[tuple[subprocess.CompletedProcess[bytes], float, float]] = partial(
        timed_build, baseline_dir
    )
    locked_build: partial[tuple[subprocess.CompletedProcess[bytes], float, float]] = partial(
        build_while_store_locked, project_dir
    )
    first_delays: list[float] = []
    total_delays: list[float] = []
    # Baseline and locked builds alternate, each pair in the opposite order to the last, so both
    # sides of every difference run under the same load; the medians drop one noisy pair.
    for first_build, second_build in (
        (baseline_build, locked_build),
        (locked_build, baseline_build),
        (baseline_build, locked_build),
    ):
        builds: dict[
            partial[tuple[subprocess.CompletedProcess[bytes], float, float]],
            tuple[subprocess.CompletedProcess[bytes], float, float],
        ] = {first_build: first_build(), second_build: second_build()}
        _, baseline_first, baseline_total = builds[baseline_build]
        locked, locked_first, locked_total = builds[locked_build]
        assert locked.returncode == test_case.expected_exit_code
        assert (
            locked.stderr.decode(errors="replace")
            .rstrip()
            .endswith("completed successfully; only its Recall history is affected.")
        )
        first_delays.append(locked_first - baseline_first)
        total_delays.append(locked_total - baseline_total)

    slack: float = 0.75
    assert statistics.median(first_delays) <= slack
    assert statistics.median(total_delays) <= test_case.shutdown_seconds + slack


@pytest.mark.parametrize(
    "test_case",
    [
        ConcurrentWritersTestCase(
            description="two threads writing both streams keep one global order",
            lines_per_stream=400,
            expected_order=list(range(800)),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_concurrent_stream_writers_when_recalled_then_order_matches_compute_logs(
    test_case: ConcurrentWritersTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop")

    built: subprocess.CompletedProcess[bytes] = run(
        project_dir, "build", ORDERS_TASK_THREAD_LINES=str(test_case.lines_per_stream)
    )
    [[invocation_id, stdout_hex, stderr_hex, combined_hex]] = recall_rows(
        project_dir,
        "SELECT invocation_id, hex(stdout), hex(stderr), hex(combined) FROM output_transcripts",
    )
    numbered: list[int] = numbered_lines(bytes.fromhex(str(combined_hex)))

    assert built.returncode == 0, built.stderr.decode(errors="replace")
    assert (bytes.fromhex(str(stdout_hex)), bytes.fromhex(str(stderr_hex))) == (
        compute_log_streams(project_dir, str(invocation_id))
    )
    assert numbered == test_case.expected_order


@pytest.mark.parametrize(
    "test_case",
    [
        CorruptStoreTestCase(
            description="a store that is not a database is moved aside before any output",
            expected_notice_prefix="Recall: ",
            expected_recorded=[["build", "completed"]],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_corrupt_store_when_building_then_it_is_moved_aside_and_recording_restarts(
    test_case: CorruptStoreTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop")
    write_corrupt_store(project_dir)

    built: subprocess.CompletedProcess[bytes] = run(project_dir, "build")
    again: subprocess.CompletedProcess[bytes] = run(project_dir, "build")
    recorded: list[list[object]] = recall_rows(
        project_dir, "SELECT command, status FROM invocations ORDER BY invocation_started_at"
    )
    moved: list[Path] = list((project_dir / "logs").glob("recall.sqlite3.corrupt-*"))
    first_line: str = built.stderr.decode(errors="replace").splitlines()[0]

    assert (built.returncode, again.returncode) == (0, 0)
    assert first_line.startswith(test_case.expected_notice_prefix)
    assert str(moved[0].name) in first_line
    assert len(moved) == 1
    assert b".corrupt-" not in again.stderr
    assert recorded == [*test_case.expected_recorded, *test_case.expected_recorded]


@pytest.mark.parametrize(
    "test_case",
    [
        FailingStderrTestCase(
            description="a Recall notice that cannot be written keeps the exit code",
            expected_exit_code=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failing_stderr_when_recall_reports_then_the_exit_code_is_unchanged(
    test_case: FailingStderrTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(
        tmp_path / "orders_shop", extra_toml='\n[recall]\nshutdown_timeout = "300ms"\n'
    )
    _ = run(project_dir, "build")

    exit_code: int = build_with_failing_recall_notices(project_dir, tmp_path / "site")

    assert exit_code == test_case.expected_exit_code


@pytest.mark.parametrize(
    "test_case",
    [
        CorruptStoreRaceTestCase(
            description="builds starting together on a corrupt store move it aside exactly once",
            targets=("dev", "dev_1"),
            expected_exit_codes=[0, 0],
            expected_moved=1,
            expected_recorded=[["build", "completed", 2]],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_corrupt_store_when_builds_race_then_one_moves_it_and_both_are_recorded(
    test_case: CorruptStoreRaceTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop", targets=len(test_case.targets))
    _ = run(project_dir, "compile")
    write_corrupt_store(project_dir)

    exit_codes: list[int] = simultaneous_builds(project_dir, test_case.targets)
    moved: list[Path] = list((project_dir / "logs").glob("recall.sqlite3.corrupt-*[0-9Z]"))
    recorded: list[list[object]] = recall_rows(
        project_dir,
        "SELECT command, status, COUNT(*) FROM invocations WHERE command = 'build' "
        "GROUP BY command, status",
    )

    assert exit_codes == test_case.expected_exit_codes
    assert len(moved) == test_case.expected_moved
    assert recorded == test_case.expected_recorded


@pytest.mark.parametrize(
    "test_case",
    [
        CaptureLimitTestCase(
            description="under the default limit stdout is byte-exact",
            extra_toml="",
            stdout_bytes=1536 * 1024,
            expected_kept=(MEBIBYTE * 1024, 0),
            expected_gap=[None, 1],
        ),
        CaptureLimitTestCase(
            description="past the default limit the first and last MiB are kept exactly",
            extra_toml="",
            stdout_bytes=3 * MEBIBYTE,
            expected_kept=(MEBIBYTE, MEBIBYTE),
            expected_gap=["truncated", 0],
        ),
        CaptureLimitTestCase(
            description="a larger target limit keeps everything",
            extra_toml="\n[targets.dev.recall]\nmax_output_bytes = 8388608\n",
            stdout_bytes=3 * MEBIBYTE,
            expected_kept=(MEBIBYTE * 1024, 0),
            expected_gap=[None, 1],
        ),
        CaptureLimitTestCase(
            description="zero records all output",
            extra_toml="\n[recall]\nmax_output_bytes = 0\n",
            stdout_bytes=3 * MEBIBYTE,
            expected_kept=(MEBIBYTE * 1024, 0),
            expected_gap=[None, 1],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_output_limit_when_recorded_then_kept_bytes_are_exact_around_one_gap(
    test_case: CaptureLimitTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop", extra_toml=test_case.extra_toml)

    built: subprocess.CompletedProcess[bytes] = run(
        project_dir, "build", ORDERS_TASK_STDOUT_BYTES=str(test_case.stdout_bytes)
    )
    [[stdout_hex, gap_reason, complete, truncated, warning_first]] = recall_rows(
        project_dir,
        "SELECT hex(t.stdout), o.gap_reason, o.complete, o.truncated_bytes, "
        "instr(CAST(t.combined AS TEXT), 'from the task') < instr(t.combined, 'yyyy') "
        "FROM output_invocations o JOIN output_transcripts t USING (invocation_id)",
    )
    head, tail = test_case.expected_kept
    expected: bytes = built.stdout[:head] + built.stdout[len(built.stdout) - tail :]

    assert built.returncode == 0, built.stderr.decode(errors="replace")
    assert bytes.fromhex(str(stdout_hex)) == expected
    assert [gap_reason, complete] == test_case.expected_gap
    assert truncated == len(built.stdout) - len(expected)
    assert warning_first == 1
    assert b"max_output_bytes" not in built.stdout + built.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        TruncationNoteTestCase(
            description="the note sits where stdout was left out",
            stdout_bytes=3 * MEBIBYTE,
            expected_kept=MEBIBYTE,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_truncated_output_when_shown_then_a_note_marks_the_gap(
    test_case: TruncationNoteTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop")
    built: subprocess.CompletedProcess[bytes] = run(
        project_dir, "build", ORDERS_TASK_STDOUT_BYTES=str(test_case.stdout_bytes)
    )

    shown: subprocess.CompletedProcess[bytes] = run(
        project_dir, "recall", "output", "last", "--stream", "stdout"
    )
    omitted: int = len(built.stdout) - 2 * test_case.expected_kept
    note: bytes = (
        f"\u2026 {omitted / 1_000_000:.1f} MB of stdout not recorded (kept the first and last "
        "1 MiB). Raise [recall] max_output_bytes to keep more.\n"
    ).encode()

    assert shown.returncode == 0, shown.stderr.decode(errors="replace")
    assert shown.stdout == (
        built.stdout[: test_case.expected_kept]
        + b"\n"
        + note
        + built.stdout[len(built.stdout) - test_case.expected_kept :]
    )
    assert shown.stderr == b""


@pytest.mark.parametrize(
    "test_case",
    [
        InvalidOutputLimitTestCase(
            description="a negative limit is a configuration error",
            value="-1",
            expected_message=b"error[D001]",
        ),
        InvalidOutputLimitTestCase(
            description="a size string is a configuration error",
            value='"2MiB"',
            expected_message=b"recall.max_output_bytes must be a whole number of bytes",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_invalid_output_limit_when_running_then_the_command_fails_with_its_code(
    test_case: InvalidOutputLimitTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(
        tmp_path / "orders_shop", extra_toml=f"\n[recall]\nmax_output_bytes = {test_case.value}\n"
    )

    completed: subprocess.CompletedProcess[bytes] = run(project_dir, "build")

    assert completed.returncode != 0
    assert test_case.expected_message in completed.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        UndecodableArgumentTestCase(
            description="an undecodable selector is recorded escaped; the error prints it escaped",
            selector="orders\udcff",
            expected_error=(
                b"error[S007]: unknown selector name 'orders\\xff'\n  = help: did you mean 'orders'?"
            ),
            expected_recorded=[['["orders\\\\xff"]', 1]],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_undecodable_argument_when_recorded_then_it_is_escaped_in_history_and_error(
    test_case: UndecodableArgumentTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop")

    completed: subprocess.CompletedProcess[bytes] = run(
        project_dir, "compile", "--json", "--select", test_case.selector
    )
    recorded: list[list[object]] = recall_rows(
        project_dir,
        "SELECT i.selection, json_extract(e.payload, '$.selection_count') "
        "FROM invocations i JOIN events e USING (invocation_id) "
        "WHERE e.event_type = 'invocation_started'",
    )

    assert completed.returncode == 1
    assert completed.stderr.rstrip().endswith(test_case.expected_error)
    assert recorded == test_case.expected_recorded
