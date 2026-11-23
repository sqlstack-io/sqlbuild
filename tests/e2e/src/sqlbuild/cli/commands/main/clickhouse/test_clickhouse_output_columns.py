from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.clickhouse._test_types import (
    ClickHouseOutputColumnNameE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.clickhouse.helpers import (
    build_clickhouse_project_toml,
    build_unique_schema_name,
    cleanup_clickhouse_schema,
    create_clickhouse_schema,
    execute_clickhouse_sql,
    fetch_clickhouse_rows,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseOutputColumnNameE2ETestCase(
            description="an unaliased joined column fails with A711",
            selected_order_id="o.order_id",
            expected_return_code=1,
            expected_output_fragments=(
                "A711",
                "'o.order_id' instead of 'order_id'",
                "o.order_id AS order_id",
            ),
            expected_columns=(),
        ),
        ClickHouseOutputColumnNameE2ETestCase(
            description="an aliased joined column builds",
            selected_order_id="o.order_id AS order_id",
            expected_return_code=0,
            expected_output_fragments=("Completed successfully",),
            expected_columns=(("order_id",), ("quantity",), ("payment_cents",)),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_three_table_join_when_building_on_clickhouse_then_column_names_match_analysis(
    tmp_path: Path,
    test_case: ClickHouseOutputColumnNameE2ETestCase,
    clickhouse_e2e_config: dict[str, object],
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqb_names")
    config: dict[str, object] = clickhouse_e2e_config
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="clickhouse_names",
        repo_files={
            "sqlbuild_project.toml": build_clickhouse_project_toml(
                project_name="clickhouse_names", schema_name=schema_name, config=config
            ),
            "sql/sources/raw.yml": (
                "sources:\n"
                + "".join(
                    f"  - name: {name}\n    description: Synthetic {name}.\n"
                    f"    schema: {schema_name}\n    table: {name}\n"
                    for name in ("orders", "products", "payments")
                )
            ),
            "sql/models/order_lines.sql": (
                "MODEL (description 'One row per order.');\n\n"
                f"SELECT {test_case.selected_order_id}, o.quantity AS quantity, "
                "p.amount AS payment_cents\n"
                'FROM __source("orders") AS o\n'
                'JOIN __source("products") AS w ON o.product_id = w.product_id\n'
                'LEFT JOIN __source("payments") AS p ON o.order_id = p.order_id\n'
            ),
        },
    )
    create_clickhouse_schema(config=config, schema_name=schema_name)
    execute_clickhouse_sql(
        config=config,
        statements=(
            f"CREATE TABLE {schema_name}.orders (order_id Int64, product_id Int64, "
            "quantity Int64) ENGINE = MergeTree ORDER BY order_id",
            f"CREATE TABLE {schema_name}.products (product_id Int64, order_id Int64) "
            "ENGINE = MergeTree ORDER BY product_id",
            f"CREATE TABLE {schema_name}.payments (order_id Int64, amount Int64) "
            "ENGINE = MergeTree ORDER BY order_id",
            f"INSERT INTO {schema_name}.orders VALUES (1, 7, 2)",
            f"INSERT INTO {schema_name}.products VALUES (7, 0)",
            f"INSERT INTO {schema_name}.payments VALUES (1, 900)",
        ),
    )
    try:
        result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build"), project_dir=project_dir
        )

        assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
        assert [
            fragment in result.stdout + result.stderr
            for fragment in test_case.expected_output_fragments
        ] == [True] * len(test_case.expected_output_fragments), result.stdout + result.stderr
        assert fetch_clickhouse_rows(
            config=config,
            sql=f"SELECT name FROM system.columns WHERE database = '{schema_name}' "
            "AND table = 'order_lines' ORDER BY position",
        ) == (test_case.expected_columns)
    finally:
        cleanup_clickhouse_schema(config=config, schema_name=schema_name)
