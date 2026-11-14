"""ClickHouse janitor usage: system.query_log shows who reads stale tables and view chains."""

from __future__ import annotations

import subprocess
from contextlib import ExitStack
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.clickhouse._test_types import (
    ClickHouseJanitorUsageE2ETestCase,
    ClickHouseJanitorUsageUncheckedE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.clickhouse.helpers import (
    execute_clickhouse_sql,
    limited_user,
    prepare_janitor_usage_project,
    read_as,
    stale_relation_names,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb

APPROVED_JANITOR: tuple[str, ...] = ("--no-color", "janitor", "--auto-approve")


@pytest.mark.parametrize(
    "test_case",
    (
        ClickHouseJanitorUsageE2ETestCase(
            description="a view chain read by another user is in use and archived by default",
            janitor_command=APPROVED_JANITOR,
            expected_fragments=(
                "Checked relation usage: 3 of 4 in use. (",
                "\nIn use\n",
                ".old_orders  queried ",
                " by {reader} (1 query in 7 days); frozen, still in use\n",
                ".old_orders_v  queried ",
                ".old_orders_v2  queried ",
                " by {reader} (1 query in 7 days)\n",
            ),
            unexpected_fragments=("Kept (in use)", "old_customers  queried", "usage not checked"),
            expected_remaining=(),
        ),
        ClickHouseJanitorUsageE2ETestCase(
            description="--keep-in-use keeps the read table and both views of the chain",
            janitor_command=(*APPROVED_JANITOR, "--keep-in-use"),
            expected_fragments=(
                "Checked relation usage: 3 of 4 in use. (",
                "  kept (in use)          3\n",
                "\nKept (in use)\n",
                ".old_orders_v2  queried ",
            ),
            unexpected_fragments=("\nIn use\n", "old_orders_v  ->", "usage not checked"),
            expected_remaining=("old_orders", "old_orders_v", "old_orders_v2"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_query_log_when_running_janitor_then_view_chain_reads_are_in_use(
    test_case: ClickHouseJanitorUsageE2ETestCase,
    clickhouse_e2e_config: dict[str, object],
    janitor_schema: str,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_janitor_usage_project(
        tmp_path=tmp_path, config=clickhouse_e2e_config, schema=janitor_schema
    )
    with limited_user(
        config=clickhouse_e2e_config, database=janitor_schema, privileges="SELECT"
    ) as reader:
        read_as(config=reader, sql=f"SELECT sum(id) FROM {janitor_schema}.old_orders_v")
        execute_clickhouse_sql(config=clickhouse_e2e_config, statements=("SYSTEM FLUSH LOGS",))

        result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.janitor_command, project_dir=project_dir
        )

        reader_name: str = str(reader["user"])
    assert result.returncode == 0, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment.format(reader=reader_name) in result.stdout, result.stdout
    for fragment in test_case.unexpected_fragments:
        assert fragment not in result.stdout, result.stdout
    assert stale_relation_names(config=clickhouse_e2e_config, schema=janitor_schema) == (
        test_case.expected_remaining
    )


@pytest.mark.parametrize(
    "test_case",
    (
        ClickHouseJanitorUsageUncheckedE2ETestCase(
            description="--keep-in-use stops when the role may not read system.query_log",
            connection_settings="",
            grants=(),
            expected_fragments=(
                "usage was not checked: cannot read system.query_log (needs query logging on "
                "and SELECT on system.query_log); nothing was archived",
            ),
        ),
        ClickHouseJanitorUsageUncheckedE2ETestCase(
            description="--keep-in-use stops when query logging is off for the connection",
            connection_settings="\n[connection.settings]\nlog_queries = 0\n",
            grants=("GRANT SELECT ON system.query_log TO {user}",),
            expected_fragments=(
                "usage was not checked: query logging is off for this connection "
                "(log_queries = 0); nothing was archived",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_no_query_log_when_keeping_in_use_then_janitor_stops_and_archives_nothing(
    test_case: ClickHouseJanitorUsageUncheckedE2ETestCase,
    clickhouse_e2e_config: dict[str, object],
    janitor_schema: str,
    tmp_path: Path,
) -> None:
    with ExitStack() as stack:
        janitor: dict[str, object] = stack.enter_context(
            limited_user(config=clickhouse_e2e_config, database=janitor_schema, privileges="ALL")
        )
        execute_clickhouse_sql(
            config=clickhouse_e2e_config,
            statements=(
                f"CREATE DATABASE {janitor_schema}",
                *(grant.format(user=janitor["user"]) for grant in test_case.grants),
            ),
        )
        project_dir: Path = prepare_janitor_usage_project(
            tmp_path=tmp_path,
            config=janitor,
            schema=janitor_schema,
            connection_settings=test_case.connection_settings,
        )
        stale: tuple[str, ...] = stale_relation_names(
            config=clickhouse_e2e_config, schema=janitor_schema
        )

        result: subprocess.CompletedProcess[str] = run_sqb(
            command=(*APPROVED_JANITOR, "--keep-in-use"), project_dir=project_dir
        )

    assert result.returncode != 0, result.stdout
    output: str = " ".join((result.stdout + result.stderr).split())
    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in output, output
    assert stale
    assert stale_relation_names(config=clickhouse_e2e_config, schema=janitor_schema) == stale
