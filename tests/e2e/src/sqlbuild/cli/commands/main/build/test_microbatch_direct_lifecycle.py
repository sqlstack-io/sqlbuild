"""E2E coverage for direct microbatch state lifecycle invariants."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    ConcurrentMicrobatchBehaviorE2ETestCase,
    StatelessSerialMicrobatchE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import (
    direct_microbatch_project_toml,
    raw_events_source_yml,
    timestamp_microbatch_model_sql,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [
        StatelessSerialMicrobatchE2ETestCase(
            description="concurrency capability disabled remains stateless",
            settings_toml="",
            expected_complexity_warning_count=0,
            expected_state_table_count=0,
        ),
        StatelessSerialMicrobatchE2ETestCase(
            description="capability enabled with model ceiling one remains stateless",
            settings_toml=("\n[settings]\nconcurrency = 3\nmicrobatch_concurrency = true\n"),
            expected_complexity_warning_count=0,
            expected_state_table_count=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_serial_microbatch_when_building_then_state_table_is_not_used(
    test_case: StatelessSerialMicrobatchE2ETestCase, tmp_path: Path
) -> None:
    project_name: str = "serial_microbatch_ledger"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name=project_name,
        repo_files={
            "sqlbuild_project.toml": direct_microbatch_project_toml(
                project_name=project_name,
                database_name="serial.duckdb",
                settings_toml=test_case.settings_toml,
            ),
            "sql/sources/raw.yml": raw_events_source_yml(),
            "sql/models/orders.sql": timestamp_microbatch_model_sql(
                value_expression="payload",
                batch_concurrency=1,
                replay_policy="forward",
            ),
        },
    )
    db_path: Path = project_dir / "serial.duckdb"
    execute_duckdb(
        db_path=db_path,
        sql=(
            "CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP, payload VARCHAR); "
            "INSERT INTO raw_events VALUES "
            "(1, '2026-01-01 00:15:00', 'a'), "
            "(2, '2026-01-01 02:15:00', 'b')"
        ),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    output: str = result.stdout + result.stderr

    assert result.returncode == 0, output
    assert (
        output.count("Concurrent microbatch execution is enabled")
        == test_case.expected_complexity_warning_count
    )
    assert query_duckdb(
        db_path=db_path,
        sql="SELECT id, value FROM main.orders ORDER BY id",
    ) == [(1, "a"), (2, "b")]
    maximum_timestamps: tuple[object, ...] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT (SELECT MAX(event_time) FROM main.orders), "
            "(SELECT MAX(event_time) FROM main.raw_events)"
        ),
    )[0]
    assert maximum_timestamps[0] == maximum_timestamps[1]
    state_table_count: int = int(
        query_duckdb(
            db_path=db_path,
            sql=(
                "SELECT COUNT(*) FROM information_schema.tables "
                "WHERE table_schema = 'main' AND table_name = '_sqlbuild_microbatches'"
            ),
        )[0][0]
    )
    assert state_table_count == test_case.expected_state_table_count


@pytest.mark.parametrize(
    "test_case",
    [
        ConcurrentMicrobatchBehaviorE2ETestCase(
            description="integer microbatch includes the current maximum in concurrent work"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_integer_cursor_maximum_when_building_then_half_open_batches_include_final_value(
    test_case: ConcurrentMicrobatchBehaviorE2ETestCase, tmp_path: Path
) -> None:
    project_name: str = "integer_microbatch_maximum"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name=project_name,
        repo_files={
            "sqlbuild_project.toml": direct_microbatch_project_toml(
                project_name=project_name,
                database_name="integer_maximum.duckdb",
                settings_toml=("\n[settings]\nconcurrency = 3\nmicrobatch_concurrency = true\n"),
            ),
            "sql/sources/raw.yml": raw_events_source_yml(),
            "sql/models/orders.sql": (
                "MODEL (description 'Test model orders.',\n"
                "  materialized incremental,\n"
                "  incremental_strategy delete_insert,\n"
                "  incremental_mode microbatch,\n"
                "  microbatch_strategy watermark,\n"
                "  cursor_watermark_mode all,\n"
                "  cursor id,\n"
                "  cursor_type integer,\n"
                "  cursor_inputs (raw_events (column id, roles [filter, watermark]),),\n"
                '  batch_size "2",\n'
                "  batch_concurrency 3,\n"
                ");\n\n"
                "SELECT id, payload AS value\n"
                'FROM __source("raw_events")\n'
                "WHERE id >= __cursor_start() AND id < __cursor_end()\n"
            ),
        },
    )
    db_path: Path = project_dir / "integer_maximum.duckdb"
    execute_duckdb(
        db_path=db_path,
        sql=(
            "CREATE TABLE raw_events (id INTEGER, payload VARCHAR); "
            "INSERT INTO raw_events VALUES (0, 'a'), (1, 'b'), (2, 'c')"
        ),
    )
    initial: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    execute_duckdb(
        db_path=db_path,
        sql="INSERT INTO raw_events VALUES (3, 'd'), (4, 'e')",
    )

    incremental: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    assert initial.returncode == test_case.expected_exit_code, initial.stdout + initial.stderr
    assert incremental.returncode == test_case.expected_exit_code, (
        incremental.stdout + incremental.stderr
    )
    assert query_duckdb(
        db_path=db_path,
        sql="SELECT id, value FROM main.orders ORDER BY id",
    ) == [(0, "a"), (1, "b"), (2, "c"), (3, "d"), (4, "e")]
    assert (
        query_duckdb(
            db_path=db_path,
            sql=(
                "SELECT MAX(CAST(partition_end AS BIGINT)) "
                "FROM main._sqlbuild_microbatches "
                "WHERE record_type = 'partition_completion' AND cursor_type = 'integer'"
            ),
        )[0][0]
        > 4
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ConcurrentMicrobatchBehaviorE2ETestCase(
            description="first run and full refresh serialize target bootstrap"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_concurrent_ceiling_when_first_run_and_full_refresh_then_target_bootstrap_is_safe(
    test_case: ConcurrentMicrobatchBehaviorE2ETestCase, tmp_path: Path
) -> None:
    project_name: str = "microbatch_bootstrap_safety"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name=project_name,
        repo_files={
            "sqlbuild_project.toml": direct_microbatch_project_toml(
                project_name=project_name,
                database_name="bootstrap.duckdb",
                settings_toml=("\n[settings]\nconcurrency = 3\nmicrobatch_concurrency = true\n"),
            ),
            "sql/sources/raw.yml": raw_events_source_yml(),
            "sql/models/orders.sql": timestamp_microbatch_model_sql(
                value_expression="payload",
                batch_concurrency=3,
                replay_policy="forward",
            ),
        },
    )
    db_path: Path = project_dir / "bootstrap.duckdb"
    execute_duckdb(
        db_path=db_path,
        sql=(
            "CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP, payload VARCHAR); "
            "INSERT INTO raw_events VALUES "
            "(1, '2026-01-01 00:30:00', 'a'), "
            "(2, '2026-01-01 01:30:00', 'b'), "
            "(3, '2026-01-01 02:30:00', 'c')"
        ),
    )

    initial: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    execute_duckdb(
        db_path=db_path,
        sql=(
            "INSERT INTO raw_events VALUES "
            "(4, '2026-01-01 03:30:00', 'd'), "
            "(5, '2026-01-01 04:30:00', 'e')"
        ),
    )
    refreshed: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--full-refresh"), project_dir=project_dir
    )

    assert initial.returncode == test_case.expected_exit_code, initial.stdout + initial.stderr
    assert refreshed.returncode == test_case.expected_exit_code, refreshed.stdout + refreshed.stderr
    assert "Concurrent microbatch execution is enabled" not in (
        initial.stdout + initial.stderr + refreshed.stdout + refreshed.stderr
    )
    assert query_duckdb(
        db_path=db_path,
        sql="SELECT id, value FROM main.orders ORDER BY id",
    ) == [(1, "a"), (2, "b"), (3, "c"), (4, "d"), (5, "e")]
    assert query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT COUNT(*) FROM information_schema.tables "
            "WHERE table_schema = 'main' AND table_name LIKE 'orders__delta%'"
        ),
    ) == [(0,)]
    assert (
        query_duckdb(
            db_path=db_path,
            sql=(
                "SELECT COUNT(DISTINCT physical_generation_id) "
                "FROM main._sqlbuild_microbatches "
                "WHERE record_type = 'partition_completion'"
            ),
        )[0][0]
        >= 2
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ConcurrentMicrobatchBehaviorE2ETestCase(
            description="out of band target recreation starts a distinct ledger generation"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_recreated_target_when_building_then_old_completion_generation_is_not_inherited(
    test_case: ConcurrentMicrobatchBehaviorE2ETestCase, tmp_path: Path
) -> None:
    project_name: str = "microbatch_recreated_target"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name=project_name,
        repo_files={
            "sqlbuild_project.toml": direct_microbatch_project_toml(
                project_name=project_name,
                database_name="recreated.duckdb",
                settings_toml="\n[settings]\nmicrobatch_concurrency = true\n",
            ),
            "sql/sources/raw.yml": raw_events_source_yml(),
            "sql/models/orders.sql": timestamp_microbatch_model_sql(
                value_expression="payload",
                batch_concurrency=2,
                replay_policy="forward",
            ),
        },
    )
    db_path: Path = project_dir / "recreated.duckdb"
    execute_duckdb(
        db_path=db_path,
        sql=(
            "CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP, payload VARCHAR); "
            "INSERT INTO raw_events VALUES "
            "(1, '2026-01-01 00:30:00', 'a'), "
            "(2, '2026-01-01 01:30:00', 'b')"
        ),
    )
    initial: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    initial_generation: object = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT physical_generation_id FROM main._sqlbuild_microbatches "
            "WHERE record_type = 'partition_completion' ORDER BY created_at DESC LIMIT 1"
        ),
    )[0][0]
    execute_duckdb(
        db_path=db_path,
        sql=(
            "DROP TABLE main.orders; "
            "CREATE TABLE main.orders (id INTEGER, event_time TIMESTAMP, value VARCHAR)"
        ),
    )
    rebuilt: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    latest_generation: object = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT physical_generation_id FROM main._sqlbuild_microbatches "
            "WHERE record_type = 'partition_completion' ORDER BY created_at DESC LIMIT 1"
        ),
    )[0][0]

    assert initial.returncode == test_case.expected_exit_code, initial.stdout + initial.stderr
    assert rebuilt.returncode == test_case.expected_exit_code, rebuilt.stdout + rebuilt.stderr
    assert latest_generation != initial_generation
    assert query_duckdb(
        db_path=db_path,
        sql="SELECT id, value FROM main.orders ORDER BY id",
    ) == [(1, "a"), (2, "b")]


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
