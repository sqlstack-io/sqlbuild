"""DuckDB E2E coverage for capped microbatch dependency graphs."""

import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    CalendarGrainCursorBoundaryE2ETestCase,
    CappedFilterJoinE2ETestCase,
    CappedMicrobatchBuildE2ETestCase,
    CappedMicrobatchScenarioE2ETestCase,
    CappedWatermarkRejectionE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import (
    capped_filter_join_consumer_project_files,
    capped_microbatch_project_files,
    capped_watermark_consumer_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    (
        CalendarGrainCursorBoundaryE2ETestCase(
            description="monthly mid-bucket maximum caps daily consumer at calendar boundary",
            expected_batch_starts=tuple(f"2026-07-{day:02d} 00:00:00" for day in range(25, 32)),
            expected_exclusive_end="2026-08-01 00:00:00",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_monthly_producer_with_midmonth_max_when_building_capped_daily_consumer_then_no_future_batches_run(
    tmp_path: Path,
    test_case: CalendarGrainCursorBoundaryE2ETestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="calendar_cursor_boundary",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "calendar_cursor_boundary"
                adapter = "duckdb"

                [connection]
                database = "regression.duckdb"
                """
            ).strip()
            + "\n",
            "sources/raw.yml": (
                "sources:\n  - name: raw_meetings\n    description: Test source raw_meetings.\n    schema: main\n    table: raw_meetings\n"
            ),
            "models/monthly_meetings.sql": dedent(
                """
                MODEL (description "Test model monthly_meetings.",
                  materialized incremental,
                  incremental_strategy delete_insert,
                  incremental_mode microbatch,
                  microbatch_strategy watermark,
                  cursor_watermark_mode all,
                  cursor meeting_date,
                  cursor_type timestamp,
                  cursor_grain month,
                  cursor_start '2026-07-01',
                  cursor_inputs (
                    raw_meetings (column meeting_date, roles [filter, watermark]),
                  ),
                  batch_size 1mo,
                );
                SELECT meeting_date FROM __source("raw_meetings")
                """
            ).strip()
            + "\n",
            "models/daily_suffix.sql": dedent(
                """
                MODEL (description "Test model daily_suffix.",
                  materialized incremental,
                  incremental_strategy delete_insert,
                  incremental_mode microbatch,
                  microbatch_strategy watermark,
                  cursor_watermark_mode all,
                  cursor batch_start,
                  cursor_type timestamp,
                  cursor_grain day,
                  cursor_start '2026-07-01',
                  cursor_inputs (
                    monthly_meetings (column meeting_date, roles [watermark]),
                  ),
                  batch_size 1d,
                  microbatch_limit (max_batches 7, action cap_from_end),
                );
                WITH meetings AS (SELECT COUNT(*) AS row_count FROM __ref("monthly_meetings"))
                SELECT
                  CAST(__cursor_start() AS TIMESTAMP) AS batch_start,
                  CAST(__cursor_end() AS TIMESTAMP) AS batch_end
                FROM meetings
                """
            ).strip()
            + "\n",
        },
    )
    db_path: Path = project_dir / "regression.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute("CREATE TABLE raw_meetings (meeting_date TIMESTAMP)")
    connection.execute("INSERT INTO raw_meetings VALUES ('2026-07-15 12:00:00')")
    connection.close()

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT CAST(batch_start AS VARCHAR), CAST(batch_end AS VARCHAR) "
        "FROM main.daily_suffix ORDER BY batch_start",
    )
    assert tuple(str(row[0]) for row in rows) == test_case.expected_batch_starts
    assert all(str(row[1]) <= test_case.expected_exclusive_end for row in rows)
    assert rows[-1][1] == test_case.expected_exclusive_end
    assert query_duckdb(
        db_path=db_path,
        sql="SELECT COUNT(*) FROM information_schema.tables "
        "WHERE table_name = '_sqlbuild_microbatches'",
    ) == [(0,)]


@pytest.mark.parametrize(
    "test_case",
    (
        CappedMicrobatchBuildE2ETestCase(
            description="cap from start advances deferred producer prefix",
            limit_action="cap_from_start",
            expected_first_ids=(1, 2, 3),
            expected_final_ids=(1, 2, 3, 4, 5),
            run_count=3,
        ),
        CappedMicrobatchBuildE2ETestCase(
            description="cap from end materializes only latest suffix",
            limit_action="cap_from_end",
            expected_first_ids=(3, 4, 5),
            expected_final_ids=(3, 4, 5),
            run_count=2,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_external_watermark_and_capped_terminal_model_when_building_then_graph_stays_bounded(
    tmp_path: Path,
    test_case: CappedMicrobatchBuildE2ETestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="capped_microbatch",
        repo_files=capped_microbatch_project_files(limit_action=test_case.limit_action),
    )
    db_path: Path = project_dir / "regression.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute("CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP)")
    connection.execute(
        "INSERT INTO raw_events VALUES "
        "(1, '2026-01-01 12:00:00'), (2, '2026-01-02 12:00:00'), "
        "(3, '2026-01-03 12:00:00'), (4, '2026-01-04 12:00:00'), "
        "(5, '2026-01-05 12:00:00')"
    )
    connection.close()

    first_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert first_result.returncode == 0, first_result.stdout + first_result.stderr
    first_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT id FROM main.downstream_events ORDER BY id",
    )
    assert tuple(row[0] for row in first_rows) == test_case.expected_first_ids
    for _ in range(test_case.run_count - 1):
        repeated_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build"), project_dir=project_dir
        )
        assert repeated_result.returncode == 0, repeated_result.stdout + repeated_result.stderr

    final_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT id FROM main.downstream_events ORDER BY id",
    )
    assert tuple(row[0] for row in final_rows) == test_case.expected_final_ids


@pytest.mark.parametrize(
    "test_case",
    (CappedMicrobatchScenarioE2ETestCase("project warning precedes cap", 0),),
    ids=lambda case: case.description,
)
def test_given_project_warning_outside_model_cap_when_building_then_safety_warns_before_cap_executes(
    tmp_path: Path, test_case: CappedMicrobatchScenarioE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="project_limit_with_model_cap",
        repo_files=capped_microbatch_project_files(
            limit_action="cap_from_end",
            project_limit='[microbatches.limits]\nmax_batches = 2\naction = "warn"',
        ),
    )
    db_path: Path = project_dir / "regression.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute("CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP)")
    connection.execute(
        "INSERT INTO raw_events VALUES "
        "(1, '2026-01-01 12:00:00'), (2, '2026-01-02 12:00:00'), "
        "(3, '2026-01-03 12:00:00'), (4, '2026-01-04 12:00:00'), "
        "(5, '2026-01-05 12:00:00')"
    )
    connection.close()

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert "above the per-model limit of 2 (action=warn)" in result.stdout
    assert "above the per-model limit of 3 (action=cap_from_end)" in result.stdout
    rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT id FROM main.downstream_events ORDER BY id",
    )
    assert tuple(row[0] for row in rows) == (3, 4, 5)


@pytest.mark.parametrize(
    "test_case",
    (CappedMicrobatchScenarioE2ETestCase("CLI limit remains hard", 1),),
    ids=lambda case: case.description,
)
def test_given_cli_hard_limit_over_model_cap_when_building_then_invocation_fails_before_execution(
    tmp_path: Path, test_case: CappedMicrobatchScenarioE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="cli_limit_over_model_cap",
        repo_files=capped_microbatch_project_files(limit_action="cap_from_end"),
    )
    db_path: Path = project_dir / "regression.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute("CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP)")
    connection.execute(
        "INSERT INTO raw_events VALUES "
        "(1, '2026-01-01 12:00:00'), (2, '2026-01-02 12:00:00'), "
        "(3, '2026-01-03 12:00:00'), (4, '2026-01-04 12:00:00'), "
        "(5, '2026-01-05 12:00:00')"
    )
    connection.close()

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--max-microbatches", "2"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code
    assert "above the per-model limit of 2 (action=error)" in result.stderr
    connection = duckdb.connect(str(db_path))
    try:
        target_names: set[str] = {
            row[0]
            for row in connection.execute(
                "SELECT table_name FROM information_schema.tables WHERE table_schema = 'main'"
            ).fetchall()
        }
    finally:
        connection.close()
    assert "capped_events" not in target_names
    assert "downstream_events" not in target_names


@pytest.mark.parametrize(
    "test_case",
    (CappedMicrobatchScenarioE2ETestCase("watermark jump preserves gap", 0),),
    ids=lambda case: case.description,
)
def test_given_cap_from_end_watermark_jump_when_rebuilding_then_producer_leaves_disjoint_gap_untouched(
    tmp_path: Path, test_case: CappedMicrobatchScenarioE2ETestCase
) -> None:
    project_files: dict[str, str] = capped_microbatch_project_files(limit_action="cap_from_end")
    project_files["models/capped_events.sql"] = project_files["models/capped_events.sql"].replace(
        "cursor_end '2026-01-06'", "cursor_end '2026-01-12'"
    )
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="cap_from_end_watermark_jump",
        repo_files=project_files,
    )
    db_path: Path = project_dir / "regression.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute("CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP)")
    connection.execute(
        "INSERT INTO raw_events VALUES "
        "(1, '2026-01-01 12:00:00'), (2, '2026-01-02 12:00:00'), "
        "(3, '2026-01-03 12:00:00'), (4, '2026-01-04 12:00:00'), "
        "(5, '2026-01-05 12:00:00')"
    )
    connection.close()

    initial: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert initial.returncode == test_case.expected_exit_code, initial.stdout + initial.stderr

    connection = duckdb.connect(str(db_path))
    connection.execute(
        "INSERT INTO raw_events VALUES "
        "(9, '2026-01-09 12:00:00'), (10, '2026-01-10 12:00:00'), "
        "(11, '2026-01-11 12:00:00')"
    )
    connection.close()
    jumped: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert jumped.returncode == test_case.expected_exit_code, jumped.stdout + jumped.stderr

    rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT id FROM main.capped_events ORDER BY id",
    )
    assert tuple(row[0] for row in rows) == (3, 4, 5, 9, 10, 11)
    gap_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT id FROM main.capped_events "
            "WHERE event_time >= '2026-01-06' AND event_time < '2026-01-09'"
        ),
    )
    assert gap_rows == []


@pytest.mark.parametrize(
    "test_case",
    (CappedMicrobatchScenarioE2ETestCase("bounded any uses uncapped input", 0),),
    ids=lambda case: case.description,
)
def test_given_any_watermarks_and_consumer_end_when_capped_winner_is_later_then_uncapped_input_runs(
    tmp_path: Path, test_case: CappedMicrobatchScenarioE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="capped_any_cursor_end",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "capped_any_cursor_end"
                adapter = "duckdb"

                [connection]
                database = "regression.duckdb"
                """
            ).strip()
            + "\n",
            "sources/raw.yml": dedent(
                """
                sources:
                  - name: raw_late
                    description: Test source raw_late.
                    schema: main
                    table: raw_late
                  - name: raw_early
                    description: Test source raw_early.
                    schema: main
                    table: raw_early
                """
            ).strip()
            + "\n",
            "models/capped_late.sql": dedent(
                """
                MODEL (description "Test model capped_late.",
                  materialized incremental,
                  incremental_strategy delete_insert,
                  incremental_mode microbatch,
                  microbatch_strategy watermark,
                  cursor event_time,
                  cursor_type timestamp,
                  cursor_grain day,
                  cursor_start '2026-01-18',
                  cursor_end '2026-01-25',
                  cursor_watermark_mode all,
                  cursor_inputs (
                    raw_late (column event_time, roles [filter, watermark]),
                  ),
                  batch_size 1d,
                  lookback 1d,
                  microbatch_limit (
                    max_batches 3,
                    action cap_from_end,
                  ),
                );
                SELECT id, event_time FROM __source("raw_late")
                """
            ).strip()
            + "\n",
            "models/uncapped_early.sql": dedent(
                """
                MODEL (description "Test model uncapped_early.", materialized table);
                SELECT id, event_time FROM __source("raw_early")
                """
            ).strip()
            + "\n",
            "models/any_consumer.sql": dedent(
                """
                MODEL (description "Test model any_consumer.",
                  materialized incremental,
                  incremental_strategy delete_insert,
                  incremental_mode microbatch,
                  microbatch_strategy watermark,
                  cursor event_time,
                  cursor_type timestamp,
                  cursor_grain day,
                  cursor_start '2026-01-01',
                  cursor_end '2026-01-15',
                  cursor_watermark_mode any,
                  cursor_inputs (
                    capped_late (column event_time, roles [filter]),
                    uncapped_early (column event_time, roles [filter, watermark]),
                  ),
                  batch_size 1d,
                  lookback 1d,
                );
                SELECT id, event_time FROM __ref("capped_late")
                UNION ALL
                SELECT id, event_time FROM __ref("uncapped_early")
                """
            ).strip()
            + "\n",
        },
    )
    db_path: Path = project_dir / "regression.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute("CREATE TABLE raw_late (id INTEGER, event_time TIMESTAMP)")
    connection.execute(
        "INSERT INTO raw_late VALUES "
        "(18, '2026-01-18 12:00:00'), (19, '2026-01-19 12:00:00'), "
        "(20, '2026-01-20 12:00:00')"
    )
    connection.execute("CREATE TABLE raw_early (id INTEGER, event_time TIMESTAMP)")
    connection.execute(
        "INSERT INTO raw_early VALUES (101, '2026-01-01 12:00:00'), (102, '2026-01-14 12:00:00')"
    )
    connection.close()

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT id FROM main.any_consumer ORDER BY id",
    )
    assert tuple(row[0] for row in rows) == (101, 102)


