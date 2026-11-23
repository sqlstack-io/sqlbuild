"""E2E coverage for opt-in concurrent microbatch execution and history."""

from __future__ import annotations

import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    ConcurrentMicrobatchBehaviorE2ETestCase,
    MicrobatchReconciliationPolicyE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import (
    replay_microbatch_model_sql,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ConcurrentMicrobatchBehaviorE2ETestCase(
            description="project capability disabled rejects model concurrency",
            expected_exit_code=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_project_capability_disabled_when_batch_concurrency_exceeds_one_then_compile_fails(
    test_case: ConcurrentMicrobatchBehaviorE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="disabled_concurrent_microbatch_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "disabled_concurrent_microbatch_project"
                adapter = "duckdb"

                [connection]
                database = "disabled.duckdb"
                """
            ).strip()
            + "\n",
            "models/orders.sql": dedent(
                """
                MODEL (description "Test model orders.",
                  materialized incremental,
                  incremental_strategy delete_insert,
                  incremental_mode microbatch,
          microbatch_strategy watermark,
          cursor_watermark_mode all,
                  cursor event_time,
                  cursor_type timestamp,
                  cursor_grain hour,
                  cursor_inputs (
                    raw_events (column event_time, roles [filter, watermark]),
                  ),
                  batch_size 1h,
                  batch_concurrency 2,
                );

                SELECT 1 AS id, TIMESTAMP '2026-01-01 00:00:00' AS event_time
                FROM __source("raw_events")
                """
            ).strip()
            + "\n",
            "sources/raw.yml": (
                "sources:\n  - name: raw_events\n    description: Test source raw_events.\n    schema: main\n    table: raw_events\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    assert result.returncode == test_case.expected_exit_code
    assert (
        "= help: to run microbatches concurrently, set this in sqlbuild_project.toml:\n"
        "            [settings]\n"
        "            microbatch_concurrency = true"
    ) in (result.stdout + result.stderr)


@pytest.mark.parametrize(
    "test_case",
    [ConcurrentMicrobatchBehaviorE2ETestCase(description="concurrent batches record history")],
    ids=lambda case: case.description,
)
def test_given_opt_in_concurrent_microbatch_when_building_twice_then_records_all_batches(
    test_case: ConcurrentMicrobatchBehaviorE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="concurrent_microbatch_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "concurrent_microbatch_project"
                adapter = "duckdb"

                [connection]
                database = "concurrent.duckdb"

                [settings]
                concurrency = 3
                microbatch_concurrency = true
                """
            ).strip()
            + "\n",
            "sources/raw.yml": dedent(
                """
                sources:
                  - name: raw_events
                    description: Test source raw_events.
                    schema: main
                    table: raw_events
                """
            ).strip()
            + "\n",
            "models/orders.sql": dedent(
                """
                MODEL (description "Test model orders.",
                  materialized incremental,
                  incremental_strategy delete_insert,
                  incremental_mode microbatch,
          microbatch_strategy watermark,
          cursor_watermark_mode all,
                  cursor event_time,
                  cursor_type timestamp,
                  cursor_grain hour,
                  cursor_inputs (
            raw_events (column event_time, roles [filter, watermark]),
          ),
                  batch_size 1h,
                  batch_concurrency 3,
                );

                SELECT id, event_time, payload
                FROM __source("raw_events")
                WHERE event_time >= __cursor_start()
                  AND event_time < __cursor_end()
                """
            ).strip()
            + "\n",
        },
    )
    db_path: Path = project_dir / "concurrent.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(
        "CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP, payload VARCHAR)"
    )
    connection.execute(
        "INSERT INTO raw_events VALUES "
        "(1, '2026-01-01 00:30:00', 'a'), "
        "(2, '2026-01-01 01:30:00', 'b'), "
        "(3, '2026-01-01 02:30:00', 'c')"
    )
    connection.close()

    initial: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert initial.returncode == 0, initial.stdout + initial.stderr

    connection = duckdb.connect(str(db_path))
    connection.execute(
        "INSERT INTO raw_events VALUES "
        "(4, '2026-01-01 03:30:00', 'd'), "
        "(5, '2026-01-01 04:30:00', 'e'), "
        "(6, '2026-01-01 05:30:00', 'f')"
    )
    connection.close()

    incremental: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    output: str = incremental.stdout + incremental.stderr
    assert incremental.returncode == test_case.expected_exit_code, output
    assert output.count("Concurrent microbatch execution is enabled") == 1

    assert query_duckdb(
        db_path=db_path,
        sql="SELECT id, payload FROM main.orders ORDER BY id",
    ) == [(1, "a"), (2, "b"), (3, "c"), (4, "d"), (5, "e"), (6, "f")]
    history: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT record_type, fingerprint_status, COUNT(*) "
            "FROM main._sqlbuild_microbatches "
            "GROUP BY record_type, fingerprint_status "
            "ORDER BY record_type, fingerprint_status"
        ),
    )
    assert history == [
        ("partition_completion", "known", 9),
    ]
    delta_count: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT COUNT(*) FROM information_schema.tables "
            "WHERE table_schema = 'main' AND table_name LIKE 'orders__delta%'"
        ),
    )
    assert delta_count == [(0,)]


