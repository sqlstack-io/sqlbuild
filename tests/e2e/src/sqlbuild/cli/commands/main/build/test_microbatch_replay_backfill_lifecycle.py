"""E2E coverage for difficult direct microbatch backfill and replay transitions."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    ConcurrentMicrobatchBehaviorE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import (
    prepare_replay_microbatch_project,
    timestamp_microbatch_model_sql,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    query_duckdb,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ConcurrentMicrobatchBehaviorE2ETestCase(
            description="ahead of frontier partial backfill repairs only continuity gap"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_ahead_of_frontier_partial_backfill_when_normal_run_resumes_then_leading_gap_recovers(
    test_case: ConcurrentMicrobatchBehaviorE2ETestCase, tmp_path: Path
) -> None:
    project_dir, db_path = prepare_replay_microbatch_project(
        tmp_path=tmp_path,
        project_name="microbatch_future_backfill",
        database_name="future_backfill.duckdb",
        replay_policy="forward",
    )
    execute_duckdb(
        db_path=db_path,
        sql=(
            "CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP, payload VARCHAR); "
            "INSERT INTO raw_events VALUES (1, '2026-01-01 00:30:00', '1')"
        ),
    )
    initial: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    execute_duckdb(
        db_path=db_path,
        sql=(
            "INSERT INTO raw_events VALUES "
            "(2, '2026-01-01 01:30:00', '2'), "
            "(3, '2026-01-01 02:30:00', 'bad'), "
            "(4, '2026-01-01 03:30:00', '4')"
        ),
    )
    backfill: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--start-cursor-ts",
            "2026-01-01T01:00:00",
            "--end-cursor-ts",
            "2026-01-01T04:00:00",
        ),
        project_dir=project_dir,
    )
    execute_duckdb(
        db_path=db_path,
        sql="UPDATE raw_events SET payload = '3' WHERE id = 3",
    )
    resumed: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    assert initial.returncode == test_case.expected_exit_code, initial.stdout + initial.stderr
    assert backfill.returncode != test_case.expected_exit_code
    assert resumed.returncode == test_case.expected_exit_code, resumed.stdout + resumed.stderr
    assert query_duckdb(
        db_path=db_path,
        sql="SELECT id, value FROM main.orders ORDER BY id",
    ) == [(1, 1), (2, 2), (3, 3), (4, 4)]
    assert (
        query_duckdb(
            db_path=db_path,
            sql=(
                "SELECT COUNT(*) FROM main._sqlbuild_microbatches "
                "WHERE record_type = 'partition_completion' "
                "AND run_type = 'backfill' AND completion_type = 'recovery'"
            ),
        )[0][0]
        >= 1
    )
    assert query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT COUNT(*) FROM main._sqlbuild_microbatches "
            "WHERE record_type = 'replay_requirement'"
        ),
    ) == [(0,)]


@pytest.mark.parametrize(
    "test_case",
    [
        ConcurrentMicrobatchBehaviorE2ETestCase(
            description="failed backfill tail remains a one shot operator request"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_failed_backfill_tail_when_source_tail_disappears_then_no_durable_retry_is_created(
    test_case: ConcurrentMicrobatchBehaviorE2ETestCase, tmp_path: Path
) -> None:
    project_dir, db_path = prepare_replay_microbatch_project(
        tmp_path=tmp_path,
        project_name="microbatch_backfill_tail",
        database_name="backfill_tail.duckdb",
        replay_policy="forward",
    )
    execute_duckdb(
        db_path=db_path,
        sql=(
            "CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP, payload VARCHAR); "
            "INSERT INTO raw_events VALUES (1, '2026-01-01 00:30:00', '1')"
        ),
    )
    initial: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    execute_duckdb(
        db_path=db_path,
        sql=(
            "INSERT INTO raw_events VALUES "
            "(2, '2026-01-01 01:30:00', '2'), "
            "(3, '2026-01-01 02:30:00', 'bad'), "
            "(4, '2026-01-01 03:30:00', 'bad')"
        ),
    )
    backfill: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--start-cursor-ts",
            "2026-01-01T01:00:00",
            "--end-cursor-ts",
            "2026-01-01T03:00:00",
        ),
        project_dir=project_dir,
    )
    execute_duckdb(db_path=db_path, sql="DELETE FROM raw_events WHERE id IN (3, 4)")
    resumed: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    assert initial.returncode == test_case.expected_exit_code, initial.stdout + initial.stderr
    assert backfill.returncode != test_case.expected_exit_code
    assert resumed.returncode == test_case.expected_exit_code, resumed.stdout + resumed.stderr
    assert query_duckdb(
        db_path=db_path,
        sql="SELECT id, value FROM main.orders ORDER BY id",
    ) == [(1, 1), (2, 2)]
    assert query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT COUNT(*) FROM main._sqlbuild_microbatches "
            "WHERE record_type = 'replay_requirement'"
        ),
    ) == [(0,)]
    assert query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT COUNT(*) FROM main._sqlbuild_microbatches "
            "WHERE record_type = 'partition_completion' "
            "AND run_type = 'backfill' AND completion_type = 'recovery'"
        ),
    ) == [(0,)]


@pytest.mark.parametrize(
    "test_case",
    [
        ConcurrentMicrobatchBehaviorE2ETestCase(
            description="partial bounded replay retains fixed bounds and resumes missing version"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_partial_bounded_replay_and_later_upstream_progress_when_retried_then_requirement_does_not_slide(
    test_case: ConcurrentMicrobatchBehaviorE2ETestCase, tmp_path: Path
) -> None:
    project_dir, db_path = prepare_replay_microbatch_project(
        tmp_path=tmp_path,
        project_name="microbatch_partial_replay",
        database_name="partial_replay.duckdb",
        replay_policy="bounded-4h",
    )
    execute_duckdb(
        db_path=db_path,
        sql=(
            "CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP, payload VARCHAR); "
            "INSERT INTO raw_events VALUES "
            "(1, '2026-01-01 00:30:00', '1'), "
            "(2, '2026-01-01 01:30:00', '2'), "
            "(3, '2026-01-01 02:30:00', '3'), "
            "(4, '2026-01-01 03:30:00', '4')"
        ),
    )
    initial: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    execute_duckdb(db_path=db_path, sql="UPDATE raw_events SET payload = 'bad' WHERE id = 2")
    f2_sql: str = timestamp_microbatch_model_sql(
        value_expression="CAST(payload AS INTEGER) + 10",
        batch_concurrency=3,
        replay_policy="bounded-4h",
    )
    (project_dir / "sql" / "models" / "orders.sql").write_text(f2_sql, encoding="utf-8")
    partial: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    requirement_before: tuple[object, ...] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT replay_requirement_id, run_start, run_end, required_model_version_hash "
            "FROM main._sqlbuild_microbatches WHERE record_type = 'replay_requirement' "
            "ORDER BY created_at DESC LIMIT 1"
        ),
    )[0]
    execute_duckdb(
        db_path=db_path,
        sql=(
            "UPDATE raw_events SET payload = '2' WHERE id = 2; "
            "INSERT INTO raw_events VALUES (5, '2026-01-02 12:30:00', '5')"
        ),
    )
    retried: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    requirement_after: tuple[object, ...] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT replay_requirement_id, run_start, run_end, required_model_version_hash "
            "FROM main._sqlbuild_microbatches WHERE record_type = 'replay_requirement' "
            "ORDER BY created_at DESC LIMIT 1"
        ),
    )[0]
    advanced: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    assert initial.returncode == test_case.expected_exit_code, initial.stdout + initial.stderr
    assert partial.returncode != test_case.expected_exit_code
    assert retried.returncode == test_case.expected_exit_code, retried.stdout + retried.stderr
    assert advanced.returncode == test_case.expected_exit_code, advanced.stdout + advanced.stderr
    assert requirement_after == requirement_before
    assert query_duckdb(
        db_path=db_path,
        sql="SELECT id, value FROM main.orders ORDER BY id",
    ) == [(1, 11), (2, 12), (3, 13), (4, 14), (5, 15)]
    assert query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT COUNT(DISTINCT replay_requirement_id) "
            "FROM main._sqlbuild_microbatches WHERE record_type = 'replay_requirement'"
        ),
    ) == [(1,)]


@pytest.mark.parametrize(
    "test_case",
    [
        ConcurrentMicrobatchBehaviorE2ETestCase(
            description="all failed F3 requirement supersedes all failed F2 before returning to F2"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_all_failed_f2_and_f3_when_code_returns_to_f2_then_old_requirement_is_not_resurrected(
    test_case: ConcurrentMicrobatchBehaviorE2ETestCase, tmp_path: Path
) -> None:
    project_dir, db_path = prepare_replay_microbatch_project(
        tmp_path=tmp_path,
        project_name="microbatch_all_failed_supersession",
        database_name="all_failed_supersession.duckdb",
        replay_policy="bounded-2h",
    )
    execute_duckdb(
        db_path=db_path,
        sql=(
            "CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP, payload VARCHAR); "
            "INSERT INTO raw_events VALUES "
            "(1, '2026-01-01 00:30:00', '1'), "
            "(2, '2026-01-01 01:30:00', '2')"
        ),
    )
    initial: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    execute_duckdb(db_path=db_path, sql="UPDATE raw_events SET payload = 'bad'")
    f2_sql: str = timestamp_microbatch_model_sql(
        value_expression="CAST(payload AS INTEGER) + 10",
        batch_concurrency=3,
        replay_policy="bounded-2h",
    )
    f3_sql: str = timestamp_microbatch_model_sql(
        value_expression="CAST(payload AS INTEGER) + 20",
        batch_concurrency=3,
        replay_policy="bounded-2h",
    )
    model_path: Path = project_dir / "sql" / "models" / "orders.sql"
    model_path.write_text(f2_sql, encoding="utf-8")
    failed_f2: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    model_path.write_text(f3_sql, encoding="utf-8")
    failed_f3: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    model_path.write_text(f2_sql, encoding="utf-8")
    returned_f2: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    assert initial.returncode == test_case.expected_exit_code, initial.stdout + initial.stderr
    assert failed_f2.returncode != test_case.expected_exit_code
    assert failed_f3.returncode != test_case.expected_exit_code
    assert returned_f2.returncode != test_case.expected_exit_code
    requirement_counts: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT required_model_version_hash, COUNT(DISTINCT replay_requirement_id) "
            "FROM main._sqlbuild_microbatches WHERE record_type = 'replay_requirement' "
            "GROUP BY required_model_version_hash ORDER BY COUNT(*) DESC"
        ),
    )
    assert sorted(int(str(count)) for _version, count in requirement_counts) == [1, 2]


@pytest.mark.parametrize(
    "test_case",
    [
        ConcurrentMicrobatchBehaviorE2ETestCase(
            description="formatting-only edit keeps the pending replay requirement"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_pending_replay_when_model_is_only_reformatted_then_replay_still_completes(
    test_case: ConcurrentMicrobatchBehaviorE2ETestCase, tmp_path: Path
) -> None:
    project_dir, db_path = prepare_replay_microbatch_project(
        tmp_path=tmp_path,
        project_name="microbatch_reformatted_replay",
        database_name="reformatted_replay.duckdb",
        replay_policy="bounded-4h",
    )
    execute_duckdb(
        db_path=db_path,
        sql=(
            "CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP, payload VARCHAR); "
            "INSERT INTO raw_events VALUES "
            "(1, '2026-01-01 00:30:00', '1'), "
            "(2, '2026-01-01 01:30:00', '2'), "
            "(3, '2026-01-01 02:30:00', '3')"
        ),
    )
    initial: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    execute_duckdb(db_path=db_path, sql="UPDATE raw_events SET payload = 'bad' WHERE id = 2")
    f2_sql: str = timestamp_microbatch_model_sql(
        value_expression="CAST(payload AS INTEGER) + 10",
        batch_concurrency=3,
        replay_policy="bounded-4h",
    )
    model_path: Path = project_dir / "sql" / "models" / "orders.sql"
    model_path.write_text(f2_sql, encoding="utf-8")
    partial: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    execute_duckdb(db_path=db_path, sql="UPDATE raw_events SET payload = '2' WHERE id = 2")
    reformatted: str = f2_sql.replace(
        "SELECT id, event_time,", "-- Orders with a shifted value.\nselect\n  id,\n  event_time,"
    )
    model_path.write_text(reformatted, encoding="utf-8")
    retried: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    assert initial.returncode == test_case.expected_exit_code, initial.stdout + initial.stderr
    assert partial.returncode != test_case.expected_exit_code
    assert retried.returncode == test_case.expected_exit_code, retried.stdout + retried.stderr
    assert query_duckdb(
        db_path=db_path,
        sql="SELECT id, value FROM main.orders ORDER BY id",
    ) == [(1, 11), (2, 12), (3, 13)]
    assert query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT COUNT(DISTINCT replay_requirement_id) "
            "FROM main._sqlbuild_microbatches WHERE record_type = 'replay_requirement'"
        ),
    ) == [(1,)]


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
