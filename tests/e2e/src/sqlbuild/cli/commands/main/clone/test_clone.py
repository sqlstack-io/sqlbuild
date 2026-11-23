"""E2E tests for sqb clone command."""

from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.clone._test_types import (
    CloneChangedPlanE2ETestCase,
    CloneDeferredSourceFunctionE2ETestCase,
    CloneE2ETestCase,
    CloneFunctionGraphE2ETestCase,
    CloneMissingUpstreamE2ETestCase,
    ClonePolicyErrorTestCase,
    ClonePythonFunctionE2ETestCase,
    DeferToMissingUpstreamE2ETestCase,
    MissingUpstreamFixesE2ETestCase,
    PartialDeferCloneE2ETestCase,
    UnwritableConsumptionDeferCloneE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.clone.helpers import (
    MISSING_UPSTREAMS,
    defer_clone_with_missing_origin,
    plan_missing_upstreams,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    REPO_ROOT,
    execute_duckdb,
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)
from tests.unit.src.sqlbuild.python_nodes.helpers import node_module_files

_CHANGED_CLONE_FIXTURE: Path = REPO_ROOT / "benchmarks/duckshop/edge_cases/changed_builds_clone"
_UPSTREAMS: tuple[str, ...] = (
    "customers",
    "orders",
    "payments",
    "products",
    "returns",
    "shipments",
)
_DEFER_DATABASE: str = "warehouse.duckdb"
_DEFER_FIXTURE: Path = REPO_ROOT / "benchmarks/duckshop/edge_cases/deferred_reads"
_DEFER_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "deferred_reads"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
        f'[connection]\ndatabase = "{_DEFER_DATABASE}"\n\n'
        '[targets.dev]\nschema = "dev"\n\n'
        '[targets.prod]\nschema = "prod"\n'
    ),
    **{
        str(path.relative_to(_DEFER_FIXTURE)): path.read_text(encoding="utf-8")
        for path in sorted(_DEFER_FIXTURE.glob("sql/models/*.sql"))
    },
}


