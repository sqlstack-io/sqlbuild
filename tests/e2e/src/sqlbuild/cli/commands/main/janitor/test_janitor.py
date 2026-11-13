"""E2E tests for the janitor CLI command."""

from __future__ import annotations

import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.janitor._test_types import (
    JanitorCleanupE2ETestCase,
    JanitorDirectStatePruningE2ETestCase,
    JanitorDisabledE2ETestCase,
    JanitorInvalidConfigE2ETestCase,
    JanitorNonInteractiveE2ETestCase,
    JanitorSourceOverlapE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.janitor.helpers import (
    create_direct_state_history,
    create_janitor_demo_relations,
    create_janitor_scenario_relations,
    prepare_janitor_project,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    query_duckdb,
    run_sqb,
    table_exists,
)


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorDisabledE2ETestCase(
            description="disabled janitor exits successfully with guidance",
            command=("janitor", "--auto-approve"),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Janitor is disabled for this project.",
                "enabled = true",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_default_config_when_running_janitor_then_it_reports_disabled(
    test_case: JanitorDisabledE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_janitor_project(
        tmp_path=tmp_path,
        project_name="janitor_disabled_project",
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorCleanupE2ETestCase(
            description="tracked-only direct janitor archives only tracked stale relations",
            build_command=("--no-color", "build", "--full-refresh"),
            janitor_command=("janitor", "--auto-approve", "--verbose"),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "relations to archive   1",
                "archives to delete     0",
                "objects skipped        5",
                "main.janitor_tracked_extra  ->  main._sqb_archive__",
                "main.janitor_untracked_extra  relation is not tracked by SQLBuild",
                "main.partition_state  relation matches exclude pattern 'partition_*'",
                "main._sqlbuild_fingerprints  relation matches exclude pattern",
                "main._sqlbuild_microbatches  relation matches exclude pattern",
                "main._sqlbuild_consumption  relation matches exclude pattern",
                "Archived 1 relation.",
            ),
            expected_existing_tables=(
                "orders",
                "janitor_untracked_extra",
                "partition_state",
                "_sqlbuild_fingerprints",
                "_sqlbuild_microbatches",
                "_sqlbuild_janitor_events",
                "_sqlbuild_consumption",
            ),
            expected_missing_tables=("janitor_tracked_extra",),
            expected_archived_original_names=("janitor_tracked_extra",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_stale_relations_when_running_direct_janitor_then_it_archives_tracked_relations(
    test_case: JanitorCleanupE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_janitor_project(
        tmp_path=tmp_path,
        project_name="janitor_cleanup_project",
        janitor_config=dedent(
            """
              enabled = true
              retention_days = 0
              exclude_patterns = ["partition_*"]
            """
        ),
    )
    db_path: Path = project_dir / "janitor.duckdb"

    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.build_command,
        project_dir=project_dir,
    )
    assert build_result.returncode == test_case.expected_exit_code, (
        build_result.stdout + build_result.stderr
    )
    create_janitor_demo_relations(db_path=db_path)

    janitor_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command,
        project_dir=project_dir,
    )

    assert janitor_result.returncode == test_case.expected_exit_code, (
        janitor_result.stdout + janitor_result.stderr
    )
    fragment: str
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in janitor_result.stdout
    table_name: str
    for table_name in test_case.expected_existing_tables:
        assert table_exists(db_path=db_path, table_name=table_name)
    for table_name in test_case.expected_missing_tables:
        assert not table_exists(db_path=db_path, table_name=table_name)
    for original_name in test_case.expected_archived_original_names:
        assert query_duckdb(
            db_path=db_path,
            sql=(
                "SELECT COUNT(*) FROM information_schema.tables "
                f"WHERE table_name LIKE '_sqb_archive__%__{original_name}'"
            ),
        ) == [(1,)]
    assert "Eligible expired VDEs" not in janitor_result.stdout
    assert "Eligible state backups" not in janitor_result.stdout
    assert "Eligible expired locks" not in janitor_result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorNonInteractiveE2ETestCase(
            description="janitor without auto-approve refuses when stdin is not a terminal",
            janitor_command=("--no-color", "janitor"),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[C503]: janitor cleanup requires confirmation, but stdin is not a terminal",
                "pass --auto-approve",
            ),
            expected_existing_tables=("janitor_tracked_extra", "janitor_untracked_extra"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_piped_stdin_when_janitor_needs_confirmation_then_it_refuses_cleanly(
    test_case: JanitorNonInteractiveE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_janitor_project(
        tmp_path=tmp_path,
        project_name="janitor_non_interactive_project",
        janitor_config=dedent(
            """
              enabled = true
              retention_days = 0
            """
        ),
    )
    db_path: Path = project_dir / "janitor.duckdb"
    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--full-refresh"),
        project_dir=project_dir,
    )
    create_janitor_demo_relations(db_path=db_path)

    janitor_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command,
        project_dir=project_dir,
        input_text="",
    )

    output: str = janitor_result.stdout + janitor_result.stderr
    assert build_result.returncode == 0, build_result.stdout + build_result.stderr
    assert janitor_result.returncode == test_case.expected_exit_code, test_case.description + output
    assert "Traceback" not in output, test_case.description + output
    assert all(
        fragment in janitor_result.stderr for fragment in test_case.expected_stderr_fragments
    ), test_case.description + output
    assert all(
        table_exists(db_path=db_path, table_name=table_name)
        for table_name in test_case.expected_existing_tables
    ), test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorSourceOverlapE2ETestCase(
            description="source in managed schema blocks all direct janitor actions",
            janitor_command=("--no-color", "janitor", "--auto-approve"),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "Janitor blocked",
                "Managed target schemas contain active configured sources.",
                "main  active sources: raw_events",
                "suppressed archive: main.stale_main",
                "No janitor actions will be performed.",
            ),
            expected_existing_relations=(
                ("main", "raw_events"),
                ("main", "stale_main"),
                ("safe", "stale_safe"),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_source_in_managed_schema_when_running_direct_janitor_then_blocks_all_actions(
    test_case: JanitorSourceOverlapE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="janitor_source_overlap",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "janitor_source_overlap"
                adapter = "duckdb"

                [connection]
                database = "janitor.duckdb"

                [janitor]
                enabled = true
                retention_days = 0
                delete_tracked_only = false

                [defaults]
                materialized = "table"
                """
            ).strip()
            + "\n",
            "models/orders.sql": "MODEL (description 'Test model orders.');\n\nSELECT 1 AS order_id\n",
            "models/safe_orders.sql": "MODEL (description 'Test model safe_orders.', schema safe,);\n\nSELECT 1 AS order_id\n",
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
        },
    )
    db_path: Path = project_dir / "janitor.duckdb"
    execute_duckdb(
        db_path=db_path,
        sql=(
            "CREATE TABLE raw_events AS SELECT 1 AS id; "
            "CREATE TABLE stale_main AS SELECT 1 AS id; "
            "CREATE SCHEMA safe; "
            "CREATE TABLE safe.stale_safe AS SELECT 1 AS id"
        ),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout
    for schema_name, relation_name in test_case.expected_existing_relations:
        assert (
            query_duckdb(
                db_path=db_path,
                sql=(
                    "SELECT COUNT(*) FROM information_schema.tables "
                    f"WHERE table_schema = '{schema_name}' AND table_name = '{relation_name}'"
                ),
            )[0][0]
            == 1
        )


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorCleanupE2ETestCase(
            description="tracked-only direct janitor archives strict scenario artifacts",
            build_command=("--no-color", "build", "--full-refresh"),
            janitor_command=("janitor", "--auto-approve", "--verbose"),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "relations to archive   2",
                "objects skipped        3",
                "main.__sqb_a13f09c2e7b8__model__daily_revenue  ->  main._sqb_archive__",
                "main.__sqb_a13f09c2e7b8__source__raw_orders  ->  main._sqb_archive__",
                "main.__sqb_a13f09c2e7b__model__daily_revenue  relation is not tracked by SQLBuild",
            ),
            expected_existing_tables=(
                "orders",
                "__sqb_a13f09c2e7b__model__daily_revenue",
            ),
            expected_missing_tables=(
                "__sqb_a13f09c2e7b8__source__raw_orders",
                "__sqb_a13f09c2e7b8__model__daily_revenue",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_scenario_artifacts_when_running_direct_janitor_then_it_archives_them(
    test_case: JanitorCleanupE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_janitor_project(
        tmp_path=tmp_path,
        project_name="janitor_scenario_cleanup_project",
        janitor_config=dedent(
            """
              enabled = true
              retention_days = 0
            """
        ),
    )
    db_path: Path = project_dir / "janitor.duckdb"

    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.build_command,
        project_dir=project_dir,
    )
    assert build_result.returncode == test_case.expected_exit_code, (
        build_result.stdout + build_result.stderr
    )
    create_janitor_scenario_relations(db_path=db_path)

    janitor_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command,
        project_dir=project_dir,
    )

    assert janitor_result.returncode == test_case.expected_exit_code, (
        janitor_result.stdout + janitor_result.stderr
    )
    fragment: str
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in janitor_result.stdout
    table_name: str
    for table_name in test_case.expected_existing_tables:
        assert table_exists(db_path=db_path, table_name=table_name)
    for table_name in test_case.expected_missing_tables:
        assert not table_exists(db_path=db_path, table_name=table_name)


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorDirectStatePruningE2ETestCase(
            description="auto-approved janitor prunes direct state history",
            build_command=("--no-color", "build", "--full-refresh"),
            janitor_command=(
                "--no-color",
                "janitor",
                "--auto-approve",
                "--direct-state-history-versions",
                "2",
            ),
            plan_command=("--no-color", "plan"),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "direct state pruned    4",
                "Eligible direct state pruning",
                "main._sqlbuild_fingerprints  keep latest 2",
                "main._sqlbuild_source_freshness  keep latest 2",
                "main._sqlbuild_consumption  keep latest 2",
                "main._sqlbuild_audit_results  keep latest 2",
                "Pruned 4 direct state tables.",
            ),
            expected_fingerprint_count_before=5,
            expected_fingerprint_count_after=3,
            expected_source_freshness_count_before=4,
            expected_source_freshness_count_after=2,
            expected_fingerprint_run_ids_after=("run_003", "run_002"),
            expected_source_freshness_run_ids_after=("run_003", "run_002"),
            # Newest two per audit binding.
            expected_audit_results_after=(
                ("customers.unique_ids", "run_003"),
                ("customers.unique_ids", "run_002"),
                ("orders.valid_orders", "run_003"),
                ("orders.valid_orders", "run_002"),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_direct_state_history_when_running_janitor_then_it_prunes_history(
    test_case: JanitorDirectStatePruningE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_janitor_project(
        tmp_path=tmp_path,
        project_name="janitor_direct_state_pruning_project",
        janitor_config=dedent(
            """
              enabled = true
              retention_days = 0
            """
        ),
    )
    db_path: Path = project_dir / "janitor.duckdb"

    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.build_command,
        project_dir=project_dir,
    )
    assert build_result.returncode == test_case.expected_exit_code, (
        build_result.stdout + build_result.stderr
    )
    create_direct_state_history(db_path=db_path)
    assert query_duckdb(
        db_path=db_path,
        sql="SELECT COUNT(*) FROM main._sqlbuild_fingerprints",
    ) == [(test_case.expected_fingerprint_count_before,)]
    assert query_duckdb(
        db_path=db_path,
        sql="SELECT COUNT(*) FROM main._sqlbuild_source_freshness",
    ) == [(test_case.expected_source_freshness_count_before,)]

    janitor_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command,
        project_dir=project_dir,
    )

    assert janitor_result.returncode == test_case.expected_exit_code, (
        janitor_result.stdout + janitor_result.stderr
    )
    fragment: str
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in janitor_result.stdout
    assert query_duckdb(
        db_path=db_path,
        sql="SELECT COUNT(*) FROM main._sqlbuild_fingerprints",
    ) == [(test_case.expected_fingerprint_count_after,)]
    assert query_duckdb(
        db_path=db_path,
        sql="SELECT COUNT(*) FROM main._sqlbuild_source_freshness",
    ) == [(test_case.expected_source_freshness_count_after,)]
    assert query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT run_id FROM main._sqlbuild_fingerprints "
            "WHERE node_name = 'janitor_state_probe' ORDER BY ts DESC, run_id DESC"
        ),
    ) == [(run_id,) for run_id in test_case.expected_fingerprint_run_ids_after]
    assert query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT run_id FROM main._sqlbuild_source_freshness "
            "WHERE source_name = 'raw.janitor_state_probe' "
            "ORDER BY observed_at DESC, run_id DESC"
        ),
    ) == [(run_id,) for run_id in test_case.expected_source_freshness_run_ids_after]
    assert query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT binding_key, run_id FROM main._sqlbuild_audit_results "
            "WHERE run_id LIKE 'run_%' ORDER BY binding_key, occurred_at DESC, result_id DESC"
        ),
    ) == list(test_case.expected_audit_results_after)

    plan_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.plan_command,
        project_dir=project_dir,
    )
    assert plan_result.returncode == test_case.expected_exit_code, (
        plan_result.stdout + plan_result.stderr
    )


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorInvalidConfigE2ETestCase(
            description="tracked-only janitor requires query tracking",
            command=("janitor", "--auto-approve"),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "[janitor] delete_tracked_only = true requires query change tracking",
                "            query_change_tracking = true",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_query_tracking_disabled_when_running_tracked_only_janitor_then_it_errors(
    test_case: JanitorInvalidConfigE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_janitor_project(
        tmp_path=tmp_path,
        project_name="janitor_invalid_config_project",
        settings_config="query_change_tracking = false\n",
        janitor_config="enabled = true\n",
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_stderr_fragments:
        assert fragment in result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout
