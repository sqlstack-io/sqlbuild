"""E2E tests for janitor progress output, inspection reads, concurrency and audit batching."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.janitor._test_types import (
    JanitorConcurrencyE2ETestCase,
    JanitorConcurrentFailureE2ETestCase,
    JanitorInspectionCountE2ETestCase,
    JanitorJsonE2ETestCase,
    JanitorJsonErrorE2ETestCase,
    JanitorJsonFailedRunE2ETestCase,
    JanitorProgressE2ETestCase,
    JanitorShortenedArchiveE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.janitor.helpers import (
    archive_line_positions,
    assert_in_order,
    count_statements,
    janitor_end_state,
    prepare_probe_janitor_project,
    read_probe_statements,
    rename_probe_models,
    statement_index,
    write_probe_settings,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    query_duckdb,
    run_sqb,
    table_exists,
)

APPROVED_JANITOR: tuple[str, ...] = ("--no-color", "janitor", "--auto-approve")


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorProgressE2ETestCase(
            description="each phase announces its work, streams each action and closes",
            janitor_command=APPROVED_JANITOR,
            stale_tables=12,
            expected_exit_code=0,
            expected_ordered_lines=(
                "Janitor preview  default",
                "Relations to archive",
                "  main.old_orders_009  ->  main._sqb_archive__",
                "  ... and 2 more (use --verbose to show all)",
                "Skipped objects (",
                "Archiving 12 relations...",
                "   1/12  archived  main.old_orders_000 -> main._sqb_archive__",
                "  OK  ",
                "  12/12  archived  main.old_orders_011 -> main._sqb_archive__",
                "Archived 12 relations. (",
                "Deleting 1 expired archive...",
                "  1/1  deleted   main._sqb_archive__20200101t000000z__old_products  OK",
                "Deleted 1 expired archive. (",
                "Pruning 2 direct state tables...",
                "  1/2  pruned    main._sqlbuild_fingerprints  OK",
                "  2/2  pruned    main._sqlbuild_consumption   OK",
                "Pruned 2 direct state tables. (",
                "\u2713 Janitor completed  archived 12, deleted 1, pruned 2  (",
            ),
            expected_archive_events=12,
            unexpected_fragments=(
                "  main.old_orders_010  ->  ",
                "main._sqlbuild_fingerprints  relation matches exclude pattern",
            ),
        ),
        JanitorProgressE2ETestCase(
            description="verbose preview lists every relation and skipped object",
            janitor_command=(*APPROVED_JANITOR, "--verbose"),
            stale_tables=12,
            expected_exit_code=0,
            expected_ordered_lines=(
                "  main.old_orders_011  ->  main._sqb_archive__",
                "Skipped objects\n  main._sqlbuild_consumption  relation matches exclude pattern",
                "  main._sqlbuild_fingerprints  relation matches exclude pattern",
                "\u2713 Janitor completed  archived 12, deleted 1, pruned 2  (",
            ),
            expected_archive_events=12,
            unexpected_fragments=("... and", "Skipped objects ("),
        ),
        JanitorProgressE2ETestCase(
            description="a failing rename keeps earlier lines, marks the item and fails the run",
            janitor_command=APPROVED_JANITOR,
            stale_tables=12,
            expected_exit_code=1,
            fail_rename="old_orders_005",
            expected_ordered_lines=(
                "Archiving 12 relations...",
                "   5/12  archived  main.old_orders_004 -> main._sqb_archive__",
                "   6/12  archived  main.old_orders_005 -> main._sqb_archive__",
                "  FAIL  ",
                "simulated rename failure for old_orders_005",
                "\u2717 Archiving failed  after 5 of 12 relations  (",
                "\u2717 Janitor failed  archived 5, deleted 0, pruned 0  (",
            ),
            expected_archive_events=5,
            unexpected_fragments=("7/12", "Deleting 1 expired archive", "Janitor completed"),
            expected_remaining_tables=("old_orders_005", "old_orders_011"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_stale_relations_when_running_janitor_then_progress_streams_in_order(
    test_case: JanitorProgressE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_probe_janitor_project(
        tmp_path=tmp_path,
        project_name="janitor_progress",
        stale_tables={"main": test_case.stale_tables},
    )
    write_probe_settings(project_dir=project_dir, fail_rename=test_case.fail_rename)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command, project_dir=project_dir
    )

    db_path: Path = project_dir / "janitor.duckdb"
    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert_in_order(output=result.stdout, lines=test_case.expected_ordered_lines)
    fragment: str
    for fragment in test_case.unexpected_fragments:
        assert fragment not in result.stdout
    assert query_duckdb(
        db_path=db_path,
        sql="SELECT count(*) FROM main._sqlbuild_janitor_events WHERE event_type = 'archive'",
    ) == [(test_case.expected_archive_events,)]
    table: str
    for table in test_case.expected_remaining_tables:
        assert table_exists(db_path=db_path, table_name=table)


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorJsonE2ETestCase(
            description="stdout is one JSON document and progress goes to stderr",
            janitor_command=("janitor", "--auto-approve", "--json"),
            stale_tables=3,
            expected_status="completed",
            expected_archived=3,
            expected_item_statuses=("ok",) * 6,
            expected_stderr_fragments=(
                "Janitor preview  default",
                "Archiving 3 relations...",
                "  1/3  archived  main.old_orders_000 -> main._sqb_archive__",
                "\u2713 Janitor completed  archived 3, deleted 1, pruned 2",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_json_flag_when_running_janitor_then_stdout_holds_plan_and_result(
    test_case: JanitorJsonE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_probe_janitor_project(
        tmp_path=tmp_path,
        project_name="janitor_json",
        stale_tables={"main": test_case.stale_tables},
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command, project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    payload: dict[str, Any] = json.loads(result.stdout)
    assert payload["status"] == test_case.expected_status
    assert len(payload["plan"]["relations_to_archive"]) == test_case.stale_tables
    assert payload["result"]["counts"]["archived"] == test_case.expected_archived
    assert tuple(item["status"] for item in payload["result"]["items"]) == (
        test_case.expected_item_statuses
    )
    assert payload["error"] is None
    assert_in_order(output=result.stderr, lines=test_case.expected_stderr_fragments)


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorJsonErrorE2ETestCase(
            description="a refused non-interactive confirmation prints a JSON error document",
            janitor_command=("janitor", "--json"),
            expected_exit_code=1,
            expected_error_code="C503",
            expected_stderr_fragment="Janitor preview  default",
        ),
        JanitorJsonErrorE2ETestCase(
            description="an invalid setting prints a JSON error document before any inspection",
            janitor_command=("janitor", "--json", "--retention-days", "-1"),
            expected_exit_code=1,
            expected_error_code="C501",
            expected_stderr_fragment="error[C501]",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_json_flag_when_janitor_fails_before_cleanup_then_stdout_holds_error_document(
    test_case: JanitorJsonErrorE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_probe_janitor_project(
        tmp_path=tmp_path, project_name="janitor_json_error", stale_tables={"main": 2}
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command, project_dir=project_dir
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    payload: dict[str, Any] = json.loads(result.stdout)
    assert payload["error"]["code"] == test_case.expected_error_code
    assert test_case.expected_stderr_fragment in result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorJsonFailedRunE2ETestCase(
            description="a run failing mid-phase prints exactly one JSON document",
            janitor_command=("janitor", "--auto-approve", "--json"),
            fail_rename="old_orders_001",
            expected_exit_code=1,
            expected_status="failed",
            expected_error_fragment="simulated rename failure for old_orders_001",
            expected_failed_items=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_json_flag_when_janitor_fails_mid_phase_then_stdout_holds_one_run_document(
    test_case: JanitorJsonFailedRunE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_probe_janitor_project(
        tmp_path=tmp_path, project_name="janitor_json_failed", stale_tables={"main": 3}
    )
    write_probe_settings(project_dir=project_dir, fail_rename=test_case.fail_rename)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command, project_dir=project_dir
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    payload: dict[str, Any] = json.loads(result.stdout)
    assert payload["status"] == test_case.expected_status
    assert test_case.expected_error_fragment in payload["error"]
    assert [item["status"] for item in payload["result"]["items"]].count("failed") == (
        test_case.expected_failed_items
    )


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorInspectionCountE2ETestCase(
            description="each schema is listed once, its views read once, and events chunked",
            stale_tables={"main": 150, "mart": 3, "staging": 3},
            renamed_models=("revenue", "customers"),
            expected_listings_per_schema=1,
            expected_multi_schema_listings=0,
            expected_existence_queries_per_schema=1,
            expected_view_definition_reads=3,
            expected_batched_view_definition_reads=3,
            expected_event_inserts={"main": 3, "mart": 2, "staging": 2},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_several_schemas_when_running_janitor_then_reads_and_writes_scale_per_schema(
    test_case: JanitorInspectionCountE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_probe_janitor_project(
        tmp_path=tmp_path,
        project_name="janitor_counts",
        stale_tables=test_case.stale_tables,
        renamed_models=test_case.renamed_models,
    )
    rename_probe_models(
        project_dir=project_dir,
        schemas=tuple(test_case.stale_tables),
        renamed_models=test_case.renamed_models,
    )
    write_probe_settings(project_dir=project_dir)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=APPROVED_JANITOR, project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    statements: tuple[str, ...] = read_probe_statements(project_dir=project_dir)
    schema: str
    inserts: int
    for schema, inserts in test_case.expected_event_inserts.items():
        assert count_statements(
            statements=statements,
            fragments=("FROM information_schema.tables", f"table_schema IN ('{schema}')"),
            excluded=("table_catalog",),
        ) == (test_case.expected_listings_per_schema)
        assert count_statements(
            statements=statements,
            fragments=(f"SELECT event_id FROM {schema}._sqlbuild_janitor_events WHERE",),
        ) == (test_case.expected_existence_queries_per_schema)
        assert count_statements(
            statements=statements,
            fragments=(f"INSERT INTO {schema}._sqlbuild_janitor_events",),
        ) == (inserts)
    assert count_statements(
        statements=statements, fragments=("FROM information_schema.tables", "', '")
    ) == (test_case.expected_multi_schema_listings)
    assert count_statements(statements=statements, fragments=("FROM duckdb_views()",)) == (
        test_case.expected_view_definition_reads
    )
    assert count_statements(
        statements=statements, fragments=("FROM duckdb_views()", "view_name IN (")
    ) == (test_case.expected_batched_view_definition_reads)


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorConcurrencyE2ETestCase(
            description="concurrent actions leave the same end state as sequential ones",
            stale_tables={"main": 30, "staging": 5},
            concurrency=4,
            expected_exit_code=0,
            expected_connection_line="Connecting to probe_janitor_duckdb (3 connections)",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_concurrent_adapter_when_running_janitor_then_matches_sequential_end_state(
    test_case: JanitorConcurrencyE2ETestCase, tmp_path: Path
) -> None:
    sequential_dir: Path = prepare_probe_janitor_project(
        tmp_path=tmp_path / "sequential",
        project_name="janitor_sequential",
        stale_tables=test_case.stale_tables,
    )
    concurrent_dir: Path = prepare_probe_janitor_project(
        tmp_path=tmp_path / "concurrent",
        project_name="janitor_concurrent",
        stale_tables=test_case.stale_tables,
        concurrency=test_case.concurrency,
    )
    write_probe_settings(project_dir=sequential_dir)
    write_probe_settings(project_dir=concurrent_dir, concurrent_ddl_limit=test_case.concurrency)

    sequential: subprocess.CompletedProcess[str] = run_sqb(
        command=APPROVED_JANITOR, project_dir=sequential_dir
    )
    concurrent: subprocess.CompletedProcess[str] = run_sqb(
        command=APPROVED_JANITOR, project_dir=concurrent_dir
    )

    total: int = sum(test_case.stale_tables.values())
    schemas: tuple[str, ...] = tuple(test_case.stale_tables)
    assert sequential.returncode == test_case.expected_exit_code, sequential.stdout
    assert concurrent.returncode == test_case.expected_exit_code, concurrent.stdout
    assert test_case.expected_connection_line in concurrent.stdout
    assert test_case.expected_connection_line not in sequential.stdout
    assert archive_line_positions(concurrent.stdout) == tuple(
        (index, total) for index in range(1, total + 1)
    )
    assert janitor_end_state(db_path=concurrent_dir / "janitor.duckdb", schemas=schemas) == (
        janitor_end_state(db_path=sequential_dir / "janitor.duckdb", schemas=schemas)
    )


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorConcurrentFailureE2ETestCase(
            description="a failing concurrent rename is reported alone and events stay complete",
            stale_tables={"main": 30},
            concurrency=4,
            fail_rename="old_orders_012",
            expected_exit_code=1,
            expected_failed_lines=1,
            expected_fragments=(
                "simulated rename failure for old_orders_012",
                "\u2717 Archiving failed  after ",
                "\u2717 Janitor failed  archived ",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failing_concurrent_rename_when_running_janitor_then_events_match_renames(
    test_case: JanitorConcurrentFailureE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_probe_janitor_project(
        tmp_path=tmp_path,
        project_name="janitor_concurrent_failure",
        stale_tables=test_case.stale_tables,
        concurrency=test_case.concurrency,
    )
    write_probe_settings(
        project_dir=project_dir,
        concurrent_ddl_limit=test_case.concurrency,
        fail_rename=test_case.fail_rename,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=APPROVED_JANITOR, project_dir=project_dir
    )

    db_path: Path = project_dir / "janitor.duckdb"
    positions: tuple[tuple[int, int], ...] = archive_line_positions(result.stdout)
    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert result.stdout.count("  FAIL  ") == test_case.expected_failed_lines
    assert_in_order(output=result.stdout, lines=test_case.expected_fragments)
    assert tuple(index for index, _ in positions) == tuple(range(1, len(positions) + 1))
    assert table_exists(db_path=db_path, table_name=test_case.fail_rename)
    assert query_duckdb(
        db_path=db_path,
        sql="SELECT count(*) FROM main._sqlbuild_janitor_events WHERE event_type = 'archive'",
    ) == query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'main' "
            "AND starts_with(table_name, '_sqb_archive__') "
            "AND contains(table_name, '__old_orders_')"
        ),
    )


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorShortenedArchiveE2ETestCase(
            description="a shortened archive name gets its event right after its rename",
            long_relation_name="legacy_orders_with_a_very_long_descriptive_name_kept_for_audit",
            identifier_limit=48,
            expected_rows_in_event_insert=1,
            expected_event_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_shortened_archive_name_when_archiving_then_event_is_written_immediately(
    test_case: JanitorShortenedArchiveE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_probe_janitor_project(
        tmp_path=tmp_path, project_name="janitor_shortened", stale_tables={"main": 3}
    )
    execute_duckdb(
        db_path=project_dir / "janitor.duckdb",
        sql=f"CREATE TABLE main.{test_case.long_relation_name} AS SELECT 1 AS id",
    )
    write_probe_settings(project_dir=project_dir, identifier_limit=test_case.identifier_limit)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=APPROVED_JANITOR, project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    statements: tuple[str, ...] = read_probe_statements(project_dir=project_dir)
    rename: int = statement_index(
        statements=statements, prefix=f"ALTER TABLE main.{test_case.long_relation_name} RENAME"
    )
    event_insert: str = statements[rename + 1]
    assert event_insert.startswith("INSERT INTO main._sqlbuild_janitor_events")
    assert f"'{test_case.long_relation_name}'" in event_insert
    assert event_insert.count("), (") + 1 == test_case.expected_rows_in_event_insert
    assert query_duckdb(
        db_path=project_dir / "janitor.duckdb",
        sql=(
            "SELECT count(*) FROM main._sqlbuild_janitor_events "
            f"WHERE original_name = '{test_case.long_relation_name}'"
        ),
    ) == [(test_case.expected_event_count,)]