@pytest.mark.parametrize(
    "test_case",
    [
        CloneE2ETestCase(
            description="clone uses active destination copies tables and recreates views",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "clone_project"
                    adapter = "duckdb"
                    default_target = "dev"

                    [targets.prod]
                    schema = "prod"

                    [targets.prod.connection]
                    database = "${ENV:SQLBUILD_TEST_UNUSED_ORIGIN_DATABASE}"

                    [targets.prod.clone]
                    allow_as_clone_origin = true
                    allow_as_clone_destination = false

                    [targets.dev]
                    schema = "dev"

                    [targets.dev.connection]
                    database = "clone.duckdb"

                    [targets.dev.clone]
                    allow_as_clone_origin = true
                    allow_as_clone_destination = true
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_orders
                        description: Test source raw_orders.
                        expression: |
                          SELECT *
                          FROM (VALUES (1, 100), (2, 200))
                            AS raw_orders(order_id, amount_cents)
                    """
                ).strip()
                + "\n",
                "sql/models/fact_orders.sql": dedent(
                    """
                    MODEL (description "Test model fact_orders.", materialized table);

                    SELECT order_id, amount_cents FROM __source("raw_orders")
                    """
                ).strip()
                + "\n",
                "sql/models/orders_enriched.sql": dedent(
                    """
                    MODEL (description "Test model orders_enriched.", materialized view);

                    SELECT order_id, amount_cents, amount_cents * 2 AS doubled_cents
                    FROM __ref("fact_orders")
                    """
                ).strip()
                + "\n",
                "sql/models/missing_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model missing_snapshot.", materialized table);

                    SELECT 1 AS id
                    """
                ).strip()
                + "\n",
            },
            clone_command=(
                "--no-color",
                "clone",
                "--from",
                "prod",
                "--select",
                "fact_orders",
                "orders_enriched",
                "missing_snapshot",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "fact_orders",
                "copied",
                "orders_enriched",
                "recreated_view",
                "missing_snapshot",
                "missing in origin environment",
                "\u2713 Completed with warnings",
                "WARN=1",
            ),
            expected_query_results=(
                (
                    "SELECT order_id, amount_cents FROM dev.fact_orders ORDER BY order_id",
                    ((1, 100), (2, 200)),
                ),
                (
                    "SELECT order_id, doubled_cents FROM dev.orders_enriched ORDER BY order_id",
                    ((1, 200), (2, 400)),
                ),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_clone_command_when_running_then_managed_relations_sync_as_expected(
    test_case: CloneE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="clone_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "clone.duckdb"

    import duckdb

    prod_connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    prod_connection.execute("CREATE SCHEMA prod")
    prod_connection.execute("CREATE SCHEMA dev")
    prod_connection.execute(
        "CREATE TABLE prod.fact_orders AS "
        "SELECT * FROM (VALUES (1, 100), (2, 200)) AS t(order_id, amount_cents)"
    )
    prod_connection.execute(
        "CREATE OR REPLACE VIEW prod.orders_enriched AS "
        "SELECT order_id, amount_cents, amount_cents * 2 AS doubled_cents "
        "FROM prod.fact_orders"
    )
    prod_connection.close()

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.clone_command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout, result.stdout + result.stderr

    query: str
    expected_rows: tuple[tuple[object, ...], ...]
    for query, expected_rows in test_case.expected_query_results:
        actual_rows: list[tuple[object, ...]] = query_duckdb(db_path=db_path, sql=query)
        assert tuple(tuple(row) for row in actual_rows) == expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        CloneE2ETestCase(
            description="exact clone ignores invalid origin connection and propagates fingerprint",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "clone_connection_invariant"
                    adapter = "duckdb"
                    default_target = "dev"

                    [connection]
                    database = "clone.duckdb"

                    [targets.prod]
                    schema = "prod"

                    [targets.prod.clone]
                    allow_as_clone_origin = true

                    [targets.dev]
                    schema = "dev"

                    [targets.dev.clone]
                    allow_as_clone_destination = true
                    """
                ).strip()
                + "\n",
                "sql/models/fact_orders.sql": (
                    "MODEL (description 'Test model fact_orders.', materialized table);\n\nSELECT 1 AS order_id, 100 AS amount_cents\n"
                ),
            },
            clone_command=(
                "--no-color",
                "clone",
                "--from",
                "prod",
                "--to",
                "dev",
                "--select",
                "fact_orders",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=("fact_orders", "copied", "Completed successfully"),
            expected_query_results=(
                ("SELECT order_id, amount_cents FROM dev.fact_orders", ((1, 100),)),
                (
                    "SELECT node_type, node_name FROM dev._sqlbuild_fingerprints ",
                    (("model", "fact_orders"),),
                ),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_origin_connection_when_exact_cloning_then_destination_session_is_authoritative(
    test_case: CloneE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="clone_connection_invariant",
        repo_files=test_case.repo_files,
    )
    prod_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--target", "prod", "--select", "fact_orders"),
        project_dir=project_dir,
    )
    assert prod_result.returncode == 0, prod_result.stdout + prod_result.stderr
    project_config_path: Path = project_dir / "sqlbuild_project.toml"
    project_config_path.write_text(
        project_config_path.read_text(encoding="utf-8").replace(
            '[targets.prod]\nschema = "prod"',
            '[targets.prod]\nschema = "prod"\n\n[targets.prod.connection]\n'
            'database = "${ENV:SQLBUILD_TEST_UNUSED_ORIGIN_DATABASE}"',
        ),
        encoding="utf-8",
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.clone_command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout
    for query, expected_rows in test_case.expected_query_results:
        assert tuple(query_duckdb(db_path=project_dir / "clone.duckdb", sql=query)) == expected_rows


@pytest.mark.parametrize(
    "test_case",
    (
        CloneE2ETestCase(
            description="clone copies managed source before recreating dependent view",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "managed_source_clone_project"
                    adapter = "duckdb"
                    default_target = "dev"

                    [targets.dev]
                    schema = "dev"
                    loader_schema = "raw_dev"

                    [targets.dev.connection]
                    database = "clone.duckdb"

                    [targets.dev.clone]
                    allow_as_clone_origin = true

                    [targets.prod]
                    schema = "prod"
                    loader_schema = "raw_prod"

                    [targets.prod.connection]
                    database = "clone.duckdb"

                    [targets.prod.clone]
                    allow_as_clone_destination = true
                    """
                ).strip()
                + "\n",
                **node_module_files(
                    "python/nodes/loaders/raw.py",
                    dedent(
                        """
                    from sqlbuild.loaders import loader

                    @loader
                    def raw_customers(ctx):
                        '''Test loader raw_customers.'''
                        return []
                    """
                    ).strip()
                    + "\n",
                ),
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customers
                        description: Test source raw_customers.
                        managed: true
                        write_strategy: table
                        columns:
                          - name: customer_id
                            type: INTEGER
                          - name: first_name
                            type: VARCHAR
                    """
                ).strip()
                + "\n",
                "sql/models/stg_customers.sql": dedent(
                    """
                    MODEL (description "Test model stg_customers.", materialized view);

                    SELECT customer_id, first_name FROM __source("raw_customers")
                    """
                ).strip()
                + "\n",
            },
            clone_command=("--no-color", "clone", "--from", "dev", "--to", "prod"),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "raw_customers",
                "copied",
                "stg_customers",
                "recreated_view",
            ),
            expected_query_results=(
                (
                    "SELECT customer_id, first_name FROM prod.stg_customers ORDER BY customer_id",
                    ((1, "Ada"), (2, "Grace")),
                ),
            ),
        ),
        CloneE2ETestCase(
            description="clone preserves destination deferral without copying managed source",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "managed_source_clone_project"
                    adapter = "duckdb"
                    default_target = "dev"

                    [targets.dev]
                    schema = "dev"
                    loader_schema = "raw_dev"

                    [targets.dev.connection]
                    database = "clone.duckdb"

                    [targets.dev.clone]
                    allow_as_clone_origin = true

                    [targets.prod]
                    schema = "prod"
                    loader_schema = "raw_prod"
                    defer_sources_to = "dev"

                    [targets.prod.connection]
                    database = "clone.duckdb"

                    [targets.prod.clone]
                    allow_as_clone_destination = true
                    """
                ).strip()
                + "\n",
                **node_module_files(
                    "python/nodes/loaders/raw.py",
                    dedent(
                        """
                    from sqlbuild.loaders import loader

                    @loader
                    def raw_customers(ctx):
                        '''Test loader raw_customers.'''
                        return []
                    """
                    ).strip()
                    + "\n",
                ),
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customers
                        description: Test source raw_customers.
                        managed: true
                        write_strategy: table
                        columns:
                          - name: customer_id
                            type: INTEGER
                          - name: first_name
                            type: VARCHAR
                    """
                ).strip()
                + "\n",
                "sql/models/stg_customers.sql": dedent(
                    """
                    MODEL (description "Test model stg_customers.", materialized view);

                    SELECT customer_id, first_name FROM __source("raw_customers")
                    """
                ).strip()
                + "\n",
            },
            clone_command=("--no-color", "clone", "--from", "dev", "--to", "prod"),
            expected_exit_code=0,
            expected_stdout_fragments=("stg_customers", "recreated_view"),
            expected_query_results=(
                (
                    "SELECT customer_id, first_name FROM prod.stg_customers ORDER BY customer_id",
                    ((1, "Ada"), (2, "Grace")),
                ),
                (
                    "SELECT COUNT(*) FROM information_schema.tables "
                    "WHERE table_schema = 'raw_prod' AND table_name = 'raw_customers'",
                    ((0,),),
                ),
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_managed_source_when_cloning_then_source_routing_is_preserved(
    test_case: CloneE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="managed_source_clone_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "clone.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute("CREATE SCHEMA dev")
    connection.execute("CREATE SCHEMA prod")
    connection.execute("CREATE SCHEMA raw_dev")
    connection.execute("CREATE SCHEMA raw_prod")
    connection.execute(
        "CREATE TABLE raw_dev.raw_customers AS "
        "SELECT * FROM (VALUES (1, 'Ada'), (2, 'Grace')) AS t(customer_id, first_name)"
    )
    connection.execute(
        "CREATE VIEW dev.stg_customers AS SELECT customer_id, first_name FROM raw_dev.raw_customers"
    )
    connection.close()

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.clone_command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout, result.stdout + result.stderr
    query: str
    expected_rows: tuple[tuple[object, ...], ...]
    for query, expected_rows in test_case.expected_query_results:
        actual_rows: list[tuple[object, ...]] = query_duckdb(db_path=db_path, sql=query)
        assert tuple(tuple(row) for row in actual_rows) == expected_rows


@pytest.mark.parametrize(
    "test_case",
    (
        CloneFunctionGraphE2ETestCase(
            description="recreates multi-schema functions in graph order",
            expected_stdout_fragments=("(5 resources)", "RECREATED_FUNCTIONS=2"),
            expected_resource_order=(
                "bonuses",
                "orders",
                "add_bonus",
                "order_rows",
                "enriched_orders",
            ),
            expected_query_results=(
                ("SELECT bonus FROM clone_dest.bonuses", ((5,),)),
                (
                    "SELECT order_id, amount FROM clone_dest.orders ORDER BY order_id",
                    ((1, 10), (2, 20)),
                ),
                ("SELECT clone_dest.add_bonus(10)", ((15,),)),
                ("SELECT order_id, amount FROM clone_dest.order_rows(2)", ((2, 20),)),
                (
                    "SELECT order_id, amount_with_bonus FROM clone_dest.enriched_orders "
                    "ORDER BY order_id",
                    ((1, 15), (2, 25)),
                ),
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_multi_schema_functions_when_cloning_then_destination_graph_is_queryable(
    test_case: CloneFunctionGraphE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="function_clone_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "function_clone_project"
                adapter = "duckdb"
                default_target = "dev"

                [defaults]
                schema = "default_origin"

                [targets.prod]
                schema = "preserve"

                [targets.prod.connection]
                database = "clone.duckdb"

                [targets.prod.clone]
                allow_as_clone_origin = true

                [targets.dev]
                schema = "clone_dest"

                [targets.dev.connection]
                database = "clone.duckdb"

                [targets.dev.clone]
                allow_as_clone_destination = true
                """
            ).strip()
            + "\n",
            "sql/seeds/bonuses.csv": "bonus\n5\n",
            "sql/seeds/schema.yml": dedent(
                """
                seeds:
                  - name: bonuses
                    description: Test seed bonuses.
                    schema: seed_origin
                    columns:
                      - name: bonus
                        type: INTEGER
                """
            ).strip()
            + "\n",
            "sql/models/orders.sql": dedent(
                """
                MODEL (description "Test model orders.", materialized table, schema model_origin);

                WITH amounts(order_id, amount) AS (VALUES (1, 10), (2, 20))
                SELECT * FROM amounts
                """
            ).strip()
            + "\n",
            "sql/functions/sql/add_bonus.sql": dedent(
                """
                FUNCTION (description "Test function add_bonus.",
                  schema scalar_origin,
                  arguments (amount INTEGER),
                  returns INTEGER
                );

                amount + (SELECT bonus FROM __seed("bonuses"))
                """
            ).strip()
            + "\n",
            "sql/functions/sql/order_rows.sql": dedent(
                """
                FUNCTION (description "Test function order_rows.",
                  schema table_function_origin,
                  arguments (minimum_id INTEGER),
                  returns table (
                    order_id INTEGER,
                    amount INTEGER
                  )
                );

                SELECT order_id, amount
                FROM __ref("orders")
                WHERE order_id >= minimum_id
                """
            ).strip()
            + "\n",
            "sql/models/enriched_orders.sql": dedent(
                """
                MODEL (description "Test model enriched_orders.", materialized view, schema view_origin);

                SELECT order_id, __udf("add_bonus")(amount) AS amount_with_bonus
                FROM __table_fn("order_rows")(1)
                """
            ).strip()
            + "\n",
        },
    )
    db_path: Path = project_dir / "clone.duckdb"
    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--target", "prod"),
        project_dir=project_dir,
    )
    assert build_result.returncode == 0, build_result.stdout + build_result.stderr

    clone_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "clone", "--from", "prod", "--to", "dev"),
        project_dir=project_dir,
    )

    assert clone_result.returncode == 0, clone_result.stdout + clone_result.stderr
    fragment: str
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in clone_result.stdout
    resource_positions: dict[str, int] = {
        name: clone_result.stdout.index(name) for name in test_case.expected_resource_order
    }
    assert resource_positions["bonuses"] < resource_positions["add_bonus"]
    assert resource_positions["orders"] < resource_positions["order_rows"]
    assert resource_positions["add_bonus"] < resource_positions["enriched_orders"]
    assert resource_positions["order_rows"] < resource_positions["enriched_orders"]
    query: str
    expected_rows: tuple[tuple[object, ...], ...]
    for query, expected_rows in test_case.expected_query_results:
        assert tuple(query_duckdb(db_path=db_path, sql=query)) == expected_rows


@pytest.mark.parametrize(
    "test_case",
    (
        ClonePythonFunctionE2ETestCase(
            description="registers Python UDF only on the clone execution connection",
            expected_stdout_fragments=("add_one", "recreated_function", "RECREATED_FUNCTIONS=1"),
            expected_unregistered_function_match="add_one",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_python_udf_when_cloning_then_recreates_for_clone_connection_scope(
    test_case: ClonePythonFunctionE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_function_clone_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "python_function_clone_project"
                adapter = "duckdb"
                default_target = "dev"

                [targets.prod.connection]
                database = "clone.duckdb"

                [targets.prod.clone]
                allow_as_clone_origin = true

                [targets.dev.connection]
                database = "clone.duckdb"

                [targets.dev.clone]
                allow_as_clone_destination = true
                """
            ).strip()
            + "\n",
            "sql/functions/python/add_one.py": dedent(
                """
                from sqlbuild.functions import udf

                @udf(
                    arguments={"value": "INTEGER"},
                    returns="INTEGER",
                    runtime_version="3.11",
                )
                def main(value):
                    '''Test function main.'''
                    return value + 1
                """
            ).strip()
            + "\n",
        },
    )
    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "clone", "--from", "prod", "--to", "dev"),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout
    # DuckDB Python UDF registration belongs to the connection used by clone execution.
    with pytest.raises(Exception, match=test_case.expected_unregistered_function_match):
        query_duckdb(db_path=project_dir / "clone.duckdb", sql="SELECT add_one(1)")


@pytest.mark.parametrize(
    "test_case",
    (
        CloneDeferredSourceFunctionE2ETestCase(
            description="binds cloned function to the deferred source without copying it",
            expected_stdout_fragments=("recreated_function",),
            expected_query_results=(
                ("SELECT prod.add_deferred_bonus(10)", ((17,),)),
                (
                    "SELECT COUNT(*) FROM information_schema.tables "
                    "WHERE table_schema = 'raw_prod' AND table_name = 'raw_bonus'",
                    ((0,),),
                ),
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_destination_source_deferral_when_cloning_then_function_reads_deferred_source(
    test_case: CloneDeferredSourceFunctionE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="deferred_source_function_clone_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "deferred_source_function_clone_project"
                adapter = "duckdb"
                default_target = "prod"

                [targets.dev]
                schema = "dev"
                loader_schema = "raw_dev"

                [targets.dev.connection]
                database = "clone.duckdb"

                [targets.dev.clone]
                allow_as_clone_origin = true

                [targets.prod]
                schema = "prod"
                loader_schema = "raw_prod"
                defer_sources_to = "dev"

                [targets.prod.connection]
                database = "clone.duckdb"

                [targets.prod.clone]
                allow_as_clone_destination = true
                """
            ).strip()
            + "\n",
            **node_module_files(
                "python/nodes/loaders/raw.py",
                dedent(
                    """
                from sqlbuild.loaders import loader

                @loader
                def raw_bonus(ctx):
                    '''Test loader raw_bonus.'''
                    return []
                """
                ).strip()
                + "\n",
            ),
            "sql/sources/raw.yml": dedent(
                """
                sources:
                  - name: raw_bonus
                    description: Test source raw_bonus.
                    managed: true
                    write_strategy: table
                    columns:
                      - name: bonus
                        type: INTEGER
                """
            ).strip()
            + "\n",
            "sql/functions/sql/add_deferred_bonus.sql": dedent(
                """
                FUNCTION (description "Test function add_deferred_bonus.", arguments (amount INTEGER), returns INTEGER);

                amount + (SELECT bonus FROM __source("raw_bonus"))
                """
            ).strip()
            + "\n",
        },
    )
    db_path: Path = project_dir / "clone.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute("CREATE SCHEMA raw_dev")
    connection.execute("CREATE TABLE raw_dev.raw_bonus AS SELECT 7 AS bonus")
    connection.close()

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "clone", "--from", "dev", "--to", "prod"),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout
    query: str
    expected_rows: tuple[tuple[object, ...], ...]
    for query, expected_rows in test_case.expected_query_results:
        assert tuple(query_duckdb(db_path=db_path, sql=query)) == expected_rows


@pytest.mark.parametrize(
    "test_case",
    (
        ClonePolicyErrorTestCase(
            description="denied origin identifies its enabling policy",
            origin_allowed=False,
            destination_allowed=True,
            expected_error_code="C404",
            expected_policy_key="targets.prod.clone.allow_as_clone_origin = true",
        ),
        ClonePolicyErrorTestCase(
            description="denied destination identifies its enabling policy",
            origin_allowed=True,
            destination_allowed=False,
            expected_error_code="C405",
            expected_policy_key="targets.dev.clone.allow_as_clone_destination = true",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_denied_clone_policy_when_running_then_error_identifies_configuration_fix(
    test_case: ClonePolicyErrorTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="clone_policy_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                f"""
                name = "clone_policy_project"
                adapter = "duckdb"
                default_target = "dev"

                [targets.prod]
                schema = "prod"

                [targets.prod.clone]
                allow_as_clone_origin = {str(test_case.origin_allowed).lower()}

                [targets.dev]
                schema = "dev"

                [targets.dev.clone]
                allow_as_clone_destination = {str(test_case.destination_allowed).lower()}
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "clone", "--from", "prod", "--to", "dev"),
        project_dir=project_dir,
    )
    combined_output: str = result.stdout + result.stderr

    assert result.returncode != 0
    assert test_case.expected_error_code in combined_output
    assert test_case.expected_policy_key in combined_output


@pytest.mark.parametrize(
    "test_case",
    [
        CloneChangedPlanE2ETestCase(
            description="a cloned table rebuilds and an untouched one stays skipped",
            commands=(("clone", "--from", "prod", "--select", "fact_orders"),),
            expected_fragments=("no completed record", "Unchanged (1): skipped by --changed"),
        ),
        CloneChangedPlanE2ETestCase(
            description="a cloned table rebuilds only once",
            commands=(
                ("clone", "--from", "prod", "--select", "fact_orders"),
                ("build", "--changed", "--select", "fact_orders", "dim_customers"),
            ),
            expected_fragments=("Unchanged (2): skipped by --changed",),
            unexpected_fragments=("no completed record",),
        ),
        CloneChangedPlanE2ETestCase(
            description="an upstream a build clones for its deferral rebuilds",
            commands=(("build", "--select", "orders_report", "--defer-clone-from", "prod"),),
            expected_fragments=("Prephase  defer clone", "no completed record"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cloned_table_when_planning_changed_then_it_rebuilds(
    test_case: CloneChangedPlanE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="clone_changed_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "clone_changed_project"
                adapter = "duckdb"
                default_target = "dev"

                [connection]
                database = "clone.duckdb"

                [targets.prod]
                schema = "prod"

                [targets.prod.clone]
                allow_as_clone_origin = true

                [targets.dev]
                schema = "dev"

                [targets.dev.clone]
                allow_as_clone_destination = true
                """
            ).strip()
            + "\n",
            **{
                str(path.relative_to(_CHANGED_CLONE_FIXTURE)): path.read_text(encoding="utf-8")
                for path in sorted(_CHANGED_CLONE_FIXTURE.glob("sql/models/*.sql"))
            },
        },
    )
    for command in (
        ("build", "--target", "dev"),
        ("build", "--target", "prod"),
        *test_case.commands,
    ):
        result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", *command), project_dir=project_dir
        )
        assert result.returncode == 0, result.stdout + result.stderr
        output: str = result.stdout

    plan: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan", "--changed", "--select", "fact_orders", "dim_customers"),
        project_dir=project_dir,
    )

    assert plan.returncode == 0, plan.stdout + plan.stderr
    for fragment in test_case.expected_fragments:
        assert fragment in output + plan.stdout, output + plan.stdout
    for fragment in test_case.unexpected_fragments:
        assert fragment not in plan.stdout, plan.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        MissingUpstreamFixesE2ETestCase(
            description="a clone destination is offered deferring to and cloning from each origin",
            destination_allowed=True,
            expected_fragments=(
                "  If they exist in prod:\n"
                "    read them from it for this run:  --defer-to prod\n"
                "    copy them into this target:      sqb clone --from prod -s "
                + " ".join(MISSING_UPSTREAMS)
                + "\n  Or build them too:                 add + before your selection",
            ),
            unexpected_fragments=(),
        ),
        MissingUpstreamFixesE2ETestCase(
            description="a target that may not be cloned into is offered only building",
            destination_allowed=False,
            expected_fragments=("  Build them too:  add + before your selection",),
            unexpected_fragments=("sqb clone", "--defer-to"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_missing_upstreams_when_planning_then_s301_lists_them_with_fixes(
    test_case: MissingUpstreamFixesE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir, plan_command, refused = plan_missing_upstreams(
        tmp_path=tmp_path,
        default_target="dev",
        destination_allowed=test_case.destination_allowed,
        target_args=(),
    )

    as_json: subprocess.CompletedProcess[str] = run_sqb(
        command=(*plan_command, "--json"), project_dir=project_dir
    )

    output: str = refused.stdout + refused.stderr
    assert refused.returncode == 1, output
    assert "error[S301]: 6 upstream models this build reads don't exist in target 'dev'\n" in output
    assert "  returns\n  ... 1 more (sqb plan --json lists all)\n" in output
    assert all(fragment in output for fragment in test_case.expected_fragments), output
    assert not any(fragment in output for fragment in test_case.unexpected_fragments), output
    assert json.loads(as_json.stdout)["error"]["missing"] == list(MISSING_UPSTREAMS)


@pytest.mark.parametrize(
    "test_case",
    [
        CloneMissingUpstreamE2ETestCase(
            description="the default target is cloned into without naming it",
            default_target="dev",
            target_args=(),
            expected_clone_command="sqb clone --from prod -s " + " ".join(MISSING_UPSTREAMS),
        ),
        CloneMissingUpstreamE2ETestCase(
            description="a target other than the default is named in the clone command",
            default_target="prod",
            target_args=("--target", "dev"),
            expected_clone_command="sqb clone --from prod --to dev -s "
            + " ".join(MISSING_UPSTREAMS),
        ),
        CloneMissingUpstreamE2ETestCase(
            description="a default target that does not exist still yields a runnable clone",
            default_target="retired",
            target_args=("--target", "dev"),
            expected_clone_command="sqb clone --from prod --to dev -s "
            + " ".join(MISSING_UPSTREAMS),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_missing_upstreams_when_running_printed_clone_then_plan_succeeds(
    test_case: CloneMissingUpstreamE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir, plan_command, refused = plan_missing_upstreams(
        tmp_path=tmp_path,
        default_target=test_case.default_target,
        destination_allowed=True,
        target_args=test_case.target_args,
    )
    printed: list[str] = re.findall(
        r"^ +copy them into this target: +(sqb clone .+)$", refused.stdout + refused.stderr, re.M
    )

    cloned: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", *printed[0].split()[1:]), project_dir=project_dir
    )
    planned: subprocess.CompletedProcess[str] = run_sqb(
        command=plan_command, project_dir=project_dir
    )

    assert printed == [test_case.expected_clone_command], refused.stdout + refused.stderr
    assert cloned.returncode == 0, cloned.stdout + cloned.stderr
    assert planned.returncode == 0, planned.stdout + planned.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        PartialDeferCloneE2ETestCase(
            description="a boundary cloned before another failed still rebuilds",
            dropped_origin_table="customers",
            expected_fragments=(
                "orders  no completed record",
                "Unchanged (1): skipped by --changed",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_partly_failed_defer_clone_when_planning_changed_then_cloned_boundary_rebuilds(
    test_case: PartialDeferCloneE2ETestCase,
    tmp_path: Path,
) -> None:
    cloned, planned = defer_clone_with_missing_origin(
        tmp_path=tmp_path, dropped_origin_table=test_case.dropped_origin_table
    )

    assert cloned.returncode == 1, cloned.stdout + cloned.stderr
    assert "error[C411]" in cloned.stdout + cloned.stderr
    assert planned.returncode == 0, planned.stdout + planned.stderr
    assert all(fragment in planned.stdout for fragment in test_case.expected_fragments), (
        planned.stdout
    )


@pytest.mark.parametrize(
    "test_case",
    [
        UnwritableConsumptionDeferCloneE2ETestCase(
            description="the failed boundary is reported, not the consumption write",
            dropped_origin_table="customers",
            before_clone=(
                "DROP TABLE dev._sqlbuild_consumption",
                "CREATE TABLE dev._sqlbuild_consumption (unexpected INTEGER)",
            ),
            expected_fragments=("error[C411]", "customers: missing in origin environment"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unwritable_consumption_table_when_defer_clone_partly_fails_then_c411_is_reported(
    test_case: UnwritableConsumptionDeferCloneE2ETestCase,
    tmp_path: Path,
) -> None:
    cloned, _ = defer_clone_with_missing_origin(
        tmp_path=tmp_path,
        dropped_origin_table=test_case.dropped_origin_table,
        before_clone=test_case.before_clone,
    )

    output: str = cloned.stdout + cloned.stderr
    assert cloned.returncode == 1, output
    assert all(fragment in output for fragment in test_case.expected_fragments), output


@pytest.mark.parametrize(
    "test_case",
    [
        DeferToMissingUpstreamE2ETestCase(
            description="an aliased upstream built only in the active target is missing from prod",
            upstream_sql="CREATE TABLE dev.orders_v2 AS SELECT 5 AS order_id;",
            expected_exit_code=1,
            expected_fragments=(
                "error[S301]: 1 upstream model this build reads doesn't exist in target 'prod'\n"
                "  orders\n\n  Build it too:  add + before your selection",
            ),
            report_sql=(
                "SELECT count(*) FROM information_schema.tables "
                "WHERE table_schema = 'dev' AND table_name = 'order_report'"
            ),
            expected_report_rows=[(0,)],
        ),
        DeferToMissingUpstreamE2ETestCase(
            description="an aliased upstream built in prod is read from prod under its alias",
            upstream_sql=(
                "CREATE TABLE dev.orders_v2 AS SELECT 5 AS order_id;"
                "CREATE TABLE prod.orders_v2 AS SELECT 7 AS order_id;"
            ),
            expected_exit_code=0,
            expected_fragments=(),
            report_sql="SELECT order_id FROM dev.order_report",
            expected_report_rows=[(7,)],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_defer_to_when_an_upstream_is_missing_from_the_deferred_target_then_s301(
    test_case: DeferToMissingUpstreamE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="deferred_reads", repo_files=_DEFER_PROJECT_FILES
    )
    database: Path = project_dir / _DEFER_DATABASE
    execute_duckdb(
        db_path=database, sql="CREATE SCHEMA dev; CREATE SCHEMA prod;" + test_case.upstream_sql
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--select", "order_report", "--defer-to", "prod"),
        project_dir=project_dir,
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_exit_code, output
    assert all(fragment in output for fragment in test_case.expected_fragments), output
    assert "sqb clone" not in output, output
    assert query_duckdb(db_path=database, sql=test_case.report_sql) == (
        test_case.expected_report_rows
    )