@pytest.mark.parametrize(
    "test_case",
    (
        CappedWatermarkRejectionE2ETestCase(
            description="capped producer watermark input is rejected before DML",
            expected_exit_code=1,
            expected_error_fragment=(
                "model 'downstream_events' uses capped producer 'capped_events' as a "
                "watermark input; capped producers cannot serve as watermark inputs"
            ),
            expected_absent_relations=("capped_events", "downstream_events"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_capped_producer_as_watermark_when_building_then_plan_rejects_before_dml(
    tmp_path: Path, test_case: CappedWatermarkRejectionE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="capped_watermark_rejection",
        repo_files=capped_watermark_consumer_project_files(limit_action="cap_from_end"),
    )
    db_path: Path = project_dir / "regression.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute("CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP)")
    connection.execute(
        "INSERT INTO raw_events VALUES (4, '2026-01-04 12:00:00'), (5, '2026-01-05 12:00:00')"
    )
    connection.close()

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code
    assert test_case.expected_error_fragment in result.stdout + result.stderr
    connection = duckdb.connect(str(db_path))
    try:
        target_names: set[str] = {
            row[0]
            for row in connection.execute(
                "SELECT table_name FROM information_schema.tables WHERE table_schema = 'main'"
            ).fetchall()
        }
    finally:
        connection.close()
    assert target_names.isdisjoint(test_case.expected_absent_relations)


@pytest.mark.parametrize(
    "test_case",
    (
        CappedFilterJoinE2ETestCase(
            description="join using the shared id column of a real source table",
            limit_action="cap_from_end",
            expected_ids=(3, 4, 5),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_capped_filter_consumer_joining_source_table_using_id_when_building_then_join_binds(
    tmp_path: Path, test_case: CappedFilterJoinE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="capped_filter_join",
        repo_files=capped_filter_join_consumer_project_files(limit_action=test_case.limit_action),
    )
    db_path: Path = project_dir / "regression.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute("CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP)")
    connection.execute(
        "INSERT INTO raw_events VALUES "
        "(1, '2026-01-01 12:00:00'), (2, '2026-01-02 12:00:00'), "
        "(3, '2026-01-03 12:00:00'), (4, '2026-01-04 12:00:00'), "
        "(5, '2026-01-05 12:00:00')"
    )
    connection.close()

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    assert "B002" not in result.stdout + result.stderr
    rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT id FROM main.downstream_events ORDER BY id",
    )
    assert tuple(row[0] for row in rows) == test_case.expected_ids


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
