"""E2E coverage for microbatch failure windows, audits, and aggregate hooks."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    ConcurrentMicrobatchBehaviorE2ETestCase,
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

_AUDIT_SQL: str = 'AUDIT ();\n\nSELECT * FROM __ref("@model") WHERE NOT (@expression)\n'


@pytest.mark.parametrize(
    "test_case",
    [
        ConcurrentMicrobatchBehaviorE2ETestCase(
            description="completion write failure leaves the live target untouched and converges on retry"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_completion_write_failure_during_rebuild_when_retried_then_live_target_stays_untouched_and_converges(
    test_case: ConcurrentMicrobatchBehaviorE2ETestCase, tmp_path: Path
) -> None:
    project_name: str = "microbatch_state_write_failure"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name=project_name,
        repo_files={
            "sqlbuild_project.toml": direct_microbatch_project_toml(
                project_name=project_name,
                database_name="state_write_failure.duckdb",
                settings_toml=(
                    "\n[settings]\n"
                    "concurrency = 3\n"
                    "microbatch_concurrency = true\n"
                    'microbatch_unaccounted_partition_policy = "recover_all"\n'
                ),
            ).replace(
                'adapter = "duckdb"',
                'adapter = "failing_microbatch_state_duckdb"',
            ),
            "sql/sources/raw.yml": raw_events_source_yml(),
            "sql/models/orders.sql": timestamp_microbatch_model_sql(
                value_expression="payload",
                batch_concurrency=3,
                replay_policy="forward",
            ),
            "python/adapters/failing_microbatch_state_duckdb.py": (
                "import os\n"
                "from typing import Any\n"
                "from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter\n\n"
                "class FailingMicrobatchStateDuckDbAdapter(DuckDbAdapter):\n"
                "    adapter_name = 'failing_microbatch_state_duckdb'\n\n"
                "    def _execute(self, *, connection: Any, sql: str) -> Any:\n"
                "        if (os.environ.get('SQLBUILD_FAIL_COMPLETION_WRITE') == '1' "
                "and 'INSERT INTO main._sqlbuild_microbatches' in sql "
                "and \"'partition_completion'\" in sql "
                "and \"'2026-01-01T01:00:00'\" in sql "
                "and \"'2026-01-01T02:00:00'\" in sql):\n"
                "            raise RuntimeError('simulated completion write failure')\n"
                "        return super()._execute(connection=connection, sql=sql)\n"
            ),
        },
    )
    db_path: Path = project_dir / "state_write_failure.duckdb"
    execute_duckdb(
        db_path=db_path,
        sql=(
            "CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP, payload VARCHAR); "
            "INSERT INTO raw_events VALUES (1, '2026-01-01 00:30:00', 'a')"
        ),
    )
    initial: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
        working_dir=project_dir,
    )
    execute_duckdb(
        db_path=db_path,
        sql="INSERT INTO raw_events VALUES (2, '2026-01-01 01:15:00', 'b')",
    )
    failed: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
        env={"SQLBUILD_FAIL_COMPLETION_WRITE": "1"},
        working_dir=project_dir,
    )
    target_after_failed_state_write: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT id, value FROM main.orders ORDER BY id",
    )
    completion_count_after_failure: object = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT COUNT(*) FROM main._sqlbuild_microbatches "
            "WHERE record_type = 'partition_completion' "
            "AND partition_start = '2026-01-01T01:00:00' "
            "AND partition_end = '2026-01-01T02:00:00'"
        ),
    )[0][0]
    state_after_failure: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT record_type, partition_start, partition_end FROM main._sqlbuild_microbatches "
            "ORDER BY partition_start, record_type"
        ),
    )
    recovered: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
        working_dir=project_dir,
    )
    failure_evidence: str = f"state={state_after_failure}\n{failed.stdout}{failed.stderr}"

    assert initial.returncode == test_case.expected_exit_code, initial.stdout + initial.stderr
    assert failed.returncode != test_case.expected_exit_code, failure_evidence
    assert "simulated completion write failure" in failed.stdout + failed.stderr
    assert target_after_failed_state_write == [(1, "a")], failure_evidence
    assert completion_count_after_failure == 0, failure_evidence
    assert recovered.returncode == test_case.expected_exit_code, recovered.stdout + recovered.stderr
    assert (
        query_duckdb(
            db_path=db_path,
            sql=(
                "SELECT COUNT(*) FROM main._sqlbuild_microbatches "
                "WHERE record_type = 'partition_completion' "
                "AND partition_start = '2026-01-01T01:00:00' "
                "AND partition_end = '2026-01-01T02:00:00'"
            ),
        )[0][0]
        >= 1
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ConcurrentMicrobatchBehaviorE2ETestCase(
            description="delta audit failure blocks one batch while later successful work remains recoverable"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_concurrent_delta_audit_failure_when_fixed_then_rejected_partition_is_retried(
    test_case: ConcurrentMicrobatchBehaviorE2ETestCase, tmp_path: Path
) -> None:
    project_name: str = "microbatch_delta_audit_failure"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name=project_name,
        repo_files={
            "sqlbuild_project.toml": direct_microbatch_project_toml(
                project_name=project_name,
                database_name="delta_audit_failure.duckdb",
                settings_toml=("\n[settings]\nconcurrency = 3\nmicrobatch_concurrency = true\n"),
            ),
            "sql/sources/raw.yml": raw_events_source_yml(),
            "sql/models/orders.sql": timestamp_microbatch_model_sql(
                value_expression="CAST(payload AS INTEGER)",
                batch_concurrency=3,
                replay_policy="forward",
                extra_config=(
                    "audits [expression_is_true ("
                    'name "positive_value", expression "value > 0", severity error, '
                    "run_scope delta_and_final,)],"
                ),
            ),
            "sql/audits/generic/expression_is_true.sql": _AUDIT_SQL,
        },
    )
    db_path: Path = project_dir / "delta_audit_failure.duckdb"
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
            "(3, '2026-01-01 02:30:00', '-3'), "
            "(4, '2026-01-01 03:30:00', '4')"
        ),
    )
    failed: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    target_after_failure: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT id, value FROM main.orders ORDER BY id",
    )
    failed_partition_completions: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT COUNT(*) FROM main._sqlbuild_microbatches "
            "WHERE record_type = 'partition_completion' "
            "AND partition_start = '2026-01-01T02:00:00'"
        ),
    )
    execute_duckdb(db_path=db_path, sql="UPDATE raw_events SET payload = '3' WHERE id = 3")
    recovered: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    assert initial.returncode == test_case.expected_exit_code, initial.stdout + initial.stderr
    assert failed.returncode != test_case.expected_exit_code
    assert "delta audit" in failed.stdout + failed.stderr
    assert (3, -3) not in target_after_failure
    assert failed_partition_completions == [(0,)]
    assert recovered.returncode == test_case.expected_exit_code, recovered.stdout + recovered.stderr
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
                "AND partition_start = '2026-01-01T02:00:00'"
            ),
        )[0][0]
        >= 1
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ConcurrentMicrobatchBehaviorE2ETestCase(
            description="aggregate final audit and hooks run once around concurrent batches"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_concurrent_batches_when_audit_and_hooks_wrap_model_then_aggregate_work_runs_once(
    test_case: ConcurrentMicrobatchBehaviorE2ETestCase, tmp_path: Path
) -> None:
    project_name: str = "microbatch_aggregate_hooks"
    model_sql: str = timestamp_microbatch_model_sql(
        value_expression="payload",
        batch_concurrency=3,
        replay_policy="forward",
        extra_config=(
            "pre_hooks [inline_sql(\"INSERT INTO main.hook_log VALUES ('pre')\")], "
            "post_hooks [inline_sql(\"INSERT INTO main.hook_log VALUES ('post')\")],"
            "audits [expression_is_true ("
            'name "value_present", expression "value IS NOT NULL", severity error, '
            "run_scope final,)],"
        ),
    )
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name=project_name,
        repo_files={
            "sqlbuild_project.toml": direct_microbatch_project_toml(
                project_name=project_name,
                database_name="aggregate_hooks.duckdb",
                settings_toml=("\n[settings]\nconcurrency = 3\nmicrobatch_concurrency = true\n"),
            ),
            "sql/sources/raw.yml": raw_events_source_yml(),
            "sql/models/orders.sql": model_sql,
            "sql/audits/generic/expression_is_true.sql": _AUDIT_SQL,
        },
    )
    db_path: Path = project_dir / "aggregate_hooks.duckdb"
    execute_duckdb(
        db_path=db_path,
        sql=(
            "CREATE TABLE raw_events (id INTEGER, event_time TIMESTAMP, payload VARCHAR); "
            "CREATE TABLE hook_log (phase VARCHAR); "
            "INSERT INTO raw_events VALUES (1, '2026-01-01 00:30:00', 'a')"
        ),
    )
    initial: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    execute_duckdb(
        db_path=db_path,
        sql=(
            "INSERT INTO raw_events VALUES "
            "(2, '2026-01-01 01:30:00', 'b'), "
            "(3, '2026-01-01 02:30:00', 'c'), "
            "(4, '2026-01-01 03:30:00', 'd')"
        ),
    )
    incremental: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    assert initial.returncode == test_case.expected_exit_code, initial.stdout + initial.stderr
    assert incremental.returncode == test_case.expected_exit_code, (
        incremental.stdout + incremental.stderr
    )
    assert incremental.stdout.count("Audit evaluation  START") == 1
    assert incremental.stdout.count("Audit evaluation  OK") == 1
    assert "Audit evaluation  FAIL" not in incremental.stdout
    assert query_duckdb(
        db_path=db_path,
        sql="SELECT phase, COUNT(*) FROM hook_log GROUP BY phase ORDER BY phase",
    ) == [("post", 2), ("pre", 2)]
    assert query_duckdb(
        db_path=db_path,
        sql="SELECT COUNT(*) FROM main.orders",
    ) == [(4,)]


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