@pytest.mark.parametrize(
    "test_case",
    [ConcurrentMicrobatchBehaviorE2ETestCase(description="failed sibling gap recovers")],
    ids=lambda case: case.description,
)
def test_given_later_batches_succeed_when_one_batch_fails_then_next_run_preserves_result(
    test_case: ConcurrentMicrobatchBehaviorE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="concurrent_microbatch_recovery_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "concurrent_microbatch_recovery_project"
                adapter = "duckdb"

                [connection]
                database = "recovery.duckdb"

                [settings]
                concurrency = 3
                microbatch_concurrency = true
                """
            ).strip()
            + "\n",
            "sources/raw.yml": dedent(
                """
                sources:
                  - name: raw_events
                    description: Test source raw_events.
                    schema: main
                    table: raw_events
                """
            ).strip()
            + "\n",
            "models/orders.sql": dedent(
                """
                MODEL (description "Test model orders.",
                  materialized incremental,
                  incremental_strategy delete_insert,
                  incremental_mode microbatch,
          microbatch_strategy watermark,
          cursor_watermark_mode all,
                  cursor event_time,
                  cursor_type timestamp,
                  cursor_grain hour,
                  cursor_inputs (
            raw_events (column event_time, roles [filter, watermark]),
          ),
                  batch_size 1h,
                  batch_concurrency 3,
                );

                SELECT id, event_time, CAST(payload AS INTEGER) AS value
                FROM __source("raw_events")
                WHERE event_time >= __cursor_start()
                  AND event_time < __cursor_end()
                """
            ).strip()
            + "\n",
        },
    )
    db_path: Path = project_dir / "recovery.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(
        "CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP, payload VARCHAR)"
    )
    connection.execute(
        "INSERT INTO raw_events VALUES "
        "(1, '2026-01-01 00:30:00', '1'), "
        "(2, '2026-01-01 01:30:00', '2'), "
        "(3, '2026-01-01 02:30:00', '3')"
    )
    connection.close()
    initial: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert initial.returncode == 0, initial.stdout + initial.stderr

    connection = duckdb.connect(str(db_path))
    connection.execute(
        "UPDATE raw_events SET payload = 'bad' WHERE id = 3; "
        "INSERT INTO raw_events VALUES "
        "(4, '2026-01-01 03:30:00', '4'), "
        "(5, '2026-01-01 04:30:00', '5'), "
        "(6, '2026-01-01 05:30:00', '6'), "
        "(7, '2026-01-01 06:30:00', '7')"
    )
    connection.close()
    completion_events_sql: str = (
        "SELECT event_id, partition_start FROM main._sqlbuild_microbatches "
        "WHERE record_type = 'partition_completion'"
    )
    initial_completion_ids: set[str] = {
        row[0] for row in query_duckdb(db_path=db_path, sql=completion_events_sql)
    }

    failed: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert failed.returncode != 0
    failed_run_completion_starts: set[str] = {
        row[1]
        for row in query_duckdb(db_path=db_path, sql=completion_events_sql)
        if row[0] not in initial_completion_ids
    }
    # The failing 02:00 batch records no completion. Its 03:00 sibling is handed to a worker
    # before the coordinator runs 04:00 itself, and sibling failures are only examined after
    # 04:00 finishes, so both always complete. Whether 05:00 and 06:00 also start before the
    # failure is seen depends on runner timing.
    assert "2026-01-01T02:00:00" not in failed_run_completion_starts
    assert {"2026-01-01T03:00:00", "2026-01-01T04:00:00"} <= failed_run_completion_starts

    connection = duckdb.connect(str(db_path))
    connection.execute("UPDATE raw_events SET payload = '3' WHERE id = 3")
    connection.close()
    recovered: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert recovered.returncode == test_case.expected_exit_code, recovered.stdout + recovered.stderr

    assert query_duckdb(
        db_path=db_path,
        sql="SELECT id, value FROM main.orders ORDER BY id",
    ) == [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6), (7, 7)]
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
        MicrobatchReconciliationPolicyE2ETestCase(
            description="synthesize accepts all physical coverage",
            policy="synthesize",
            expected_synthetic=3,
            expected_minimum_recovery=0,
            expected_warning_fragment="fingerprints are unknown",
            expected_fingerprint_rows=(("unknown", None),),
        ),
        MicrobatchReconciliationPolicyE2ETestCase(
            description="recover empty reruns empty interval",
            policy="recover_empty",
            expected_synthetic=2,
            expected_minimum_recovery=1,
            expected_warning_fragment="fingerprints are unknown",
            expected_fingerprint_rows=(("unknown", None),),
        ),
        MicrobatchReconciliationPolicyE2ETestCase(
            description="recover all reruns every interval",
            policy="recover_all",
            expected_synthetic=0,
            expected_minimum_recovery=3,
            expected_warning_fragment="",
            expected_fingerprint_rows=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_microbatch_history_is_lost_when_reconciling_then_policy_is_applied(
    tmp_path: Path,
    test_case: MicrobatchReconciliationPolicyE2ETestCase,
) -> None:
    policy: str = test_case.policy
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name=f"microbatch_{policy}_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                f"""
                name = "microbatch_{policy}_project"
                adapter = "duckdb"

                [connection]
                database = "policy.duckdb"

                [settings]
                microbatch_concurrency = true
                microbatch_unaccounted_partition_policy = "{policy}"
                """
            ).strip()
            + "\n",
            "sources/raw.yml": dedent(
                """
                sources:
                  - name: raw_events
                    description: Test source raw_events.
                    schema: main
                    table: raw_events
                """
            ).strip()
            + "\n",
            "models/orders.sql": dedent(
                """
                MODEL (description "Test model orders.",
                  materialized incremental,
                  incremental_strategy delete_insert,
                  incremental_mode microbatch,
          microbatch_strategy watermark,
          cursor_watermark_mode all,
                  cursor event_time,
                  cursor_type timestamp,
                  cursor_grain hour,
                  cursor_inputs (
            raw_events (column event_time, roles [filter, watermark]),
          ),
                  batch_size 1h,
                  batch_concurrency 2,
                );

                SELECT id, event_time
                FROM __source("raw_events")
                WHERE event_time >= __cursor_start()
                  AND event_time < __cursor_end()
                """
            ).strip()
            + "\n",
        },
    )
    db_path: Path = project_dir / "policy.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute("CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP)")
    connection.execute(
        "INSERT INTO raw_events VALUES (1, '2026-01-01 00:30:00'), (2, '2026-01-01 02:30:00')"
    )
    connection.close()
    initial: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert initial.returncode == 0, initial.stdout + initial.stderr

    connection = duckdb.connect(str(db_path))
    connection.execute("DELETE FROM main._sqlbuild_microbatches")
    connection.close()
    reconciled: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert reconciled.returncode == 0, reconciled.stdout + reconciled.stderr

    counts: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT "
            "COUNT(*) FILTER (WHERE record_type = 'synthetic_completion'), "
            "COUNT(*) FILTER (WHERE record_type = 'partition_completion' "
            "AND completion_type = 'recovery') "
            "FROM main._sqlbuild_microbatches"
        ),
    )
    synthetic_count: int = int(str(counts[0][0]))
    recovery_count: int = int(str(counts[0][1]))
    assert synthetic_count == test_case.expected_synthetic
    assert recovery_count >= test_case.expected_minimum_recovery
    assert test_case.expected_warning_fragment in (reconciled.stdout + reconciled.stderr)
    assert query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT DISTINCT fingerprint_status, model_version_hash "
            "FROM main._sqlbuild_microbatches "
            "WHERE record_type = 'synthetic_completion'"
        ),
    ) == list(test_case.expected_fingerprint_rows)


@pytest.mark.parametrize(
    "test_case",
    [ConcurrentMicrobatchBehaviorE2ETestCase(description="bounded replay is durable")],
    ids=lambda case: case.description,
)
def test_given_bounded_replay_on_change_when_model_changes_then_requirement_is_durable(
    test_case: ConcurrentMicrobatchBehaviorE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="microbatch_replay_requirement_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "microbatch_replay_requirement_project"
                adapter = "duckdb"

                [connection]
                database = "replay.duckdb"

                [settings]
                microbatch_concurrency = true
                """
            ).strip()
            + "\n",
            "sources/raw.yml": dedent(
                """
                sources:
                  - name: raw_events
                    description: Test source raw_events.
                    schema: main
                    table: raw_events
                """
            ).strip()
            + "\n",
            "models/orders.sql": replay_microbatch_model_sql(
                value_expression="CAST(payload AS INTEGER)"
            ),
        },
    )
    db_path: Path = project_dir / "replay.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(
        "CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP, payload VARCHAR)"
    )
    connection.execute(
        "INSERT INTO raw_events VALUES "
        "(1, '2026-01-01 00:30:00', '1'), "
        "(2, '2026-01-01 01:30:00', '2'), "
        "(3, '2026-01-01 02:30:00', '3'), "
        "(4, '2026-01-01 03:30:00', '4')"
    )
    connection.close()
    initial: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert initial.returncode == 0, initial.stdout + initial.stderr

    (project_dir / "models" / "orders.sql").write_text(
        replay_microbatch_model_sql(value_expression="CAST(payload AS INTEGER) + 1"),
        encoding="utf-8",
    )
    replay: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert replay.returncode == test_case.expected_exit_code, replay.stdout + replay.stderr

    requirements: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT replay_requirement_id, required_model_version_hash, "
            "previous_model_version_hash, run_start, run_end "
            "FROM main._sqlbuild_microbatches "
            "WHERE record_type = 'replay_requirement'"
        ),
    )
    assert len(requirements) == 1
    requirement_id, required_hash, previous_hash, run_start, run_end = requirements[0]
    assert requirement_id
    assert required_hash
    assert previous_hash
    assert required_hash != previous_hash
    assert str(run_start) < str(run_end)
    assert (
        query_duckdb(
            db_path=db_path,
            sql=(
                "SELECT COUNT(*) FROM main._sqlbuild_microbatches "
                "WHERE record_type = 'partition_completion' "
                "AND run_type = 'replay_on_change' "
                f"AND replay_requirement_id = '{requirement_id}'"
            ),
        )[0][0]
        >= 1
    )


