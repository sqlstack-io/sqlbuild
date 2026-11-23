from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.clickhouse._test_types import (
    ClickHouseCloneE2ETestCase,
    ClickHouseDiffE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.clickhouse.helpers import (
    build_two_target_toml,
    build_unique_schema_name,
    cleanup_clickhouse_schema,
    create_clickhouse_schema,
    execute_clickhouse_sql,
    fetch_clickhouse_rows,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_FACT_ORDERS_SQL: str = (
    "MODEL (description 'One row per order.',\n"
    "  materialized table,\n"
    "  unique_key [order_id],\n"
    "  cursor order_id,\n"
    "  cursor_type integer,\n"
    ");\n\n"
    "SELECT toInt64(1) AS order_id, toInt64(100) AS amount_cents\n"
)


@pytest.mark.parametrize(
    "test_case",
    (
        ClickHouseDiffE2ETestCase(
            description="schema-only diff ignores row drift",
            command=("--no-color", "diff", "prod:dev", "--schema-only", "--select", "fact_orders"),
            mutation_sql=("ALTER TABLE {dev} UPDATE amount_cents = 105 WHERE order_id = 1",),
            expected_stdout_fragments=("fact_orders", "No schema differences."),
            expected_return_code=0,
        ),
        ClickHouseDiffE2ETestCase(
            description="full diff reports row mismatch",
            command=("--no-color", "diff", "prod:dev", "--full", "--select", "fact_orders"),
            mutation_sql=("ALTER TABLE {dev} UPDATE amount_cents = 105 WHERE order_id = 1",),
            expected_stdout_fragments=("amount_cents", "mismatches=1", "order_id=1 | 100 -> 105"),
            expected_return_code=1,
        ),
        ClickHouseDiffE2ETestCase(
            description="bounded diff filters rows below integer cursor bound",
            command=("--no-color", "diff", "prod:dev", "--bounded", "2", "--select", "fact_orders"),
            mutation_sql=(
                "ALTER TABLE {dev} UPDATE amount_cents = 105 WHERE order_id = 1",
                "ALTER TABLE {dev} UPDATE amount_cents = 205 WHERE order_id = 2",
            ),
            expected_stdout_fragments=("amount_cents", "mismatches=1", "order_id=2 | 200 -> 205"),
            expected_return_code=1,
            expected_absent_stdout_fragments=("order_id=1 | 100 -> 105",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_clickhouse_targets_when_running_diff_then_all_modes_execute_end_to_end(
    test_case: ClickHouseDiffE2ETestCase,
    tmp_path: Path,
    clickhouse_e2e_config: dict[str, object],
) -> None:
    config: dict[str, object] = clickhouse_e2e_config
    prod_schema: str = build_unique_schema_name(prefix="sqb_diff_prod")
    dev_schema: str = build_unique_schema_name(prefix="sqb_diff_dev")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="clickhouse_diff_project",
        repo_files={
            "sqlbuild_project.toml": build_two_target_toml(
                project_name="clickhouse_diff_project",
                prod_schema=prod_schema,
                dev_schema=dev_schema,
                config=config,
            ),
            "sql/models/fact_orders.sql": _FACT_ORDERS_SQL,
        },
    )
    try:
        for schema in (prod_schema, dev_schema):
            create_clickhouse_schema(config=config, schema_name=schema)
            execute_clickhouse_sql(
                config=config,
                statements=(
                    f"CREATE TABLE {schema}.fact_orders (order_id Int64, amount_cents Int64) "
                    "ENGINE = MergeTree ORDER BY order_id",
                    f"INSERT INTO {schema}.fact_orders VALUES (1, 100), (2, 200), (3, 300)",
                ),
            )
        execute_clickhouse_sql(
            config=config,
            statements=tuple(
                mutation.format(dev=f"{dev_schema}.fact_orders")
                for mutation in test_case.mutation_sql
            ),
        )

        result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.command, project_dir=project_dir
        )

        assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
        for fragment in test_case.expected_stdout_fragments:
            assert fragment in result.stdout, result.stdout + result.stderr
        for fragment in test_case.expected_absent_stdout_fragments:
            assert fragment not in result.stdout, result.stdout + result.stderr
    finally:
        cleanup_clickhouse_schema(config=config, schema_name=dev_schema)
        cleanup_clickhouse_schema(config=config, schema_name=prod_schema)


@pytest.mark.parametrize(
    "test_case",
    (
        ClickHouseCloneE2ETestCase(
            description="clone hard-links tables and recreates views",
            expected_stdout_fragments=("fact_orders", "order_summary"),
            expected_rows=((1, 100),),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_built_dev_target_when_cloning_to_prod_then_clickhouse_clones_relations(
    test_case: ClickHouseCloneE2ETestCase,
    tmp_path: Path,
    clickhouse_e2e_config: dict[str, object],
) -> None:
    config: dict[str, object] = clickhouse_e2e_config
    prod_schema: str = build_unique_schema_name(prefix="sqb_clone_prod")
    dev_schema: str = build_unique_schema_name(prefix="sqb_clone_dev")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="clickhouse_clone_project",
        repo_files={
            "sqlbuild_project.toml": build_two_target_toml(
                project_name="clickhouse_clone_project",
                prod_schema=prod_schema,
                dev_schema=dev_schema,
                config=config,
            ),
            "sql/models/fact_orders.sql": _FACT_ORDERS_SQL,
            "sql/models/order_summary.sql": (
                "MODEL (description 'Order summary.', materialized view);\n\n"
                'SELECT order_id, amount_cents FROM __ref("fact_orders")\n'
            ),
        },
    )
    try:
        built: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build"), project_dir=project_dir
        )
        assert built.returncode == 0, built.stdout + built.stderr

        cloned: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "clone", "--from", "dev", "--to", "prod"),
            project_dir=project_dir,
        )

        assert cloned.returncode == 0, cloned.stdout + cloned.stderr
        for fragment in test_case.expected_stdout_fragments:
            assert fragment in cloned.stdout, cloned.stdout + cloned.stderr
        assert (
            fetch_clickhouse_rows(
                config=config,
                sql=f"SELECT order_id, amount_cents FROM {prod_schema}.order_summary",
            )
            == test_case.expected_rows
        )
    finally:
        cleanup_clickhouse_schema(config=config, schema_name=dev_schema)
        cleanup_clickhouse_schema(config=config, schema_name=prod_schema)
