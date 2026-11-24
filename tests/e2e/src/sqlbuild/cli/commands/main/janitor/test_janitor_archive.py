"""E2E tests for direct-mode janitor archival and archive expiry."""

from __future__ import annotations

import re
import subprocess
from datetime import UTC, datetime, timedelta
from pathlib import Path

import pytest

from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from tests.e2e.src.sqlbuild.cli.commands.main.janitor._test_types import (
    JanitorArchiveExpiryE2ETestCase,
    JanitorArchiveInterruptionE2ETestCase,
    JanitorArchiveNameFittingE2ETestCase,
    JanitorArchiveRetentionE2ETestCase,
    JanitorUnaddressableRelationE2ETestCase,
    JanitorZeroRetentionE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.janitor.helpers import (
    archive_timestamp_text,
    create_direct_state_history,
    list_archive_names,
    prepare_archive_janitor_project,
    read_current_janitor_events,
    read_janitor_events,
    write_janitor_test_settings,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    query_duckdb,
    run_sqb,
    table_exists,
)

ARCHIVE_NAME_PATTERN: re.Pattern[str] = re.compile(
    r"^_sqb_archive__(?P<timestamp>[0-9]{8}t[0-9]{6}z)__(?P<logical_name>.+)$"
)
RECENT_ARCHIVE_NAME: str = (
    f"_sqb_archive__{archive_timestamp_text(datetime.now(UTC) - timedelta(days=1))}__recent_orders"
)


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorArchiveRetentionE2ETestCase(
            description="retired tracked relation is archived after retention and then retained",
            janitor_command=("--no-color", "janitor", "--auto-approve", "--verbose"),
            retired_model_ages_days={"customers": 20, "products": 3},
            expected_archived_names=("customers",),
            expected_retained_names=("products",),
            expected_stdout_fragments=(
                "retention              14 days",
                "archive retention      14 days",
                "relations to archive   1",
                "Relations to archive",
                "main.customers  ->  main._sqb_archive__",
                "age 20d, delete after",
                "main.products  relation is newer than 14 days",
                "Archived 1 relation.",
            ),
            expected_second_run_fragments=(
                "relations to archive   0",
                "archives to delete     0",
                "archives retained      1",
                "Retained archives",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_retired_tracked_relation_past_retention_when_running_janitor_then_archives_it(
    test_case: JanitorArchiveRetentionE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_archive_janitor_project(
        tmp_path=tmp_path,
        project_name="janitor_archive_retention",
        janitor_config="enabled = true\n",
        model_names=("orders", *test_case.retired_model_ages_days),
        use_aged_adapter=True,
    )
    db_path: Path = project_dir / "janitor.duckdb"
    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert build_result.returncode == 0, build_result.stdout + build_result.stderr
    now: datetime = datetime.now(UTC)
    write_janitor_test_settings(
        project_dir=project_dir,
        created_at={
            name: now - timedelta(days=age_days)
            for name, age_days in test_case.retired_model_ages_days.items()
        },
    )
    for model_name in test_case.retired_model_ages_days:
        (project_dir / "sql" / "models" / f"{model_name}.sql").unlink()

    janitor_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command, project_dir=project_dir
    )

    assert janitor_result.returncode == 0, janitor_result.stdout + janitor_result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in janitor_result.stdout
    archive_names: tuple[str, ...] = list_archive_names(db_path=db_path)
    assert tuple(ARCHIVE_NAME_PATTERN.sub(r"\g<logical_name>", name) for name in archive_names) == (
        test_case.expected_archived_names
    )
    assert all(ARCHIVE_NAME_PATTERN.fullmatch(name) for name in archive_names)
    for name in test_case.expected_archived_names:
        assert not table_exists(db_path=db_path, table_name=name)
    for name in test_case.expected_retained_names:
        assert table_exists(db_path=db_path, table_name=name)
    events: list[tuple[object, ...]] = read_janitor_events(db_path=db_path)
    assert [(event[0], event[1], event[2], event[3]) for event in events] == [
        ("archive", name, f"main.{name}", archive_name)
        for name, archive_name in zip(test_case.expected_archived_names, archive_names, strict=True)
    ]
    assert all(str(event[4]) for event in events)

    second_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command, project_dir=project_dir
    )

    assert second_result.returncode == 0, second_result.stdout + second_result.stderr
    for fragment in test_case.expected_second_run_fragments:
        assert fragment in second_result.stdout
    assert list_archive_names(db_path=db_path) == archive_names


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorArchiveExpiryE2ETestCase(
            description="only expired strict archives in managed schemas are deleted",
            janitor_command=("--no-color", "janitor", "--auto-approve", "--verbose"),
            expected_deleted_names=(
                "_sqb_archive__20200101t000000z__old_customers_view",
                "_sqb_archive__20200101t000000z__old_products",
            ),
            expected_kept_names=(
                RECENT_ARCHIVE_NAME,
                "_sqb_archive__20201399t000000z__bad_month",
                "_SQB_ARCHIVE_20200101T000000Z__one_separator",
                "_sqb_archive_notes",
                "_SQB_ARCHIVE__20200101T000000Z__legacy_orders",
            ),
            expected_kept_other_schema_names=("_sqb_archive__20200101t000000z__orders",),
            expected_stdout_fragments=(
                "archives to delete     2",
                "archives retained      1",
                "Archives to delete",
                "main._sqb_archive__20200101t000000z__old_products  archived 2020-01-01 00:00:00",
                "expired 2020-01-15 00:00:00 UTC",
                "Retained archives",
                f"main.{RECENT_ARCHIVE_NAME}  archived",
                "main._sqb_archive__20201399t000000z__bad_month  name resembles a janitor archive "
                "but does not match the strict archive grammar",
                "main._SQB_ARCHIVE_20200101T000000Z__one_separator  name resembles a janitor archive",
                "main._sqb_archive_notes  name resembles a janitor archive",
                "main._SQB_ARCHIVE__20200101T000000Z__legacy_orders  relation name is not a plain "
                "lowercase identifier and may require quoting; janitor does not act on it",
                "\u2713 Janitor completed  archived 0, deleted 2",
            ),
            expected_delete_event_count=2,
            expected_total_event_count=5,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_archive_named_relations_when_running_janitor_then_deletes_only_expired_strict_ones(
    test_case: JanitorArchiveExpiryE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_archive_janitor_project(
        tmp_path=tmp_path,
        project_name="janitor_archive_expiry",
        janitor_config="enabled = true\narchive_retention_days = 14\n",
    )
    db_path: Path = project_dir / "janitor.duckdb"
    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert build_result.returncode == 0, build_result.stdout + build_result.stderr
    execute_duckdb(
        db_path=db_path,
        sql=(
            'CREATE TABLE main."_sqb_archive__20200101t000000z__old_products" AS SELECT 1 AS id; '
            'CREATE VIEW main."_sqb_archive__20200101t000000z__old_customers_view" AS '
            "SELECT 1 AS id; "
            f'CREATE TABLE main."{RECENT_ARCHIVE_NAME}" AS SELECT 1 AS id; '
            'CREATE TABLE main."_sqb_archive__20201399t000000z__bad_month" AS SELECT 1 AS id; '
            'CREATE TABLE main."_SQB_ARCHIVE_20200101T000000Z__one_separator" AS SELECT 1 AS id; '
            'CREATE TABLE main."_sqb_archive_notes" AS SELECT 1 AS id; '
            'CREATE TABLE main."_SQB_ARCHIVE__20200101T000000Z__legacy_orders" AS SELECT 1 AS id; '
            "CREATE SCHEMA other; "
            'CREATE TABLE other."_sqb_archive__20200101t000000z__orders" AS SELECT 1 AS id; '
            + DuckDbAdapter().render_create_janitor_event_table_sql(database=None, schema="main")
            + "; INSERT INTO main._sqlbuild_janitor_events "
            "(event_id, schema_version, event_type, occurred_at, run_id, relation_schema, "
            "original_name, archive_name, archive_qualified_name, archived_at) VALUES "
            "('misleading_delete', 1, 'delete', TIMESTAMP '2020-02-01 00:00:00', 'run_old', "
            "'main', 'old_products', '_sqb_archive__20200101t000000z__old_products', "
            "'main._sqb_archive__20200101t000000z__old_products', "
            "TIMESTAMP '2020-01-01 00:00:00'), "
            "('misleading_archive', 1, 'archive', TIMESTAMP '2020-01-01 00:00:00', 'run_old', "
            f"'main', 'recent_orders', '{RECENT_ARCHIVE_NAME}', 'main.{RECENT_ARCHIVE_NAME}', "
            "TIMESTAMP '2020-01-01 00:00:00'), "
            "('misleading_malformed', 1, 'archive', TIMESTAMP '2020-01-01 00:00:00', 'run_old', "
            "'main', 'bad_month', '_sqb_archive__20201399t000000z__bad_month', "
            "'main._sqb_archive__20201399t000000z__bad_month', TIMESTAMP '2020-01-01 00:00:00')"
        ),
    )

    janitor_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command, project_dir=project_dir
    )

    assert janitor_result.returncode == 0, janitor_result.stdout + janitor_result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in janitor_result.stdout
    for name in test_case.expected_deleted_names:
        assert not table_exists(db_path=db_path, table_name=name)
    for name in test_case.expected_kept_names:
        assert table_exists(db_path=db_path, table_name=name)
    for name in test_case.expected_kept_other_schema_names:
        assert table_exists(db_path=db_path, schema="other", table_name=name)
    events: list[tuple[object, ...]] = read_janitor_events(db_path=db_path)
    current_events: list[tuple[object, ...]] = read_current_janitor_events(
        db_path=db_path, run_id="run_old"
    )
    assert len(events) == test_case.expected_total_event_count
    assert len(current_events) == test_case.expected_delete_event_count
    assert [(event[0], event[1], event[3]) for event in current_events] == [
        ("delete", None, name) for name in sorted(test_case.expected_deleted_names)
    ]


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorZeroRetentionE2ETestCase(
            description="zero retention archives and deletes in one run with unchanged pruning",
            janitor_command=(
                "--no-color",
                "janitor",
                "--auto-approve",
                "--direct-state-history-versions",
                "2",
            ),
            stale_tables=("stale_orders",),
            stale_views=("stale_customers_view",),
            expected_stdout_fragments=(
                "retention              disabled (0 days)",
                "archive retention      0 days",
                "relations to archive   2",
                "archives to delete     2",
                "deleted in this run",
                "archived in this run",
                "Eligible direct state pruning",
                "main._sqlbuild_fingerprints  keep latest 2",
                "\u2713 Janitor completed  archived 2, deleted 2",
            ),
            expected_event_types=(
                ("archive", "stale_customers_view"),
                ("archive", "stale_orders"),
                ("delete", "stale_customers_view"),
                ("delete", "stale_orders"),
            ),
            expected_fingerprint_count_after=3,
            expected_source_freshness_count_after=2,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_zero_retention_settings_when_running_janitor_then_archives_and_deletes_in_one_run(
    test_case: JanitorZeroRetentionE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_archive_janitor_project(
        tmp_path=tmp_path,
        project_name="janitor_zero_retention",
        janitor_config=(
            "enabled = true\n"
            "retention_days = 0\n"
            "archive_retention_days = 0\n"
            "delete_tracked_only = false\n"
        ),
    )
    db_path: Path = project_dir / "janitor.duckdb"
    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert build_result.returncode == 0, build_result.stdout + build_result.stderr
    create_direct_state_history(db_path=db_path)
    execute_duckdb(
        db_path=db_path,
        sql="; ".join(
            (
                *(f"CREATE TABLE main.{name} AS SELECT 1 AS id" for name in test_case.stale_tables),
                *(f"CREATE VIEW main.{name} AS SELECT 1 AS id" for name in test_case.stale_views),
            )
        ),
    )

    janitor_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command, project_dir=project_dir
    )

    assert janitor_result.returncode == 0, janitor_result.stdout + janitor_result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in janitor_result.stdout
    for name in (*test_case.stale_tables, *test_case.stale_views):
        assert not table_exists(db_path=db_path, table_name=name)
    assert list_archive_names(db_path=db_path) == ()
    events: list[tuple[object, ...]] = read_janitor_events(db_path=db_path)
    assert tuple(sorted((str(event[0]), str(event[1])) for event in events)) == (
        test_case.expected_event_types
    )
    assert len({event[4] for event in events}) == 1
    assert query_duckdb(
        db_path=db_path, sql="SELECT COUNT(*) FROM main._sqlbuild_fingerprints"
    ) == [(test_case.expected_fingerprint_count_after,)]
    assert query_duckdb(
        db_path=db_path, sql="SELECT COUNT(*) FROM main._sqlbuild_source_freshness"
    ) == [(test_case.expected_source_freshness_count_after,)]


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorArchiveNameFittingE2ETestCase(
            description="long relation name is fitted to the adapter limit with timestamp intact",
            janitor_command=("--no-color", "janitor", "--auto-approve"),
            long_relation_name="customer_order_fulfillment_history_snapshot_backup",
            identifier_limit=48,
            expected_archive_prefix_length=len("_SQB_ARCHIVE__20260101T000000Z__"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_small_identifier_limit_when_archiving_long_name_then_fits_name_and_keeps_timestamp(
    test_case: JanitorArchiveNameFittingE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_archive_janitor_project(
        tmp_path=tmp_path,
        project_name="janitor_archive_name_fitting",
        janitor_config="enabled = true\nretention_days = 0\ndelete_tracked_only = false\n",
        use_aged_adapter=True,
    )
    db_path: Path = project_dir / "janitor.duckdb"
    write_janitor_test_settings(
        project_dir=project_dir, identifier_limit=test_case.identifier_limit
    )
    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert build_result.returncode == 0, build_result.stdout + build_result.stderr
    execute_duckdb(
        db_path=db_path,
        sql=f"CREATE TABLE main.{test_case.long_relation_name} AS SELECT 1 AS id",
    )
    before: datetime = datetime.now(UTC).replace(microsecond=0)

    janitor_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command, project_dir=project_dir
    )

    after: datetime = datetime.now(UTC)
    assert janitor_result.returncode == 0, janitor_result.stdout + janitor_result.stderr
    archive_names: tuple[str, ...] = list_archive_names(db_path=db_path)
    assert len(archive_names) == 1
    archive_name: str = archive_names[0]
    match: re.Match[str] | None = ARCHIVE_NAME_PATTERN.match(archive_name)
    assert match is not None
    archived_at: datetime = datetime.strptime(
        match.group("timestamp").upper(), "%Y%m%dT%H%M%SZ"
    ).replace(tzinfo=UTC)
    assert before <= archived_at <= after
    logical_name: str = match.group("logical_name")
    assert len(archive_name) == test_case.identifier_limit
    assert len(archive_name) - len(logical_name) == test_case.expected_archive_prefix_length
    assert logical_name != test_case.long_relation_name
    assert test_case.long_relation_name.startswith(logical_name.rsplit("_", 1)[0])
    events: list[tuple[object, ...]] = read_janitor_events(db_path=db_path)
    assert [(event[0], event[1], event[3]) for event in events] == [
        ("archive", test_case.long_relation_name, archive_name)
    ]


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorArchiveInterruptionE2ETestCase(
            description="archive without an audit row is still deleted later by name and age",
            janitor_command=("--no-color", "janitor", "--auto-approve"),
            stale_table="stale_orders",
            expected_first_exit_code=1,
            expected_first_output_fragments=("simulated janitor event write failure",),
            expected_second_exit_code=0,
            expected_second_stdout_fragments=(
                "archives to delete     1",
                "Archives to delete",
                "\u2713 Janitor completed  archived 0, deleted 1",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_audit_write_interrupted_after_rename_when_janitor_reruns_then_archive_is_deleted(
    test_case: JanitorArchiveInterruptionE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_archive_janitor_project(
        tmp_path=tmp_path,
        project_name="janitor_archive_interruption",
        janitor_config=(
            "enabled = true\n"
            "retention_days = 0\n"
            "archive_retention_days = 14\n"
            "delete_tracked_only = false\n"
        ),
        use_aged_adapter=True,
    )
    db_path: Path = project_dir / "janitor.duckdb"
    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert build_result.returncode == 0, build_result.stdout + build_result.stderr
    execute_duckdb(
        db_path=db_path, sql=f"CREATE TABLE main.{test_case.stale_table} AS SELECT 1 AS id"
    )
    write_janitor_test_settings(project_dir=project_dir, fail_janitor_event_insert=True)

    first_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command, project_dir=project_dir
    )

    assert first_result.returncode == test_case.expected_first_exit_code, (
        first_result.stdout + first_result.stderr
    )
    for fragment in test_case.expected_first_output_fragments:
        assert fragment in first_result.stdout + first_result.stderr
    stranded_archives: tuple[str, ...] = list_archive_names(db_path=db_path)
    assert len(stranded_archives) == 1
    assert not table_exists(db_path=db_path, table_name=test_case.stale_table)
    assert read_janitor_events(db_path=db_path) == []

    write_janitor_test_settings(project_dir=project_dir)
    project_file: Path = project_dir / "sqlbuild_project.toml"
    project_file.write_text(
        project_file.read_text(encoding="utf-8").replace(
            "archive_retention_days = 14", "archive_retention_days = 0"
        ),
        encoding="utf-8",
    )
    second_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command, project_dir=project_dir
    )

    assert second_result.returncode == test_case.expected_second_exit_code, (
        second_result.stdout + second_result.stderr
    )
    for fragment in test_case.expected_second_stdout_fragments:
        assert fragment in second_result.stdout
    assert list_archive_names(db_path=db_path) == ()
    assert [(event[0], event[1], event[3]) for event in read_janitor_events(db_path=db_path)] == [
        ("delete", None, stranded_archives[0])
    ]


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorUnaddressableRelationE2ETestCase(
            description="quoted mixed-case relation is reported and left while plain one archives",
            janitor_command=("--no-color", "janitor", "--auto-approve", "--verbose"),
            plain_relation="old_orders",
            quoted_relation="LegacyCustomers",
            expected_stdout_fragments=(
                "relations to archive   1",
                "main.old_orders  ->  main._sqb_archive__",
                "main.LegacyCustomers  relation name is not a plain lowercase identifier and may "
                "require quoting; janitor does not act on it",
                "Archived 1 relation.",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_quoted_mixed_case_relation_when_running_janitor_then_it_is_reported_and_untouched(
    test_case: JanitorUnaddressableRelationE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_archive_janitor_project(
        tmp_path=tmp_path,
        project_name="janitor_unaddressable_relation",
        janitor_config="enabled = true\nretention_days = 0\ndelete_tracked_only = false\n",
    )
    db_path: Path = project_dir / "janitor.duckdb"
    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert build_result.returncode == 0, build_result.stdout + build_result.stderr
    execute_duckdb(
        db_path=db_path,
        sql=(
            f"CREATE TABLE main.{test_case.plain_relation} AS SELECT 1 AS id; "
            f'CREATE TABLE main."{test_case.quoted_relation}" AS SELECT 2 AS id'
        ),
    )

    janitor_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.janitor_command, project_dir=project_dir
    )

    assert janitor_result.returncode == 0, janitor_result.stdout + janitor_result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in janitor_result.stdout
    assert not table_exists(db_path=db_path, table_name=test_case.plain_relation)
    assert table_exists(db_path=db_path, table_name=test_case.quoted_relation)
    assert query_duckdb(
        db_path=db_path, sql=f'SELECT id FROM main."{test_case.quoted_relation}"'
    ) == [(2,)]
    archive_names: tuple[str, ...] = list_archive_names(db_path=db_path)
    assert tuple(ARCHIVE_NAME_PATTERN.sub(r"\g<logical_name>", name) for name in archive_names) == (
        test_case.plain_relation,
    )
    assert [(event[0], event[1]) for event in read_janitor_events(db_path=db_path)] == [
        ("archive", test_case.plain_relation)
    ]