@pytest.mark.parametrize(
    "test_case",
    [ConcurrentMicrobatchBehaviorE2ETestCase(description="full replay retry reuses requirement")],
    ids=lambda case: case.description,
)
def test_given_full_replay_fails_when_retried_then_original_requirement_is_reused(
    test_case: ConcurrentMicrobatchBehaviorE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="microbatch_full_replay_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "microbatch_full_replay_project"
                adapter = "duckdb"

                [connection]
                database = "full_replay.duckdb"

                [settings]
                microbatch_concurrency = true
                """
            ).strip()
            + "\n",
            "sources/raw.yml": dedent(
                """
                sources:
                  - name: raw_events
                    description: Test source raw_events.
                    schema: main
                    table: raw_events
                """
            ).strip()
            + "\n",
            "models/orders.sql": replay_microbatch_model_sql(
                value_expression="CAST(payload AS INTEGER)", replay_policy="full"
            ),
        },
    )
    db_path: Path = project_dir / "full_replay.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(
        "CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP, payload VARCHAR)"
    )
    connection.execute(
        "INSERT INTO raw_events VALUES "
        "(1, '2026-01-01 00:30:00', '1'), "
        "(2, '2026-01-01 01:30:00', '2')"
    )
    connection.close()
    initial: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert initial.returncode == 0, initial.stdout + initial.stderr

    (project_dir / "models" / "orders.sql").write_text(
        replay_microbatch_model_sql(
            value_expression="CAST(payload AS INTEGER) + 1", replay_policy="full"
        ),
        encoding="utf-8",
    )
    connection = duckdb.connect(str(db_path))
    connection.execute("UPDATE raw_events SET payload = 'bad' WHERE id = 2")
    connection.close()
    failed: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert failed.returncode != 0

    connection = duckdb.connect(str(db_path))
    connection.execute("UPDATE raw_events SET payload = '2' WHERE id = 2")
    connection.close()
    retried: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert retried.returncode == test_case.expected_exit_code, retried.stdout + retried.stderr
    assert query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT COUNT(DISTINCT replay_requirement_id), "
            "MIN(physical_generation_id LIKE 'replay:%') "
            "FROM main._sqlbuild_microbatches "
            "WHERE record_type = 'replay_requirement'"
        ),
    ) == [(1, True)]


@pytest.mark.parametrize(
    "test_case",
    [ConcurrentMicrobatchBehaviorE2ETestCase(description="returning hash gets new requirement")],
    ids=lambda case: case.description,
)
def test_given_version_hash_returns_when_replaying_then_new_transition_gets_new_requirement(
    test_case: ConcurrentMicrobatchBehaviorE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="microbatch_repeated_version_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "microbatch_repeated_version_project"
                adapter = "duckdb"

                [connection]
                database = "repeated_version.duckdb"

                [settings]
                microbatch_concurrency = true
                """
            ).strip()
            + "\n",
            "sources/raw.yml": dedent(
                """
                sources:
                  - name: raw_events
                    description: Test source raw_events.
                    schema: main
                    table: raw_events
                """
            ).strip()
            + "\n",
            "models/orders.sql": replay_microbatch_model_sql(
                value_expression="CAST(payload AS INTEGER)"
            ),
        },
    )
    db_path: Path = project_dir / "repeated_version.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(
        "CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP, payload VARCHAR)"
    )
    connection.execute(
        "INSERT INTO raw_events VALUES "
        "(1, '2026-01-01 00:30:00', '1'), "
        "(2, '2026-01-01 01:30:00', '2')"
    )
    connection.close()

    model_path: Path = project_dir / "models" / "orders.sql"
    f1_sql: str = replay_microbatch_model_sql(value_expression="CAST(payload AS INTEGER)")
    f2_sql: str = replay_microbatch_model_sql(value_expression="CAST(payload AS INTEGER) + 1")
    initial: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert initial.returncode == 0, initial.stdout + initial.stderr
    model_path.write_text(f2_sql, encoding="utf-8")
    first_f2: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert first_f2.returncode == 0, first_f2.stdout + first_f2.stderr
    model_path.write_text(f1_sql, encoding="utf-8")
    returned_f1: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert returned_f1.returncode == 0, returned_f1.stdout + returned_f1.stderr
    model_path.write_text(f2_sql, encoding="utf-8")
    second_f2: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert second_f2.returncode == test_case.expected_exit_code, second_f2.stdout + second_f2.stderr

    f2_requirements: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT required_model_version_hash, COUNT(DISTINCT replay_requirement_id) "
            "FROM main._sqlbuild_microbatches "
            "WHERE record_type = 'replay_requirement' "
            "GROUP BY required_model_version_hash ORDER BY COUNT(*) DESC"
        ),
    )
    assert max(int(str(count)) for _version_hash, count in f2_requirements) == 2


@pytest.mark.parametrize(
    "test_case",
    [ConcurrentMicrobatchBehaviorE2ETestCase(description="historical backfill stays isolated")],
    ids=lambda case: case.description,
)
def test_given_historical_backfill_when_normal_run_resumes_then_gap_to_normal_floor_is_not_recovered(
    test_case: ConcurrentMicrobatchBehaviorE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="microbatch_historical_backfill_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "microbatch_historical_backfill_project"
                adapter = "duckdb"

                [connection]
                database = "historical.duckdb"

                [settings]
                concurrency = 3
                microbatch_concurrency = true
                """
            ).strip()
            + "\n",
            "sources/raw.yml": dedent(
                """
                sources:
                  - name: raw_events
                    description: Test source raw_events.
                    schema: main
                    table: raw_events
                """
            ).strip()
            + "\n",
            "models/orders.sql": dedent(
                """
                MODEL (description "Test model orders.",
                  materialized incremental,
                  incremental_strategy delete_insert,
                  incremental_mode microbatch,
          microbatch_strategy watermark,
          cursor_watermark_mode all,
                  cursor event_time,
                  cursor_type timestamp,
                  cursor_grain day,
                  cursor_inputs (
            raw_events (column event_time, roles [filter, watermark]),
                  ),
                  batch_size 1d,
                  batch_concurrency 2,
                );

                SELECT id, event_time
                FROM __source("raw_events")
                WHERE event_time >= __cursor_start()
                  AND event_time < __cursor_end()
                """
            ).strip()
            + "\n",
        },
    )
    db_path: Path = project_dir / "historical.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute("CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP)")
    connection.execute("INSERT INTO raw_events VALUES (10, '2026-08-10 12:00:00')")
    connection.close()
    initial: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert initial.returncode == 0, initial.stdout + initial.stderr

    connection = duckdb.connect(str(db_path))
    connection.execute("INSERT INTO raw_events VALUES (1, '2026-08-01 12:00:00')")
    connection.close()
    backfill: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--start-cursor-ts",
            "2026-08-01T00:00:00",
            "--end-cursor-ts",
            "2026-08-02T00:00:00",
        ),
        project_dir=project_dir,
    )
    assert backfill.returncode == 0, backfill.stdout + backfill.stderr
    resumed: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert resumed.returncode == test_case.expected_exit_code, resumed.stdout + resumed.stderr

    assert query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT COUNT(*) FROM main._sqlbuild_microbatches "
            "WHERE record_type = 'partition_completion' "
            "AND completion_type = 'recovery'"
        ),
    ) == [(0,)]
    assert query_duckdb(
        db_path=db_path,
        sql="SELECT id FROM main.orders ORDER BY id",
    ) == [(1,), (10,)]


@pytest.mark.parametrize(
    "test_case",
    [ConcurrentMicrobatchBehaviorE2ETestCase(description="steady state still reconciles")],
    ids=lambda case: case.description,
)
def test_given_no_new_cursor_work_when_history_is_lost_then_reconciliation_still_runs(
    test_case: ConcurrentMicrobatchBehaviorE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="microbatch_steady_state_reconciliation_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "microbatch_steady_state_reconciliation_project"
                adapter = "duckdb"

                [connection]
                database = "steady_state.duckdb"

                [settings]
                microbatch_concurrency = true
                """
            ).strip()
            + "\n",
            "sources/raw.yml": dedent(
                """
                sources:
                  - name: raw_events
                    description: Test source raw_events.
                    schema: main
                    table: raw_events
                """
            ).strip()
            + "\n",
            "models/orders.sql": dedent(
                """
                MODEL (description "Test model orders.",
                  materialized incremental,
                  incremental_strategy delete_insert,
                  incremental_mode microbatch,
          microbatch_strategy watermark,
          cursor_watermark_mode all,
                  cursor event_time,
                  cursor_type timestamp,
                  cursor_grain hour,
                  cursor_inputs (
            raw_events (column event_time, roles [filter, watermark]),
          ),
                  batch_size 1h,
                  batch_concurrency 2,
                );

                SELECT id, event_time
                FROM __source("raw_events")
                WHERE event_time >= __cursor_start()
                  AND event_time < __cursor_end()
                """
            ).strip()
            + "\n",
        },
    )
    db_path: Path = project_dir / "steady_state.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute("CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP)")
    connection.execute("INSERT INTO raw_events VALUES (1, '2026-01-01 00:30:00')")
    connection.close()
    initial: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert initial.returncode == 0, initial.stdout + initial.stderr

    connection = duckdb.connect(str(db_path))
    connection.execute("DELETE FROM main._sqlbuild_microbatches")
    connection.execute("DELETE FROM main.raw_events")
    connection.close()
    reconciled: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert reconciled.returncode == test_case.expected_exit_code, (
        reconciled.stdout + reconciled.stderr
    )
    assert (
        query_duckdb(
            db_path=db_path,
            sql=(
                "SELECT COUNT(*) FROM main._sqlbuild_microbatches "
                "WHERE record_type = 'synthetic_completion'"
            ),
        )[0][0]
        >= 1
    )
