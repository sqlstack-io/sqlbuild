from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.clickhouse._test_types import (
    ClickHouseNullabilityE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.clickhouse.helpers import (
    build_clickhouse_project_toml,
    build_unique_schema_name,
    cleanup_clickhouse_schema,
    create_clickhouse_schema,
    execute_clickhouse_sql,
    fetch_clickhouse_rows,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    run_sqb,
    stringify_warehouse_rows,
)

_FIRST_SELECT: str = (
    "SELECT o.order_id AS order_id, o.amount AS amount, toString(o.order_id) AS note\n"
    'FROM __source("raw_orders") AS o\n'
)


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseNullabilityE2ETestCase(
            description="nullIf on a merge model",
            model_sql=(
                "MODEL (description 'One row per order.',\n"
                "  materialized incremental,\n"
                "  incremental_strategy merge,\n"
                "  unique_key [order_id],\n"
                ");\n\n"
            ),
            edited_select=(
                "SELECT o.order_id AS order_id, nullIf(o.amount, 20) AS amount, "
                "nullIf(toString(o.order_id), '3') AS note\n"
                'FROM __source("raw_orders") AS o\n'
            ),
            expected_return_code=0,
            expected_output_fragment="Completed successfully",
            expected_rows=(("1", "10", "1"), ("2", None, "2"), ("3", "30", None)),
            expected_types=(("Nullable(Int64)", "Nullable(String)"),),
        ),
        ClickHouseNullabilityE2ETestCase(
            description="nullIf on a merge model that fails on schema changes",
            model_sql=(
                "MODEL (description 'One row per order.',\n"
                "  materialized incremental,\n"
                "  incremental_strategy merge,\n"
                "  unique_key [order_id],\n"
                "  on_schema_change fail,\n"
                ");\n\n"
            ),
            edited_select=(
                "SELECT o.order_id AS order_id, nullIf(o.amount, 20) AS amount, "
                "toString(o.order_id) AS note\n"
                'FROM __source("raw_orders") AS o\n'
            ),
            expected_return_code=1,
            expected_output_fragment="type changes: amount",
            expected_rows=(("1", "10", "1"), ("2", "20", "2"), ("3", "30", "3")),
            expected_types=(("Int64", "String"),),
        ),
        ClickHouseNullabilityE2ETestCase(
            description="nullIf on a sorting key column",
            model_sql=(
                "MODEL (description 'One row per order.',\n"
                "  materialized incremental,\n"
                "  incremental_strategy merge,\n"
                "  unique_key [order_id],\n"
                "  order_by [note],\n"
                ");\n\n"
            ),
            edited_select=(
                "SELECT o.order_id AS order_id, o.amount AS amount, "
                "nullIf(toString(o.order_id), '3') AS note\n"
                'FROM __source("raw_orders") AS o\n'
            ),
            expected_return_code=1,
            expected_output_fragment="A712",
            expected_rows=(("1", "10", "1"), ("2", "20", "2"), ("3", "30", "3")),
            expected_types=(("Int64", "String"),),
        ),
        ClickHouseNullabilityE2ETestCase(
            description="left join on an append model that ignores schema changes fails loudly",
            model_sql=(
                "MODEL (description 'One row per order load.',\n"
                "  materialized incremental,\n"
                "  incremental_strategy append,\n"
                "  on_schema_change ignore,\n"
                ");\n\n"
            ),
            edited_select=(
                "WITH r AS (SELECT toInt64(1) AS order_id, 'one' AS note)\n"
                "SELECT o.order_id AS order_id, o.amount AS amount, r.note AS note\n"
                'FROM __source("raw_orders") AS o\n'
                "LEFT JOIN r ON o.order_id = r.order_id\n"
            ),
            expected_return_code=1,
            expected_output_fragment="A713",
            expected_rows=(("1", "10", "1"), ("2", "20", "2"), ("3", "30", "3")),
            expected_types=(("Int64", "String"),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_non_nullable_target_when_model_starts_producing_nulls_then_policy_decides(
    tmp_path: Path,
    test_case: ClickHouseNullabilityE2ETestCase,
    clickhouse_e2e_config: dict[str, object],
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqb_nulls")
    config: dict[str, object] = clickhouse_e2e_config
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="clickhouse_nulls",
        repo_files={
            "sqlbuild_project.toml": build_clickhouse_project_toml(
                project_name="clickhouse_nulls", schema_name=schema_name, config=config
            ),
            "sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n"
                "    description: Synthetic orders.\n"
                f"    schema: {schema_name}\n"
                "    table: raw_orders\n"
            ),
            "models/orders.sql": test_case.model_sql + _FIRST_SELECT,
        },
    )
    create_clickhouse_schema(config=config, schema_name=schema_name)
    execute_clickhouse_sql(
        config=config,
        statements=(
            f"CREATE TABLE {schema_name}.raw_orders (order_id Int64, amount Int64) "
            "ENGINE = MergeTree ORDER BY order_id",
            f"INSERT INTO {schema_name}.raw_orders VALUES (1, 10), (2, 20), (3, 30)",
        ),
    )
    try:
        first: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build"), project_dir=project_dir
        )
        assert first.returncode == 0, first.stdout + first.stderr
        (project_dir / "models" / "orders.sql").write_text(
            test_case.model_sql + test_case.edited_select
        )

        second: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build"), project_dir=project_dir
        )

        assert second.returncode == test_case.expected_return_code, second.stdout + second.stderr
        assert test_case.expected_output_fragment in second.stdout + second.stderr
        assert stringify_warehouse_rows(
            fetch_clickhouse_rows(
                config=config,
                sql=f"SELECT order_id, amount, note FROM {schema_name}.orders "
                "ORDER BY order_id, note NULLS LAST",
            )
        ) == (test_case.expected_rows)
        assert fetch_clickhouse_rows(
            config=config,
            sql=f"SELECT toTypeName(amount), toTypeName(note) FROM {schema_name}.orders LIMIT 1",
        ) == (test_case.expected_types)
    finally:
        cleanup_clickhouse_schema(config=config, schema_name=schema_name)
