"""E2E tests for snapshot build behavior."""

from __future__ import annotations

import json
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    SnapshotCheckBuildE2ETestCase,
    SnapshotCheckFailureBuildE2ETestCase,
    SnapshotDmlFailureRollbackBuildE2ETestCase,
    SnapshotFailureConsistencyBuildE2ETestCase,
    SnapshotFullRefreshFailureBuildE2ETestCase,
    SnapshotFullRefreshSuccessBuildE2ETestCase,
    SnapshotHistoricalCheckBuildE2ETestCase,
    SnapshotHistoricalTimestampBuildE2ETestCase,
    SnapshotHookBuildE2ETestCase,
    SnapshotReappearingKeyBuildE2ETestCase,
    SnapshotSelectorBuildE2ETestCase,
    SnapshotTimestampBuildE2ETestCase,
    SnapshotTimestampFailureBuildE2ETestCase,
    SnapshotWaffleShopRerunBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    assert_snapshot_scd2_invariants,
    prepare_inline_project,
    query_duckdb,
    run_sqb,
    table_exists,
)

_TIMESTAMP_REAPPEARING_KEY_FINAL_HISTORY: tuple[tuple[object, ...], ...] = (
    (1, "basic", "2024-01-15", "2024-02-03"),
    (1, "pro", "2024-02-03", None),
    (2, "basic", "2024-01-15", "2024-02-02"),
    (2, "basic", "2024-02-03", None),
    (3, "basic", "2024-01-15", "2024-02-04"),
    (3, "basic", "2024-02-05", None),
)
_TIMESTAMP_REAPPEARING_KEY_ROUND_ROWS: tuple[str, ...] = (
    "(1, 'basic', TIMESTAMP '2024-01-15', TIMESTAMP '2024-02-01'), "
    "(2, 'basic', TIMESTAMP '2024-01-15', TIMESTAMP '2024-02-01'), "
    "(3, 'basic', TIMESTAMP '2024-01-15', TIMESTAMP '2024-02-01')",
    "(1, 'basic', TIMESTAMP '2024-01-15', TIMESTAMP '2024-02-02'), "
    "(3, 'basic', TIMESTAMP '2024-01-15', TIMESTAMP '2024-02-02')",
    "(1, 'basic', TIMESTAMP '2024-01-15', TIMESTAMP '2024-02-03'), "
    "(2, 'basic', TIMESTAMP '2024-01-15', TIMESTAMP '2024-02-03'), "
    "(3, 'basic', TIMESTAMP '2024-01-15', TIMESTAMP '2024-02-03')",
    "(1, 'pro', TIMESTAMP '2024-02-03', TIMESTAMP '2024-02-04'), "
    "(2, 'basic', TIMESTAMP '2024-01-15', TIMESTAMP '2024-02-04'), "
    "(1, 'pro', TIMESTAMP '2024-02-03', TIMESTAMP '2024-02-05'), "
    "(2, 'basic', TIMESTAMP '2024-01-15', TIMESTAMP '2024-02-05'), "
    "(3, 'basic', TIMESTAMP '2024-01-15', TIMESTAMP '2024-02-05')",
)
_TIMESTAMP_REAPPEARING_KEY_TABLE_SQL: str = (
    "(customer_id INTEGER, status VARCHAR, updated_at TIMESTAMP, observed_at TIMESTAMP)"
)
_REAPPEARING_KEY_FINAL_HISTORY: tuple[tuple[object, ...], ...] = (
    (1, "active", "2024-02-01", "2024-02-04"),
    (1, "paused", "2024-02-04", None),
    (2, "active", "2024-02-01", "2024-02-02"),
    (2, "active", "2024-02-03", None),
    (3, "active", "2024-02-01", "2024-02-04"),
    (3, "active", "2024-02-05", None),
)
_REAPPEARING_KEY_OBSERVATIONS_SQL: str = (
    "SELECT 1 AS customer_id, 'active' AS status, TIMESTAMP '2024-02-01' AS observed_at "
    "UNION ALL SELECT 2, 'active', TIMESTAMP '2024-02-01' "
    "UNION ALL SELECT 3, 'active', TIMESTAMP '2024-02-01' "
    "UNION ALL SELECT 1, 'active', TIMESTAMP '2024-02-02' "
    "UNION ALL SELECT 3, 'active', TIMESTAMP '2024-02-02' "
    "UNION ALL SELECT 1, 'active', TIMESTAMP '2024-02-03' "
    "UNION ALL SELECT 2, 'active', TIMESTAMP '2024-02-03' "
    "UNION ALL SELECT 3, 'active', TIMESTAMP '2024-02-03' "
    "UNION ALL SELECT 1, 'paused', TIMESTAMP '2024-02-04' "
    "UNION ALL SELECT 2, 'active', TIMESTAMP '2024-02-04' "
    "UNION ALL SELECT 1, 'paused', TIMESTAMP '2024-02-05' "
    "UNION ALL SELECT 2, 'active', TIMESTAMP '2024-02-05' "
    "UNION ALL SELECT 3, 'active', TIMESTAMP '2024-02-05'"
)


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotSelectorBuildE2ETestCase(
            description="snapshot selector expansion respects downstream exclude",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "snapshot_selector_project"
                    adapter = "duckdb"

                    [connection]
                    database = "snapshot_selector_project.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customers
                        description: Test source raw_customers.
                        schema: main
                        table: raw_customers
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at
                    );

                    SELECT customer_id, plan, updated_at
                    FROM __source("raw_customers")
                    """
                ).strip()
                + "\n",
                "sql/models/current_customer_plans.sql": dedent(
                    """
                    MODEL (description "Test model current_customer_plans.", materialized table);

                    SELECT customer_id, plan
                    FROM __ref("customer_snapshot")
                    WHERE valid_to IS NULL
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'basic' AS plan,
                  TIMESTAMP '2024-01-01' AS updated_at
                """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                    CREATE OR REPLACE TABLE main.raw_customers AS
                    SELECT 1 AS customer_id, 'pro' AS plan,
                      TIMESTAMP '2024-01-03' AS updated_at
                    """
                ).strip(),
            ),
            excluded_downstream_command=(
                "--no-color",
                "build",
                "--select",
                "+customer_snapshot+",
                "--exclude",
                "current_customer_plans",
            ),
            downstream_command=("--no-color", "build", "--select", "+customer_snapshot+"),
            expected_exit_code=0,
            expected_snapshot_query=(
                "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_snapshot_rows_after_excluded_downstream=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-03 00:00:00"),
                (1, "pro", "2024-01-03 00:00:00", None),
            ),
            expected_downstream_query=(
                "SELECT customer_id, plan FROM main.current_customer_plans ORDER BY customer_id"
            ),
            expected_downstream_rows=((1, "pro"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_snapshot_selector_when_excluding_downstream_then_only_snapshot_builds(
    test_case: SnapshotSelectorBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snapshot_selector_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "snapshot_selector_project.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(test_case.initial_seed_sql)
    connection.close()

    initial_result: object = run_sqb(
        command=("--no-color", "build", "--select", "+customer_snapshot"),
        project_dir=project_dir,
    )
    assert initial_result.returncode == test_case.expected_exit_code, (
        initial_result.stdout + initial_result.stderr
    )

    connection = duckdb.connect(str(db_path))
    mutation_sql: str
    for mutation_sql in test_case.mutation_sql:
        connection.execute(mutation_sql)
    connection.close()

    excluded_result: object = run_sqb(
        command=test_case.excluded_downstream_command,
        project_dir=project_dir,
    )
    assert excluded_result.returncode == test_case.expected_exit_code, (
        excluded_result.stdout + excluded_result.stderr
    )
    assert table_exists(db_path=db_path, table_name="customer_snapshot") is True
    assert table_exists(db_path=db_path, table_name="current_customer_plans") is False
    snapshot_rows: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_snapshot_query)
    )
    assert snapshot_rows == test_case.expected_snapshot_rows_after_excluded_downstream
    assert_snapshot_scd2_invariants(
        db_path=db_path,
        table_name="customer_snapshot",
        key_columns=("customer_id",),
    )

    downstream_result: object = run_sqb(
        command=test_case.downstream_command,
        project_dir=project_dir,
    )
    assert downstream_result.returncode == test_case.expected_exit_code, (
        downstream_result.stdout + downstream_result.stderr
    )
    downstream_rows: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_downstream_query)
    )
    assert downstream_rows == test_case.expected_downstream_rows

    connection = duckdb.connect(str(db_path))
    connection.execute(
        dedent(
            """
            CREATE OR REPLACE TABLE main.raw_customers AS
            SELECT 1 AS customer_id, 'team' AS plan,
              TIMESTAMP '2024-01-05' AS updated_at
            """
        ).strip()
    )
    connection.close()

    downstream_only_result: object = run_sqb(
        command=("--no-color", "build", "--select", "current_customer_plans"),
        project_dir=project_dir,
    )
    assert downstream_only_result.returncode == test_case.expected_exit_code, (
        downstream_only_result.stdout + downstream_only_result.stderr
    )
    downstream_only_rows: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_downstream_query)
    )
    assert downstream_only_rows == test_case.expected_downstream_rows
    snapshot_rows_after_downstream_only: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_snapshot_query)
    )
    assert snapshot_rows_after_downstream_only == (
        test_case.expected_snapshot_rows_after_excluded_downstream
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotHookBuildE2ETestCase(
            description="snapshot pre and post hooks execute through CLI build",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "snapshot_hook_project"
                    adapter = "duckdb"

                    [connection]
                    database = "snapshot_hook_project.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customers
                        description: Test source raw_customers.
                        schema: main
                        table: raw_customers
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at,
                      pre_hooks [inline_sql("INSERT INTO main.hook_log VALUES ('pre')")],
                      post_hooks [inline_sql("INSERT INTO main.hook_log VALUES ('post')")]
                    );

                    SELECT customer_id, plan, updated_at
                    FROM __source("raw_customers")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'basic' AS plan,
                  TIMESTAMP '2024-01-01' AS updated_at;

                CREATE TABLE main.hook_log (phase VARCHAR);
                """
            ).strip(),
            command=("--no-color", "build"),
            expected_exit_code=0,
            expected_hook_query="SELECT phase FROM main.hook_log ORDER BY phase",
            expected_hook_rows=(("post",), ("pre",)),
            expected_snapshot_query=(
                "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_snapshot_rows=((1, "basic", "2024-01-01 00:00:00", None),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_snapshot_hooks_when_building_then_hooks_execute_and_history_is_valid(
    test_case: SnapshotHookBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snapshot_hook_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "snapshot_hook_project.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(test_case.initial_seed_sql)
    connection.close()

    result: object = run_sqb(command=test_case.command, project_dir=project_dir)
    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr

    hook_rows: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_hook_query)
    )
    assert hook_rows == test_case.expected_hook_rows
    snapshot_rows: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_snapshot_query)
    )
    assert snapshot_rows == test_case.expected_snapshot_rows
    assert_snapshot_scd2_invariants(
        db_path=db_path,
        table_name="customer_snapshot",
        key_columns=("customer_id",),
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotWaffleShopRerunBuildE2ETestCase(
            description="advanced snapshot edges preserve history across CLI reruns",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "snapshot_advanced_edges_project"
                    adapter = "duckdb"

                    [connection]
                    database = "snapshot_advanced_edges.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_current_customers
                        description: Test source raw_current_customers.
                        schema: main
                        table: raw_current_customers
                      - name: raw_historical_customers
                        description: Test source raw_historical_customers.
                        schema: main
                        table: raw_historical_customers
                      - name: raw_initial_customers
                        description: Test source raw_initial_customers.
                        schema: main
                        table: raw_initial_customers
                      - name: raw_audit_customers
                        description: Test source raw_audit_customers.
                        schema: main
                        table: raw_audit_customers
                    """
                ).strip()
                + "\n",
                "sql/models/current_hard_delete_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model current_hard_delete_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at,
                      invalidate_hard_deletes true
                    );

                    SELECT customer_id, plan, updated_at
                    FROM __source("raw_current_customers")
                    """
                ).strip()
                + "\n",
                "sql/models/historical_out_of_order_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model historical_out_of_order_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at,
                      observed_at observed_at,
                      historical_input snapshot
                    );

                    SELECT customer_id, plan, updated_at, observed_at
                    FROM __source("raw_historical_customers")
                    """
                ).strip()
                + "\n",
                "sql/models/initial_updated_at_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model initial_updated_at_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at,
                      initial_valid_from updated_at
                    );

                    SELECT customer_id, updated_at
                    FROM __source("raw_initial_customers")
                    """
                ).strip()
                + "\n",
                "sql/models/audited_customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model audited_customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at,
                      audits [
                        expression_is_true (
                          name "plan_is_allowed",
                          expression "plan <> 'bad'",
                          severity error,
                          run_scope final,
                        ),
                      ],
                    );

                    SELECT customer_id, plan, updated_at
                    FROM __source("raw_audit_customers")
                    """
                ).strip()
                + "\n",
                "sql/audits/generic/expression_is_true.sql": dedent(
                    """
                    AUDIT ();

                    SELECT * FROM __ref("@model") WHERE NOT (@expression)
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_current_customers AS
                SELECT 1 AS customer_id, 'basic' AS plan,
                  TIMESTAMP '2024-01-01' AS updated_at;

                CREATE TABLE main.raw_historical_customers AS
                SELECT 1 AS customer_id, 'team' AS plan,
                  TIMESTAMP '2024-01-05' AS updated_at,
                  TIMESTAMP '2024-01-05' AS observed_at
                UNION ALL
                SELECT 1 AS customer_id, 'pro' AS plan,
                  TIMESTAMP '2024-01-03' AS updated_at,
                  TIMESTAMP '2024-01-03' AS observed_at;

                CREATE TABLE main.raw_initial_customers AS
                SELECT 1 AS customer_id, TIMESTAMP '2024-01-01' AS updated_at;

                CREATE TABLE main.raw_audit_customers AS
                SELECT 1 AS customer_id, 'ok' AS plan,
                  TIMESTAMP '2024-01-01' AS updated_at;
                """
            ).strip(),
            mutation_sql_by_round=(
                (),
                (),
                (
                    dedent(
                        """
                        CREATE OR REPLACE TABLE main.raw_historical_customers AS
                        SELECT 1 AS customer_id, 'pro' AS plan,
                          TIMESTAMP '2024-01-03' AS updated_at,
                          TIMESTAMP '2024-01-03' AS observed_at
                        UNION ALL
                        SELECT 1 AS customer_id, 'team' AS plan,
                          TIMESTAMP '2024-01-05' AS updated_at,
                          TIMESTAMP '2024-01-05' AS observed_at
                        """
                    ).strip(),
                ),
                (
                    dedent(
                        """
                        CREATE OR REPLACE TABLE main.raw_current_customers AS
                        SELECT 1 AS customer_id, 'basic' AS plan,
                          TIMESTAMP '2024-01-01' AS updated_at
                        WHERE FALSE
                        """
                    ).strip(),
                ),
                (
                    dedent(
                        """
                        CREATE OR REPLACE TABLE main.raw_current_customers AS
                        SELECT 1 AS customer_id, 'pro' AS plan,
                          TIMESTAMP '2027-01-01' AS updated_at
                        """
                    ).strip(),
                ),
            ),
            command=("--no-color", "build"),
            expected_exit_code=0,
            expected_query_results_by_round=(
                (
                    ("SELECT COUNT(*) FROM main.current_hard_delete_snapshot", ((1,),)),
                    (
                        "SELECT customer_id, CAST(valid_from AS VARCHAR), valid_to "
                        "FROM main.initial_updated_at_snapshot ORDER BY customer_id",
                        ((1, "2024-01-01 00:00:00", None),),
                    ),
                    ("SELECT COUNT(*) FROM main.audited_customer_snapshot", ((1,),)),
                    (
                        "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                        "CAST(valid_to AS VARCHAR) "
                        "FROM main.historical_out_of_order_snapshot "
                        "ORDER BY customer_id, valid_from",
                        (
                            (1, "pro", "2024-01-03 00:00:00", "2024-01-05 00:00:00"),
                            (1, "team", "2024-01-05 00:00:00", None),
                        ),
                    ),
                ),
                (
                    ("SELECT COUNT(*) FROM main.current_hard_delete_snapshot", ((1,),)),
                    ("SELECT COUNT(*) FROM main.historical_out_of_order_snapshot", ((2,),)),
                    ("SELECT COUNT(*) FROM main.audited_customer_snapshot", ((1,),)),
                ),
                (
                    (
                        "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                        "CAST(valid_to AS VARCHAR) "
                        "FROM main.historical_out_of_order_snapshot "
                        "ORDER BY customer_id, valid_from",
                        (
                            (1, "pro", "2024-01-03 00:00:00", "2024-01-05 00:00:00"),
                            (1, "team", "2024-01-05 00:00:00", None),
                        ),
                    ),
                ),
                (
                    (
                        "SELECT COUNT(*), SUM(CASE WHEN valid_to IS NULL THEN 1 ELSE 0 END) "
                        "FROM main.current_hard_delete_snapshot",
                        ((1, 0),),
                    ),
                ),
                (
                    (
                        "SELECT plan, valid_to IS NULL FROM main.current_hard_delete_snapshot "
                        "ORDER BY valid_to IS NULL, plan",
                        (("basic", False), ("pro", True)),
                    ),
                ),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_advanced_snapshot_edges_when_building_then_history_remains_valid(
    test_case: SnapshotWaffleShopRerunBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snapshot_advanced_edges_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "snapshot_advanced_edges.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(test_case.initial_seed_sql)
    connection.close()

    mutation_sql_round: tuple[str, ...]
    expected_query_results: tuple[tuple[str, tuple[tuple[object, ...], ...]], ...]
    for mutation_sql_round, expected_query_results in zip(
        test_case.mutation_sql_by_round,
        test_case.expected_query_results_by_round,
        strict=True,
    ):
        connection = duckdb.connect(str(db_path))
        mutation_sql: str
        for mutation_sql in mutation_sql_round:
            connection.execute(mutation_sql)
        connection.close()

        result: object = run_sqb(command=test_case.command, project_dir=project_dir)
        assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr

        query: str
        expected_rows: tuple[tuple[object, ...], ...]
        for query, expected_rows in expected_query_results:
            rows: tuple[tuple[object, ...], ...] = tuple(
                tuple(row) for row in query_duckdb(db_path=db_path, sql=query)
            )
            assert rows == expected_rows

        for snapshot_model_name in (
            "current_hard_delete_snapshot",
            "historical_out_of_order_snapshot",
            "initial_updated_at_snapshot",
            "audited_customer_snapshot",
        ):
            assert_snapshot_scd2_invariants(
                db_path=db_path,
                table_name=snapshot_model_name,
                key_columns=("customer_id",),
            )


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotFailureConsistencyBuildE2ETestCase(
            description="pre-hook failure leaves previous snapshot history unchanged",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                name = "snapshot_failure_consistency_project"
                adapter = "duckdb"

                [connection]
                database = "snapshot_failure_consistency.duckdb"
                """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                sources:
                  - name: raw_customers
                    description: Test source raw_customers.
                    schema: main
                    table: raw_customers
                """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                MODEL (description "Test model customer_snapshot.",
                  materialized snapshot,
                  unique_key [customer_id],
                  snapshot_strategy timestamp,
                  updated_at updated_at,
                  pre_hooks [inline_sql("SELECT * FROM main.pre_hook_guard")]
                );

                SELECT customer_id, plan, updated_at
                FROM __source("raw_customers")
                """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
            CREATE TABLE main.raw_customers AS
            SELECT 1 AS customer_id, 'basic' AS plan,
              TIMESTAMP '2024-01-01' AS updated_at;

            CREATE TABLE main.pre_hook_guard (id INTEGER);
            """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                CREATE OR REPLACE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'pro' AS plan,
                  TIMESTAMP '2024-01-03' AS updated_at;
                """
                ).strip(),
                "DROP TABLE main.pre_hook_guard",
            ),
            command=("--no-color", "build"),
            expected_initial_exit_code=0,
            expected_failure_exit_code=1,
            expected_output_fragments=("customer_snapshot", "pre_hook_guard"),
            expected_snapshot_query=(
                "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_rows_after_failure=((1, "basic", "2024-01-01 00:00:00", None),),
            recovery_sql=("CREATE TABLE main.pre_hook_guard (id INTEGER)",),
            expected_rows_after_recovery=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-03 00:00:00"),
                (1, "pro", "2024-01-03 00:00:00", None),
            ),
        ),
        SnapshotFailureConsistencyBuildE2ETestCase(
            description="post-hook failure leaves updated snapshot history valid",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                name = "snapshot_failure_consistency_project"
                adapter = "duckdb"

                [connection]
                database = "snapshot_failure_consistency.duckdb"
                """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                sources:
                  - name: raw_customers
                    description: Test source raw_customers.
                    schema: main
                    table: raw_customers
                """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                MODEL (description "Test model customer_snapshot.",
                  materialized snapshot,
                  unique_key [customer_id],
                  snapshot_strategy timestamp,
                  updated_at updated_at,
                  post_hooks [inline_sql("SELECT * FROM main.post_hook_guard")]
                );

                SELECT customer_id, plan, updated_at
                FROM __source("raw_customers")
                """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
            CREATE TABLE main.raw_customers AS
            SELECT 1 AS customer_id, 'basic' AS plan,
              TIMESTAMP '2024-01-01' AS updated_at;

            CREATE TABLE main.post_hook_guard (id INTEGER);
            """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                CREATE OR REPLACE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'pro' AS plan,
                  TIMESTAMP '2024-01-03' AS updated_at;
                """
                ).strip(),
                "DROP TABLE main.post_hook_guard",
            ),
            command=("--no-color", "build"),
            expected_initial_exit_code=0,
            expected_failure_exit_code=1,
            expected_output_fragments=("customer_snapshot", "post_hook_guard"),
            expected_snapshot_query=(
                "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_rows_after_failure=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-03 00:00:00"),
                (1, "pro", "2024-01-03 00:00:00", None),
            ),
            recovery_sql=("CREATE TABLE main.post_hook_guard (id INTEGER)",),
            expected_rows_after_recovery=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-03 00:00:00"),
                (1, "pro", "2024-01-03 00:00:00", None),
            ),
        ),
        SnapshotFailureConsistencyBuildE2ETestCase(
            description="default delta-and-final audit failure leaves snapshot history unchanged",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                name = "snapshot_failure_consistency_project"
                adapter = "duckdb"

                [connection]
                database = "snapshot_failure_consistency.duckdb"

                [settings]
                default_audit_severity = "error"
                """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                sources:
                  - name: raw_customers
                    description: Test source raw_customers.
                    schema: main
                    table: raw_customers
                """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                MODEL (description "Test model customer_snapshot.",
                  materialized snapshot,
                  unique_key [customer_id],
                  snapshot_strategy timestamp,
                  updated_at updated_at,
                  audits [
                    expression_is_true (
                      name "plan_is_allowed",
                      expression "plan <> 'bad'",
                    ),
                  ],
                );

                SELECT customer_id, plan, updated_at
                FROM __source("raw_customers")
                """
                ).strip()
                + "\n",
                "sql/audits/generic/expression_is_true.sql": dedent(
                    """
                AUDIT ();

                SELECT * FROM __ref("@model") WHERE NOT (@expression)
                """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
            CREATE TABLE main.raw_customers AS
            SELECT 1 AS customer_id, 'basic' AS plan,
              TIMESTAMP '2024-01-01' AS updated_at;
            """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                CREATE OR REPLACE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'bad' AS plan,
                  TIMESTAMP '2024-01-03' AS updated_at;
                """
                ).strip(),
            ),
            command=("--no-color", "build"),
            expected_initial_exit_code=0,
            expected_failure_exit_code=1,
            expected_output_fragments=(
                "customer_snapshot",
                "delta audit for 'customer_snapshot' failed before target update",
            ),
            expected_snapshot_query=(
                "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_rows_after_failure=((1, "basic", "2024-01-01 00:00:00", None),),
            recovery_sql=(
                dedent(
                    """
                CREATE OR REPLACE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'pro' AS plan,
                  TIMESTAMP '2024-01-03' AS updated_at;
                """
                ).strip(),
            ),
            expected_rows_after_recovery=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-03 00:00:00"),
                (1, "pro", "2024-01-03 00:00:00", None),
            ),
        ),
        SnapshotFailureConsistencyBuildE2ETestCase(
            description="final audit failure leaves updated snapshot history valid",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                name = "snapshot_failure_consistency_project"
                adapter = "duckdb"

                [connection]
                database = "snapshot_failure_consistency.duckdb"
                """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                sources:
                  - name: raw_customers
                    description: Test source raw_customers.
                    schema: main
                    table: raw_customers
                """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                MODEL (description "Test model customer_snapshot.",
                  materialized snapshot,
                  unique_key [customer_id],
                  snapshot_strategy timestamp,
                  updated_at updated_at,
                  audits [
                    expression_is_true (
                      name "plan_is_not_blocked",
                      expression "plan <> 'blocked' OR valid_to IS NOT NULL",
                      severity error,
                      run_scope final,
                    ),
                  ],
                );

                SELECT customer_id, plan, updated_at
                FROM __source("raw_customers")
                """
                ).strip()
                + "\n",
                "sql/audits/generic/expression_is_true.sql": dedent(
                    """
                AUDIT ();

                SELECT * FROM __ref("@model") WHERE NOT (@expression)
                """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
            CREATE TABLE main.raw_customers AS
            SELECT 1 AS customer_id, 'basic' AS plan,
              TIMESTAMP '2024-01-01' AS updated_at;
            """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                CREATE OR REPLACE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'blocked' AS plan,
                  TIMESTAMP '2024-01-03' AS updated_at;
                """
                ).strip(),
            ),
            command=("--no-color", "build"),
            expected_initial_exit_code=0,
            expected_failure_exit_code=1,
            expected_output_fragments=(
                "customer_snapshot",
                "final audit for 'customer_snapshot' failed after target update",
            ),
            expected_snapshot_query=(
                "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_rows_after_failure=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-03 00:00:00"),
                (1, "blocked", "2024-01-03 00:00:00", None),
            ),
            recovery_sql=(
                dedent(
                    """
                CREATE OR REPLACE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'team' AS plan,
                  TIMESTAMP '2024-01-05' AS updated_at;
                """
                ).strip(),
            ),
            expected_rows_after_recovery=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-03 00:00:00"),
                (1, "blocked", "2024-01-03 00:00:00", "2024-01-05 00:00:00"),
                (1, "team", "2024-01-05 00:00:00", None),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snapshot_failure_when_building_then_history_remains_consistent(
    test_case: SnapshotFailureConsistencyBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snapshot_failure_consistency_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "snapshot_failure_consistency.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(test_case.initial_seed_sql)
    connection.close()

    initial_result: object = run_sqb(command=test_case.command, project_dir=project_dir)
    assert initial_result.returncode == test_case.expected_initial_exit_code, (
        initial_result.stdout + initial_result.stderr
    )

    connection = duckdb.connect(str(db_path))
    mutation_sql: str
    for mutation_sql in test_case.mutation_sql:
        connection.execute(mutation_sql)
    connection.close()

    failure_result: object = run_sqb(command=test_case.command, project_dir=project_dir)
    assert failure_result.returncode == test_case.expected_failure_exit_code, (
        failure_result.stdout + failure_result.stderr
    )
    output: str = failure_result.stdout + failure_result.stderr
    fragment: str
    for fragment in test_case.expected_output_fragments:
        assert fragment in output

    rows_after_failure: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_snapshot_query)
    )
    assert rows_after_failure == test_case.expected_rows_after_failure
    assert_snapshot_scd2_invariants(
        db_path=db_path,
        table_name="customer_snapshot",
        key_columns=("customer_id",),
    )

    connection = duckdb.connect(str(db_path))
    recovery_sql: str
    for recovery_sql in test_case.recovery_sql:
        connection.execute(recovery_sql)
    connection.close()

    recovery_result: object = run_sqb(command=test_case.command, project_dir=project_dir)
    assert recovery_result.returncode == test_case.expected_initial_exit_code, (
        recovery_result.stdout + recovery_result.stderr
    )
    rows_after_recovery: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_snapshot_query)
    )
    assert rows_after_recovery == test_case.expected_rows_after_recovery
    assert_snapshot_scd2_invariants(
        db_path=db_path,
        table_name="customer_snapshot",
        key_columns=("customer_id",),
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotWaffleShopRerunBuildE2ETestCase(
            description="shallow waffle shop snapshots track multiple source changes across reruns",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                name = "snapshot_waffle_shop"
                adapter = "duckdb"

                [connection]
                database = "snapshot_waffle_shop.duckdb"
                """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                sources:
                  - name: raw_current_timestamp_customers
                    description: Test source raw_current_timestamp_customers.
                    schema: main
                    table: raw_current_timestamp_customers
                  - name: raw_current_check_customers
                    description: Test source raw_current_check_customers.
                    schema: main
                    table: raw_current_check_customers
                  - name: raw_historical_snapshot_customers
                    description: Test source raw_historical_snapshot_customers.
                    schema: main
                    table: raw_historical_snapshot_customers
                  - name: raw_historical_change_customers
                    description: Test source raw_historical_change_customers.
                    schema: main
                    table: raw_historical_change_customers
                """
                ).strip()
                + "\n",
                "sql/models/customer_plan_timestamp_snapshot.sql": dedent(
                    """
                MODEL (description "Test model customer_plan_timestamp_snapshot.",
                  materialized snapshot,
                  unique_key [customer_id],
                  snapshot_strategy timestamp,
                  updated_at updated_at
                );

                SELECT customer_id, plan, updated_at
                FROM __source("raw_current_timestamp_customers")
                """
                ).strip()
                + "\n",
                "sql/models/customer_status_check_snapshot.sql": dedent(
                    """
                MODEL (description "Test model customer_status_check_snapshot.",
                  materialized snapshot,
                  unique_key [customer_id],
                  snapshot_strategy check,
                  check_columns [status]
                );

                SELECT customer_id, plan, status
                FROM __source("raw_current_check_customers")
                """
                ).strip()
                + "\n",
                "sql/models/customer_plan_historical_snapshot.sql": dedent(
                    """
                MODEL (description "Test model customer_plan_historical_snapshot.",
                  materialized snapshot,
                  unique_key [customer_id],
                  snapshot_strategy timestamp,
                  updated_at updated_at,
                  observed_at observed_at,
                  historical_input snapshot
                );

                SELECT customer_id, plan, updated_at, observed_at
                FROM __source("raw_historical_snapshot_customers")
                """
                ).strip()
                + "\n",
                "sql/models/customer_plan_change_records_snapshot.sql": dedent(
                    """
                MODEL (description "Test model customer_plan_change_records_snapshot.",
                  materialized snapshot,
                  unique_key [customer_id],
                  snapshot_strategy timestamp,
                  updated_at updated_at,
                  observed_at observed_at,
                  historical_input changes
                );

                SELECT customer_id, plan, updated_at, observed_at
                FROM __source("raw_historical_change_customers")
                """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
            CREATE TABLE main.raw_current_timestamp_customers AS
            SELECT 1 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-01-01' AS updated_at;

            CREATE TABLE main.raw_current_check_customers AS
            SELECT 1 AS customer_id, 'basic' AS plan, 'active' AS status;

            CREATE TABLE main.raw_historical_snapshot_customers AS
            SELECT 1 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-01-01' AS updated_at,
              TIMESTAMP '2024-01-02' AS observed_at;

            CREATE TABLE main.raw_historical_change_customers AS
            SELECT 1 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-01-01' AS updated_at,
              TIMESTAMP '2024-01-10' AS observed_at;
            """
            ).strip(),
            mutation_sql_by_round=(
                (),
                (
                    dedent(
                        """
                    CREATE OR REPLACE TABLE main.raw_current_timestamp_customers AS
                    SELECT 1 AS customer_id, 'pro' AS plan, TIMESTAMP '2024-01-03' AS updated_at
                    UNION ALL
                    SELECT 2 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-01-02' AS updated_at
                    """
                    ).strip(),
                    dedent(
                        """
                    CREATE OR REPLACE TABLE main.raw_current_check_customers AS
                    SELECT 1 AS customer_id, 'pro' AS plan, 'paused' AS status
                    UNION ALL
                    SELECT 2 AS customer_id, 'basic' AS plan, 'active' AS status
                    """
                    ).strip(),
                    dedent(
                        """
                    CREATE OR REPLACE TABLE main.raw_historical_snapshot_customers AS
                    SELECT 1 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-01-01' AS updated_at,
                      TIMESTAMP '2024-01-02' AS observed_at
                    UNION ALL
                    SELECT 1 AS customer_id, 'pro' AS plan, TIMESTAMP '2024-01-03' AS updated_at,
                      TIMESTAMP '2024-01-04' AS observed_at
                    UNION ALL
                    SELECT 2 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-01-02' AS updated_at,
                      TIMESTAMP '2024-01-04' AS observed_at
                    """
                    ).strip(),
                    dedent(
                        """
                    CREATE OR REPLACE TABLE main.raw_historical_change_customers AS
                    SELECT 1 AS customer_id, 'pro' AS plan, TIMESTAMP '2024-01-03' AS updated_at,
                      TIMESTAMP '2024-01-10' AS observed_at
                    """
                    ).strip(),
                ),
                (
                    dedent(
                        """
                    CREATE OR REPLACE TABLE main.raw_current_timestamp_customers AS
                    SELECT 1 AS customer_id, 'team' AS plan, TIMESTAMP '2024-01-05' AS updated_at
                    UNION ALL
                    SELECT 2 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-01-02' AS updated_at
                    """
                    ).strip(),
                    dedent(
                        """
                    CREATE OR REPLACE TABLE main.raw_current_check_customers AS
                    SELECT 1 AS customer_id, 'team' AS plan, 'active' AS status
                    UNION ALL
                    SELECT 2 AS customer_id, 'basic' AS plan, 'active' AS status
                    """
                    ).strip(),
                    dedent(
                        """
                    CREATE OR REPLACE TABLE main.raw_historical_snapshot_customers AS
                    SELECT 1 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-01-01' AS updated_at,
                      TIMESTAMP '2024-01-02' AS observed_at
                    UNION ALL
                    SELECT 1 AS customer_id, 'pro' AS plan, TIMESTAMP '2024-01-03' AS updated_at,
                      TIMESTAMP '2024-01-04' AS observed_at
                    UNION ALL
                    SELECT 1 AS customer_id, 'team' AS plan, TIMESTAMP '2024-01-05' AS updated_at,
                      TIMESTAMP '2024-01-06' AS observed_at
                    UNION ALL
                    SELECT 2 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-01-02' AS updated_at,
                      TIMESTAMP '2024-01-04' AS observed_at
                    """
                    ).strip(),
                    dedent(
                        """
                    CREATE OR REPLACE TABLE main.raw_historical_change_customers AS
                    SELECT 1 AS customer_id, 'team' AS plan, TIMESTAMP '2024-01-05' AS updated_at,
                      TIMESTAMP '2024-01-11' AS observed_at
                    """
                    ).strip(),
                ),
                (
                    dedent(
                        """
                    CREATE OR REPLACE TABLE main.raw_current_timestamp_customers AS
                    SELECT 1 AS customer_id, 'team' AS plan, TIMESTAMP '2024-01-05' AS updated_at
                    UNION ALL
                    SELECT 2 AS customer_id, 'premium' AS plan, TIMESTAMP '2024-01-07' AS updated_at
                    """
                    ).strip(),
                    dedent(
                        """
                    CREATE OR REPLACE TABLE main.raw_current_check_customers AS
                    SELECT 1 AS customer_id, 'team' AS plan, 'active' AS status
                    UNION ALL
                    SELECT 2 AS customer_id, 'premium' AS plan, 'paused' AS status
                    """
                    ).strip(),
                    dedent(
                        """
                    CREATE OR REPLACE TABLE main.raw_historical_snapshot_customers AS
                    SELECT 1 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-01-01' AS updated_at,
                      TIMESTAMP '2024-01-02' AS observed_at
                    UNION ALL
                    SELECT 1 AS customer_id, 'pro' AS plan, TIMESTAMP '2024-01-03' AS updated_at,
                      TIMESTAMP '2024-01-04' AS observed_at
                    UNION ALL
                    SELECT 1 AS customer_id, 'team' AS plan, TIMESTAMP '2024-01-05' AS updated_at,
                      TIMESTAMP '2024-01-06' AS observed_at
                    UNION ALL
                    SELECT 2 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-01-02' AS updated_at,
                      TIMESTAMP '2024-01-04' AS observed_at
                    UNION ALL
                    SELECT 2 AS customer_id, 'premium' AS plan,
                      TIMESTAMP '2024-01-07' AS updated_at,
                      TIMESTAMP '2024-01-08' AS observed_at
                    """
                    ).strip(),
                    dedent(
                        """
                    CREATE OR REPLACE TABLE main.raw_historical_change_customers AS
                    SELECT 2 AS customer_id, 'premium' AS plan,
                      TIMESTAMP '2024-01-07' AS updated_at,
                      TIMESTAMP '2024-01-12' AS observed_at
                    """
                    ).strip(),
                ),
            ),
            command=("--no-color", "build"),
            expected_exit_code=0,
            expected_query_results_by_round=(
                (
                    (
                        "SELECT COUNT(*) FROM main.customer_plan_timestamp_snapshot",
                        ((1,),),
                    ),
                    ("SELECT COUNT(*) FROM main.customer_status_check_snapshot", ((1,),)),
                    ("SELECT COUNT(*) FROM main.customer_plan_historical_snapshot", ((1,),)),
                    ("SELECT COUNT(*) FROM main.customer_plan_change_records_snapshot", ((1,),)),
                ),
                (
                    (
                        "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                        "CAST(valid_to AS VARCHAR) FROM main.customer_plan_timestamp_snapshot "
                        "ORDER BY customer_id, valid_from",
                        (
                            (1, "basic", "2024-01-01 00:00:00", "2024-01-03 00:00:00"),
                            (1, "pro", "2024-01-03 00:00:00", None),
                            (2, "basic", "2024-01-02 00:00:00", None),
                        ),
                    ),
                    (
                        "SELECT COUNT(*) FROM main.customer_status_check_snapshot",
                        ((3,),),
                    ),
                    ("SELECT COUNT(*) FROM main.customer_plan_historical_snapshot", ((3,),)),
                    ("SELECT COUNT(*) FROM main.customer_plan_change_records_snapshot", ((2,),)),
                ),
                (
                    (
                        "SELECT customer_id, plan, valid_to IS NULL "
                        "FROM main.customer_status_check_snapshot "
                        "ORDER BY customer_id, valid_to IS NULL, plan",
                        (
                            (1, "basic", False),
                            (1, "pro", False),
                            (1, "team", True),
                            (2, "basic", True),
                        ),
                    ),
                    ("SELECT COUNT(*) FROM main.customer_plan_timestamp_snapshot", ((4,),)),
                    ("SELECT COUNT(*) FROM main.customer_plan_historical_snapshot", ((4,),)),
                    ("SELECT COUNT(*) FROM main.customer_plan_change_records_snapshot", ((3,),)),
                ),
                (
                    (
                        "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                        "CAST(valid_to AS VARCHAR) FROM main.customer_plan_historical_snapshot "
                        "ORDER BY customer_id, valid_from",
                        (
                            (1, "basic", "2024-01-01 00:00:00", "2024-01-03 00:00:00"),
                            (1, "pro", "2024-01-03 00:00:00", "2024-01-05 00:00:00"),
                            (1, "team", "2024-01-05 00:00:00", None),
                            (2, "basic", "2024-01-02 00:00:00", "2024-01-07 00:00:00"),
                            (2, "premium", "2024-01-07 00:00:00", None),
                        ),
                    ),
                    (
                        "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                        "CAST(valid_to AS VARCHAR) FROM main.customer_plan_change_records_snapshot "
                        "ORDER BY customer_id, valid_from",
                        (
                            (1, "basic", "2024-01-01 00:00:00", "2024-01-03 00:00:00"),
                            (1, "pro", "2024-01-03 00:00:00", "2024-01-05 00:00:00"),
                            (1, "team", "2024-01-05 00:00:00", None),
                            (2, "premium", "2024-01-07 00:00:00", None),
                        ),
                    ),
                    ("SELECT COUNT(*) FROM main.customer_plan_timestamp_snapshot", ((5,),)),
                    ("SELECT COUNT(*) FROM main.customer_status_check_snapshot", ((5,),)),
                    (
                        "SELECT COUNT(*) FROM ("
                        "SELECT customer_id FROM main.customer_plan_timestamp_snapshot "
                        "WHERE valid_to IS NULL GROUP BY customer_id HAVING COUNT(*) > 1"
                        ")",
                        ((0,),),
                    ),
                    (
                        "SELECT COUNT(*) FROM ("
                        "SELECT customer_id FROM main.customer_plan_historical_snapshot "
                        "WHERE valid_to IS NULL GROUP BY customer_id HAVING COUNT(*) > 1"
                        ")",
                        ((0,),),
                    ),
                    (
                        "SELECT COUNT(*) FROM ("
                        "SELECT customer_id FROM main.customer_plan_change_records_snapshot "
                        "WHERE valid_to IS NULL GROUP BY customer_id HAVING COUNT(*) > 1"
                        ")",
                        ((0,),),
                    ),
                ),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_shallow_waffle_shop_snapshots_when_sources_change_then_cli_reruns_track_history(
    test_case: SnapshotWaffleShopRerunBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snapshot_waffle_shop",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "snapshot_waffle_shop.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(test_case.initial_seed_sql)
    connection.close()

    mutation_sql_round: tuple[str, ...]
    expected_query_results: tuple[tuple[str, tuple[tuple[object, ...], ...]], ...]
    for mutation_sql_round, expected_query_results in zip(
        test_case.mutation_sql_by_round,
        test_case.expected_query_results_by_round,
        strict=True,
    ):
        connection = duckdb.connect(str(db_path))
        mutation_sql: str
        for mutation_sql in mutation_sql_round:
            connection.execute(mutation_sql)
        connection.close()

        result: object = run_sqb(command=test_case.command, project_dir=project_dir)
        assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr

        query: str
        expected_rows: tuple[tuple[object, ...], ...]
        for query, expected_rows in expected_query_results:
            rows: tuple[tuple[object, ...], ...] = tuple(
                tuple(row) for row in query_duckdb(db_path=db_path, sql=query)
            )
            assert rows == expected_rows
        assert_snapshot_scd2_invariants(
            db_path=db_path,
            table_name="customer_plan_timestamp_snapshot",
            key_columns=("customer_id",),
        )
        assert_snapshot_scd2_invariants(
            db_path=db_path,
            table_name="customer_status_check_snapshot",
            key_columns=("customer_id",),
        )
        assert_snapshot_scd2_invariants(
            db_path=db_path,
            table_name="customer_plan_historical_snapshot",
            key_columns=("customer_id",),
        )
        assert_snapshot_scd2_invariants(
            db_path=db_path,
            table_name="customer_plan_change_records_snapshot",
            key_columns=("customer_id",),
        )
        snapshot_model_name: str
        for snapshot_model_name in (
            "customer_plan_timestamp_snapshot",
            "customer_status_check_snapshot",
            "customer_plan_historical_snapshot",
            "customer_plan_change_records_snapshot",
        ):
            assert (
                table_exists(
                    db_path=db_path,
                    table_name=f"{snapshot_model_name}__snapshot_delta",
                )
                is False
            )
        fingerprint_rows: tuple[str, ...] = tuple(
            str(row[0])
            for row in query_duckdb(
                db_path=db_path,
                sql=(
                    "SELECT DISTINCT node_name FROM main._sqlbuild_fingerprints "
                    "WHERE node_name LIKE 'customer_%snapshot' ORDER BY node_name"
                ),
            )
        )
        assert fingerprint_rows == (
            "customer_plan_change_records_snapshot",
            "customer_plan_historical_snapshot",
            "customer_plan_timestamp_snapshot",
            "customer_status_check_snapshot",
        )

    snapshot_artifact_name: str
    for snapshot_artifact_name in (
        "customer_plan_timestamp_snapshot",
        "customer_status_check_snapshot",
        "customer_plan_historical_snapshot",
        "customer_plan_change_records_snapshot",
    ):
        compiled_path: Path = (
            project_dir / "target" / "compiled" / "sql" / "models" / (f"{snapshot_artifact_name}.sql")
        )
        run_path: Path = (
            project_dir / "target" / "run" / "sql" / "models" / (f"{snapshot_artifact_name}.sql")
        )
        assert compiled_path.exists(), f"expected compiled snapshot artifact: {compiled_path}"
        assert run_path.exists(), f"expected run snapshot artifact: {run_path}"
        assert "SELECT" in compiled_path.read_text(encoding="utf-8")
        assert "SELECT" in run_path.read_text(encoding="utf-8")

    plan_result: object = run_sqb(command=("plan", "--json"), project_dir=project_dir)
    assert plan_result.returncode == test_case.expected_exit_code, (
        plan_result.stdout + plan_result.stderr
    )
    plan_payload: dict[str, object] = json.loads(plan_result.stdout)
    model_entries: list[dict[str, object]] = plan_payload["models"]
    model_entries_by_name: dict[str, dict[str, object]] = {
        str(entry["name"]): entry for entry in model_entries
    }
    assert len(model_entries_by_name) == len(model_entries)
    snapshot_model_name: str
    for snapshot_model_name in (
        "customer_plan_timestamp_snapshot",
        "customer_status_check_snapshot",
        "customer_plan_historical_snapshot",
        "customer_plan_change_records_snapshot",
    ):
        assert str(model_entries_by_name[snapshot_model_name]["reason"]) != "query_changed"


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotWaffleShopRerunBuildE2ETestCase(
            description="shallow waffle shop snapshot edge variants track reruns",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "snapshot_waffle_shop_edges"
                    adapter = "duckdb"

                    [connection]
                    database = "snapshot_waffle_shop_edges.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customer_regions
                        description: Test source raw_customer_regions.
                        schema: main
                        table: raw_customer_regions
                      - name: raw_customer_deletes
                        description: Test source raw_customer_deletes.
                        schema: main
                        table: raw_customer_deletes
                      - name: raw_customer_status_history
                        description: Test source raw_customer_status_history.
                        schema: main
                        table: raw_customer_status_history
                      - name: raw_customer_status_hard_delete_history
                        description: Test source raw_customer_status_hard_delete_history.
                        schema: main
                        table: raw_customer_status_hard_delete_history
                    """
                ).strip()
                + "\n",
                "sql/models/customer_region_custom_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_region_custom_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id, region],
                      snapshot_strategy timestamp,
                      updated_at updated_at,
                      valid_from_column effective_from,
                      valid_to_column effective_to
                    );

                    SELECT customer_id, region, plan, updated_at
                    FROM __source("raw_customer_regions")
                    """
                ).strip()
                + "\n",
                "sql/models/customer_hard_delete_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_hard_delete_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at,
                      invalidate_hard_deletes true
                    );

                    SELECT customer_id, plan, updated_at
                    FROM __source("raw_customer_deletes")
                    """
                ).strip()
                + "\n",
                "sql/models/customer_status_historical_check_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_status_historical_check_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy check,
                      check_columns [status],
                      observed_at observed_at
                    );

                    SELECT customer_id, plan, status, observed_at
                    FROM __source("raw_customer_status_history")
                    """
                ).strip()
                + "\n",
                "sql/models/customer_status_historical_check_hard_delete_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_status_historical_check_hard_delete_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy check,
                      check_columns [status],
                      observed_at observed_at,
                      invalidate_hard_deletes true
                    );

                    SELECT customer_id, plan, status, observed_at
                    FROM __source("raw_customer_status_hard_delete_history")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customer_regions AS
                SELECT 1 AS customer_id, 'us' AS region, 'basic' AS plan,
                  TIMESTAMP '2024-01-01' AS updated_at
                UNION ALL
                SELECT 1 AS customer_id, 'eu' AS region, 'basic' AS plan,
                  TIMESTAMP '2024-01-01' AS updated_at;

                CREATE TABLE main.raw_customer_deletes AS
                SELECT 1 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-01-01' AS updated_at
                UNION ALL
                SELECT 2 AS customer_id, 'pro' AS plan, TIMESTAMP '2024-01-02' AS updated_at;

                CREATE TABLE main.raw_customer_status_history AS
                SELECT 1 AS customer_id, 'basic' AS plan, 'active' AS status,
                  TIMESTAMP '2024-01-01' AS observed_at
                UNION ALL
                SELECT 2 AS customer_id, 'pro' AS plan, 'active' AS status,
                  TIMESTAMP '2024-01-01' AS observed_at;

                CREATE TABLE main.raw_customer_status_hard_delete_history AS
                SELECT 3 AS customer_id, 'basic' AS plan, 'active' AS status,
                  TIMESTAMP '2024-01-01' AS observed_at
                UNION ALL
                SELECT 4 AS customer_id, 'pro' AS plan, 'active' AS status,
                  TIMESTAMP '2024-01-01' AS observed_at;
                """
            ).strip(),
            mutation_sql_by_round=(
                (),
                (
                    dedent(
                        """
                        CREATE OR REPLACE TABLE main.raw_customer_regions AS
                        SELECT 1 AS customer_id, 'us' AS region, 'team' AS plan,
                          TIMESTAMP '2024-01-03' AS updated_at
                        UNION ALL
                        SELECT 1 AS customer_id, 'eu' AS region, 'basic' AS plan,
                          TIMESTAMP '2024-01-01' AS updated_at
                        """
                    ).strip(),
                    dedent(
                        """
                        CREATE OR REPLACE TABLE main.raw_customer_deletes AS
                        SELECT 1 AS customer_id, 'basic' AS plan,
                          TIMESTAMP '2024-01-01' AS updated_at
                        """
                    ).strip(),
                    dedent(
                        """
                        CREATE OR REPLACE TABLE main.raw_customer_status_history AS
                        SELECT 1 AS customer_id, 'basic' AS plan, 'active' AS status,
                          TIMESTAMP '2024-01-01' AS observed_at
                        UNION ALL
                        SELECT 1 AS customer_id, 'team' AS plan, 'paused' AS status,
                          TIMESTAMP '2024-01-03' AS observed_at
                        UNION ALL
                        SELECT 2 AS customer_id, 'pro' AS plan, 'active' AS status,
                          TIMESTAMP '2024-01-01' AS observed_at
                        """
                    ).strip(),
                    dedent(
                        """
                        CREATE OR REPLACE TABLE main.raw_customer_status_hard_delete_history AS
                        SELECT 3 AS customer_id, 'basic' AS plan, 'active' AS status,
                          TIMESTAMP '2024-01-01' AS observed_at
                        UNION ALL
                        SELECT 3 AS customer_id, 'team' AS plan, 'paused' AS status,
                          TIMESTAMP '2024-01-03' AS observed_at
                        UNION ALL
                        SELECT 4 AS customer_id, 'pro' AS plan, 'active' AS status,
                          TIMESTAMP '2024-01-01' AS observed_at
                        UNION ALL
                        SELECT 4 AS customer_id, 'pro' AS plan, 'active' AS status,
                          TIMESTAMP '2024-01-03' AS observed_at
                        """
                    ).strip(),
                ),
                (
                    dedent(
                        """
                        CREATE OR REPLACE TABLE main.raw_customer_regions AS
                        SELECT 1 AS customer_id, 'us' AS region, 'team' AS plan,
                          TIMESTAMP '2024-01-03' AS updated_at
                        UNION ALL
                        SELECT 1 AS customer_id, 'eu' AS region, 'premium' AS plan,
                          TIMESTAMP '2024-01-05' AS updated_at
                        """
                    ).strip(),
                    dedent(
                        """
                        CREATE OR REPLACE TABLE main.raw_customer_deletes AS
                        SELECT 1 AS customer_id, 'enterprise' AS plan,
                          TIMESTAMP '2024-01-06' AS updated_at
                        """
                    ).strip(),
                    dedent(
                        """
                        CREATE OR REPLACE TABLE main.raw_customer_status_history AS
                        SELECT 1 AS customer_id, 'basic' AS plan, 'active' AS status,
                          TIMESTAMP '2024-01-01' AS observed_at
                        UNION ALL
                        SELECT 1 AS customer_id, 'team' AS plan, 'paused' AS status,
                          TIMESTAMP '2024-01-03' AS observed_at
                        UNION ALL
                        SELECT 2 AS customer_id, 'pro' AS plan, 'active' AS status,
                          TIMESTAMP '2024-01-01' AS observed_at
                        UNION ALL
                        SELECT 2 AS customer_id, 'pro' AS plan, 'deleted' AS status,
                          TIMESTAMP '2024-01-04' AS observed_at
                        """
                    ).strip(),
                    dedent(
                        """
                        CREATE OR REPLACE TABLE main.raw_customer_status_hard_delete_history AS
                        SELECT 3 AS customer_id, 'basic' AS plan, 'active' AS status,
                          TIMESTAMP '2024-01-01' AS observed_at
                        UNION ALL
                        SELECT 3 AS customer_id, 'team' AS plan, 'paused' AS status,
                          TIMESTAMP '2024-01-03' AS observed_at
                        UNION ALL
                        SELECT 3 AS customer_id, 'team' AS plan, 'paused' AS status,
                          TIMESTAMP '2024-01-04' AS observed_at
                        UNION ALL
                        SELECT 4 AS customer_id, 'pro' AS plan, 'active' AS status,
                          TIMESTAMP '2024-01-01' AS observed_at
                        UNION ALL
                        SELECT 4 AS customer_id, 'pro' AS plan, 'active' AS status,
                          TIMESTAMP '2024-01-03' AS observed_at
                        """
                    ).strip(),
                ),
            ),
            command=("--no-color", "build"),
            expected_exit_code=0,
            expected_query_results_by_round=(
                (
                    ("SELECT COUNT(*) FROM main.customer_region_custom_snapshot", ((2,),)),
                    ("SELECT COUNT(*) FROM main.customer_hard_delete_snapshot", ((2,),)),
                    (
                        "SELECT COUNT(*) FROM main.customer_status_historical_check_snapshot",
                        ((2,),),
                    ),
                    (
                        "SELECT COUNT(*) "
                        "FROM main.customer_status_historical_check_hard_delete_snapshot",
                        ((2,),),
                    ),
                ),
                (
                    (
                        "SELECT customer_id, region, plan, CAST(effective_from AS VARCHAR), "
                        "CAST(effective_to AS VARCHAR) "
                        "FROM main.customer_region_custom_snapshot "
                        "ORDER BY customer_id, region DESC, effective_from",
                        (
                            (1, "us", "basic", "2024-01-01 00:00:00", "2024-01-03 00:00:00"),
                            (1, "us", "team", "2024-01-03 00:00:00", None),
                            (1, "eu", "basic", "2024-01-01 00:00:00", None),
                        ),
                    ),
                    (
                        "SELECT customer_id, plan, valid_to IS NULL "
                        "FROM main.customer_hard_delete_snapshot ORDER BY customer_id",
                        ((1, "basic", True), (2, "pro", False)),
                    ),
                    (
                        "SELECT COUNT(*) FROM main.customer_status_historical_check_snapshot",
                        ((3,),),
                    ),
                    (
                        "SELECT COUNT(*) "
                        "FROM main.customer_status_historical_check_hard_delete_snapshot",
                        ((3,),),
                    ),
                ),
                (
                    (
                        "SELECT customer_id, plan, valid_to IS NULL "
                        "FROM main.customer_hard_delete_snapshot "
                        "ORDER BY customer_id, valid_to IS NULL, plan",
                        (
                            (1, "basic", False),
                            (1, "enterprise", True),
                            (2, "pro", False),
                        ),
                    ),
                    (
                        "SELECT customer_id, status, CAST(valid_from AS VARCHAR), "
                        "CAST(valid_to AS VARCHAR) "
                        "FROM main.customer_status_historical_check_snapshot "
                        "ORDER BY customer_id, valid_from",
                        (
                            (1, "active", "2024-01-01 00:00:00", "2024-01-03 00:00:00"),
                            (1, "paused", "2024-01-03 00:00:00", None),
                            (2, "active", "2024-01-01 00:00:00", "2024-01-04 00:00:00"),
                            (2, "deleted", "2024-01-04 00:00:00", None),
                        ),
                    ),
                    (
                        "SELECT customer_id, status, CAST(valid_from AS VARCHAR), "
                        "CAST(valid_to AS VARCHAR) "
                        "FROM main.customer_status_historical_check_hard_delete_snapshot "
                        "ORDER BY customer_id, valid_from",
                        (
                            (3, "active", "2024-01-01 00:00:00", "2024-01-03 00:00:00"),
                            (3, "paused", "2024-01-03 00:00:00", None),
                            (4, "active", "2024-01-01 00:00:00", "2024-01-04 00:00:00"),
                        ),
                    ),
                    ("SELECT COUNT(*) FROM main.customer_region_custom_snapshot", ((4,),)),
                    (
                        "SELECT COUNT(*) FROM ("
                        "SELECT customer_id, region FROM main.customer_region_custom_snapshot "
                        "WHERE effective_to IS NULL GROUP BY customer_id, region "
                        "HAVING COUNT(*) > 1"
                        ")",
                        ((0,),),
                    ),
                    (
                        "SELECT COUNT(*) FROM ("
                        "SELECT customer_id FROM main.customer_hard_delete_snapshot "
                        "WHERE valid_to IS NULL GROUP BY customer_id HAVING COUNT(*) > 1"
                        ")",
                        ((0,),),
                    ),
                    (
                        "SELECT COUNT(*) FROM ("
                        "SELECT customer_id FROM main.customer_status_historical_check_snapshot "
                        "WHERE valid_to IS NULL GROUP BY customer_id HAVING COUNT(*) > 1"
                        ")",
                        ((0,),),
                    ),
                    (
                        "SELECT COUNT(*) FROM ("
                        "SELECT customer_id FROM "
                        "main.customer_status_historical_check_hard_delete_snapshot "
                        "WHERE valid_to IS NULL GROUP BY customer_id HAVING COUNT(*) > 1"
                        ")",
                        ((0,),),
                    ),
                ),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_shallow_waffle_shop_snapshot_edges_when_sources_change_then_cli_tracks_variants(
    test_case: SnapshotWaffleShopRerunBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snapshot_waffle_shop_edges",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "snapshot_waffle_shop_edges.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(test_case.initial_seed_sql)
    connection.close()

    mutation_sql_round: tuple[str, ...]
    expected_query_results: tuple[tuple[str, tuple[tuple[object, ...], ...]], ...]
    for mutation_sql_round, expected_query_results in zip(
        test_case.mutation_sql_by_round,
        test_case.expected_query_results_by_round,
        strict=True,
    ):
        connection = duckdb.connect(str(db_path))
        mutation_sql: str
        for mutation_sql in mutation_sql_round:
            connection.execute(mutation_sql)
        connection.close()

        result: object = run_sqb(command=test_case.command, project_dir=project_dir)
        assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr

        query: str
        expected_rows: tuple[tuple[object, ...], ...]
        for query, expected_rows in expected_query_results:
            rows: tuple[tuple[object, ...], ...] = tuple(
                tuple(row) for row in query_duckdb(db_path=db_path, sql=query)
            )
            assert rows == expected_rows
        assert_snapshot_scd2_invariants(
            db_path=db_path,
            table_name="customer_region_custom_snapshot",
            key_columns=("customer_id", "region"),
            valid_from_column="effective_from",
            valid_to_column="effective_to",
        )
        assert_snapshot_scd2_invariants(
            db_path=db_path,
            table_name="customer_hard_delete_snapshot",
            key_columns=("customer_id",),
        )
        assert_snapshot_scd2_invariants(
            db_path=db_path,
            table_name="customer_status_historical_check_snapshot",
            key_columns=("customer_id",),
        )
        assert_snapshot_scd2_invariants(
            db_path=db_path,
            table_name="customer_status_historical_check_hard_delete_snapshot",
            key_columns=("customer_id",),
        )
        for snapshot_model_name in (
            "customer_region_custom_snapshot",
            "customer_hard_delete_snapshot",
            "customer_status_historical_check_snapshot",
            "customer_status_historical_check_hard_delete_snapshot",
        ):
            assert (
                table_exists(
                    db_path=db_path,
                    table_name=f"{snapshot_model_name}__snapshot_delta",
                )
                is False
            )
        fingerprint_rows: tuple[str, ...] = tuple(
            str(row[0])
            for row in query_duckdb(
                db_path=db_path,
                sql=(
                    "SELECT DISTINCT node_name FROM main._sqlbuild_fingerprints "
                    "WHERE node_name LIKE 'customer_%snapshot' ORDER BY node_name"
                ),
            )
        )
        assert fingerprint_rows == (
            "customer_hard_delete_snapshot",
            "customer_region_custom_snapshot",
            "customer_status_historical_check_hard_delete_snapshot",
            "customer_status_historical_check_snapshot",
        )


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotReappearingKeyBuildE2ETestCase(
            description=(
                "historical check snapshot reopens hard-deleted keys that reappear unchanged"
            ),
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "snapshot_reappearing_keys"
                    adapter = "duckdb"

                    [connection]
                    database = "snapshot_reappearing_keys.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_membership_observations
                        description: Test source raw_membership_observations.
                        schema: main
                        table: raw_membership_observations
                      - name: raw_membership_full_history
                        description: Test source raw_membership_full_history.
                        schema: main
                        table: raw_membership_full_history
                    """
                ).strip()
                + "\n",
                "sql/models/membership_incremental_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model membership_incremental_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy check,
                      check_columns [status],
                      observed_at observed_at,
                      invalidate_hard_deletes true
                    );

                    SELECT customer_id, status, observed_at
                    FROM __source("raw_membership_observations")
                    """
                ).strip()
                + "\n",
                "sql/models/membership_full_history_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model membership_full_history_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy check,
                      check_columns [status],
                      observed_at observed_at,
                      invalidate_hard_deletes true
                    );

                    SELECT customer_id, status, observed_at
                    FROM __source("raw_membership_full_history")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=(
                "CREATE TABLE main.raw_membership_observations AS "
                "SELECT 1 AS customer_id, 'active' AS status, TIMESTAMP '2024-02-01' AS observed_at "
                "UNION ALL SELECT 2, 'active', TIMESTAMP '2024-02-01' "
                "UNION ALL SELECT 3, 'active', TIMESTAMP '2024-02-01'; "
                f"CREATE TABLE main.raw_membership_full_history AS {_REAPPEARING_KEY_OBSERVATIONS_SQL}"
            ),
            observation_sql_by_round=(
                (),
                (
                    "INSERT INTO main.raw_membership_observations VALUES "
                    "(1, 'active', TIMESTAMP '2024-02-02'), (3, 'active', TIMESTAMP '2024-02-02')",
                ),
                (
                    "INSERT INTO main.raw_membership_observations VALUES "
                    "(1, 'active', TIMESTAMP '2024-02-03'), (2, 'active', TIMESTAMP '2024-02-03'), "
                    "(3, 'active', TIMESTAMP '2024-02-03')",
                ),
                (
                    "INSERT INTO main.raw_membership_observations VALUES "
                    "(1, 'paused', TIMESTAMP '2024-02-04'), (2, 'active', TIMESTAMP '2024-02-04'), "
                    "(1, 'paused', TIMESTAMP '2024-02-05'), (2, 'active', TIMESTAMP '2024-02-05'), "
                    "(3, 'active', TIMESTAMP '2024-02-05')",
                ),
            ),
            history_query_template=(
                "SELECT customer_id, status, strftime(valid_from, '%Y-%m-%d'), "
                "strftime(valid_to, '%Y-%m-%d') FROM main.{table_name} "
                "ORDER BY customer_id, valid_from"
            ),
            expected_incremental_rows_by_round=(
                (
                    (1, "active", "2024-02-01", None),
                    (2, "active", "2024-02-01", None),
                    (3, "active", "2024-02-01", None),
                ),
                (
                    (1, "active", "2024-02-01", None),
                    (2, "active", "2024-02-01", "2024-02-02"),
                    (3, "active", "2024-02-01", None),
                ),
                (
                    (1, "active", "2024-02-01", None),
                    (2, "active", "2024-02-01", "2024-02-02"),
                    (2, "active", "2024-02-03", None),
                    (3, "active", "2024-02-01", None),
                ),
                _REAPPEARING_KEY_FINAL_HISTORY,
            ),
            expected_full_history_rows=_REAPPEARING_KEY_FINAL_HISTORY,
        ),
        SnapshotReappearingKeyBuildE2ETestCase(
            description=(
                "historical timestamp snapshot reopens hard-deleted keys at the observation time"
            ),
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "snapshot_reappearing_keys"
                    adapter = "duckdb"

                    [connection]
                    database = "snapshot_reappearing_keys.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_membership_observations
                        description: Test source raw_membership_observations.
                        schema: main
                        table: raw_membership_observations
                      - name: raw_membership_full_history
                        description: Test source raw_membership_full_history.
                        schema: main
                        table: raw_membership_full_history
                    """
                ).strip()
                + "\n",
                "sql/models/membership_incremental_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model membership_incremental_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at,
                      observed_at observed_at,
                      historical_input snapshot,
                      invalidate_hard_deletes true
                    );

                    SELECT customer_id, status, updated_at, observed_at
                    FROM __source("raw_membership_observations")
                    """
                ).strip()
                + "\n",
                "sql/models/membership_full_history_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model membership_full_history_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at,
                      observed_at observed_at,
                      historical_input snapshot,
                      invalidate_hard_deletes true
                    );

                    SELECT customer_id, status, updated_at, observed_at
                    FROM __source("raw_membership_full_history")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=(
                f"CREATE TABLE main.raw_membership_observations {_TIMESTAMP_REAPPEARING_KEY_TABLE_SQL}; "
                "INSERT INTO main.raw_membership_observations VALUES "
                f"{_TIMESTAMP_REAPPEARING_KEY_ROUND_ROWS[0]}; "
                f"CREATE TABLE main.raw_membership_full_history {_TIMESTAMP_REAPPEARING_KEY_TABLE_SQL}; "
                "INSERT INTO main.raw_membership_full_history VALUES "
                f"{', '.join(_TIMESTAMP_REAPPEARING_KEY_ROUND_ROWS)}"
            ),
            observation_sql_by_round=(
                (),
                (
                    "INSERT INTO main.raw_membership_observations VALUES "
                    f"{_TIMESTAMP_REAPPEARING_KEY_ROUND_ROWS[1]}",
                ),
                (
                    "INSERT INTO main.raw_membership_observations VALUES "
                    f"{_TIMESTAMP_REAPPEARING_KEY_ROUND_ROWS[2]}",
                ),
                (
                    "INSERT INTO main.raw_membership_observations VALUES "
                    f"{_TIMESTAMP_REAPPEARING_KEY_ROUND_ROWS[3]}",
                ),
            ),
            history_query_template=(
                "SELECT customer_id, status, strftime(valid_from, '%Y-%m-%d'), "
                "strftime(valid_to, '%Y-%m-%d') FROM main.{table_name} "
                "ORDER BY customer_id, valid_from"
            ),
            expected_incremental_rows_by_round=(
                (
                    (1, "basic", "2024-01-15", None),
                    (2, "basic", "2024-01-15", None),
                    (3, "basic", "2024-01-15", None),
                ),
                (
                    (1, "basic", "2024-01-15", None),
                    (2, "basic", "2024-01-15", "2024-02-02"),
                    (3, "basic", "2024-01-15", None),
                ),
                (
                    (1, "basic", "2024-01-15", None),
                    (2, "basic", "2024-01-15", "2024-02-02"),
                    (2, "basic", "2024-02-03", None),
                    (3, "basic", "2024-01-15", None),
                ),
                _TIMESTAMP_REAPPEARING_KEY_FINAL_HISTORY,
            ),
            expected_full_history_rows=_TIMESTAMP_REAPPEARING_KEY_FINAL_HISTORY,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_hard_deleted_key_when_it_reappears_unchanged_then_snapshot_opens_new_version(
    test_case: SnapshotReappearingKeyBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snapshot_reappearing_keys",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "snapshot_reappearing_keys.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(test_case.initial_seed_sql)
    connection.close()

    observation_sql_round: tuple[str, ...]
    expected_incremental_rows: tuple[tuple[object, ...], ...]
    for observation_sql_round, expected_incremental_rows in zip(
        test_case.observation_sql_by_round,
        test_case.expected_incremental_rows_by_round,
        strict=True,
    ):
        connection = duckdb.connect(str(db_path))
        observation_sql: str
        for observation_sql in observation_sql_round:
            connection.execute(observation_sql)
        connection.close()

        result: object = run_sqb(command=("--no-color", "build"), project_dir=project_dir)
        assert result.returncode == 0, result.stdout + result.stderr

        incremental_rows: tuple[tuple[object, ...], ...] = tuple(
            tuple(row)
            for row in query_duckdb(
                db_path=db_path,
                sql=test_case.history_query_template.format(
                    table_name="membership_incremental_snapshot"
                ),
            )
        )
        full_history_rows: tuple[tuple[object, ...], ...] = tuple(
            tuple(row)
            for row in query_duckdb(
                db_path=db_path,
                sql=test_case.history_query_template.format(
                    table_name="membership_full_history_snapshot"
                ),
            )
        )
        assert incremental_rows == expected_incremental_rows
        assert full_history_rows == test_case.expected_full_history_rows
        table_name: str
        for table_name in ("membership_incremental_snapshot", "membership_full_history_snapshot"):
            assert_snapshot_scd2_invariants(
                db_path=db_path, table_name=table_name, key_columns=("customer_id",)
            )


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotTimestampBuildE2ETestCase(
            description="current-state timestamp snapshot tracks history across CLI builds",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "snapshot_project"
                    adapter = "duckdb"

                    [connection]
                    database = "snapshot.duckdb"

                    [defaults]
                    materialized = "table"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customers
                        description: Test source raw_customers.
                        schema: main
                        table: raw_customers
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at,
                      valid_from_column effective_from,
                      valid_to_column effective_to
                    );

                    SELECT customer_id, plan, updated_at
                    FROM __source("raw_customers")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customers (
                  customer_id INTEGER,
                  plan VARCHAR,
                  updated_at TIMESTAMP
                );

                INSERT INTO main.raw_customers VALUES
                  (1, 'basic', '2024-01-01 00:00:00');
                """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                    CREATE OR REPLACE TABLE main.raw_customers AS
                    SELECT 1 AS customer_id, 'pro' AS plan,
                      TIMESTAMP '2024-01-03 00:00:00' AS updated_at
                    UNION ALL
                    SELECT 2 AS customer_id, 'basic' AS plan,
                      TIMESTAMP '2024-01-02 00:00:00' AS updated_at
                    """
                ).strip(),
            ),
            command=("--no-color", "build"),
            expected_exit_code=0,
            expected_query=(
                "SELECT customer_id, plan, CAST(effective_from AS VARCHAR), "
                "CAST(effective_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, effective_from"
            ),
            expected_initial_rows=((1, "basic", "2024-01-01 00:00:00", None),),
            expected_changed_rows=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-03 00:00:00"),
                (1, "pro", "2024-01-03 00:00:00", None),
                (2, "basic", "2024-01-02 00:00:00", None),
            ),
        ),
        SnapshotTimestampBuildE2ETestCase(
            description="current-state timestamp snapshot invalidates hard deletes through CLI",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "snapshot_project"
                    adapter = "duckdb"

                    [connection]
                    database = "snapshot.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customers
                        description: Test source raw_customers.
                        schema: main
                        table: raw_customers
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at,
                      invalidate_hard_deletes true
                    );

                    SELECT customer_id, plan, updated_at
                    FROM __source("raw_customers")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'basic' AS plan,
                  TIMESTAMP '2024-01-01 00:00:00' AS updated_at
                UNION ALL
                SELECT 2 AS customer_id, 'pro' AS plan,
                  TIMESTAMP '2024-01-02 00:00:00' AS updated_at
                """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                    CREATE OR REPLACE TABLE main.raw_customers AS
                    SELECT 1 AS customer_id, 'basic' AS plan,
                      TIMESTAMP '2024-01-01 00:00:00' AS updated_at
                    """
                ).strip(),
            ),
            command=("--no-color", "build"),
            expected_exit_code=0,
            expected_query=(
                "SELECT customer_id, plan, valid_to IS NULL FROM main.customer_snapshot "
                "ORDER BY customer_id"
            ),
            expected_initial_rows=((1, "basic", True), (2, "pro", True)),
            expected_changed_rows=((1, "basic", True), (2, "pro", False)),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_timestamp_snapshot_project_when_rerunning_build_then_tracks_history(
    test_case: SnapshotTimestampBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snapshot_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "snapshot.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(test_case.initial_seed_sql)
    connection.close()

    first_result: object = run_sqb(command=test_case.command, project_dir=project_dir)
    assert first_result.returncode == test_case.expected_exit_code, (
        first_result.stdout + first_result.stderr
    )
    rows_after_initial: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_query)
    )

    second_result: object = run_sqb(command=test_case.command, project_dir=project_dir)
    assert second_result.returncode == test_case.expected_exit_code, (
        second_result.stdout + second_result.stderr
    )
    rows_after_unchanged: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_query)
    )

    connection = duckdb.connect(str(db_path))
    statement: str
    for statement in test_case.mutation_sql:
        connection.execute(statement)
    connection.close()

    third_result: object = run_sqb(command=test_case.command, project_dir=project_dir)
    assert third_result.returncode == test_case.expected_exit_code, (
        third_result.stdout + third_result.stderr
    )
    rows_after_changed: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_query)
    )

    assert rows_after_initial == test_case.expected_initial_rows
    assert rows_after_unchanged == test_case.expected_initial_rows
    assert rows_after_changed == test_case.expected_changed_rows


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotTimestampFailureBuildE2ETestCase(
            description="duplicate snapshot source key fails build through CLI",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "snapshot_failure_project"
                    adapter = "duckdb"

                    [connection]
                    database = "snapshot_failure.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customers
                        description: Test source raw_customers.
                        schema: main
                        table: raw_customers
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at
                    );

                    SELECT customer_id, plan, updated_at
                    FROM __source("raw_customers")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'basic' AS plan,
                  TIMESTAMP '2024-01-01 00:00:00' AS updated_at
                UNION ALL
                SELECT 1 AS customer_id, 'pro' AS plan,
                  TIMESTAMP '2024-01-02 00:00:00' AS updated_at
                """
            ).strip(),
            command=("--no-color", "build"),
            expected_exit_code=1,
            expected_output_fragments=(
                "customer_snapshot",
                "source query returned multiple rows for the same unique_key (customer_id)",
            ),
        ),
        SnapshotTimestampFailureBuildE2ETestCase(
            description="historical snapshot duplicate observed identity fails build through CLI",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "snapshot_failure_project"
                    adapter = "duckdb"

                    [connection]
                    database = "snapshot_failure.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customers
                        description: Test source raw_customers.
                        schema: main
                        table: raw_customers
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at,
                      observed_at observed_at,
                      historical_input snapshot
                    );

                    SELECT customer_id, plan, updated_at, observed_at
                    FROM __source("raw_customers")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'basic' AS plan,
                  TIMESTAMP '2024-01-01' AS updated_at,
                  TIMESTAMP '2024-01-02' AS observed_at
                UNION ALL
                SELECT 1 AS customer_id, 'pro' AS plan,
                  TIMESTAMP '2024-01-03' AS updated_at,
                  TIMESTAMP '2024-01-02' AS observed_at
                """
            ).strip(),
            command=("--no-color", "build"),
            expected_exit_code=1,
            expected_output_fragments=(
                "customer_snapshot",
                "source query returned multiple rows for the same snapshot identity "
                "(customer_id, observed_at)",
            ),
        ),
        SnapshotTimestampFailureBuildE2ETestCase(
            description="historical changes duplicate updated identity fails build through CLI",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "snapshot_failure_project"
                    adapter = "duckdb"

                    [connection]
                    database = "snapshot_failure.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customers
                        description: Test source raw_customers.
                        schema: main
                        table: raw_customers
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at,
                      observed_at observed_at,
                      historical_input changes
                    );

                    SELECT customer_id, plan, updated_at, observed_at
                    FROM __source("raw_customers")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'basic' AS plan,
                  TIMESTAMP '2024-01-01' AS updated_at,
                  TIMESTAMP '2024-01-02' AS observed_at
                UNION ALL
                SELECT 1 AS customer_id, 'basic_duplicate' AS plan,
                  TIMESTAMP '2024-01-01' AS updated_at,
                  TIMESTAMP '2024-01-03' AS observed_at
                """
            ).strip(),
            command=("--no-color", "build"),
            expected_exit_code=1,
            expected_output_fragments=(
                "customer_snapshot",
                "source query returned multiple rows for the same snapshot identity "
                "(customer_id, updated_at)",
            ),
        ),
        SnapshotTimestampFailureBuildE2ETestCase(
            description="missing timestamp snapshot updated_at column fails build through CLI",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "snapshot_failure_project"
                    adapter = "duckdb"

                    [connection]
                    database = "snapshot_failure.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customers
                        description: Test source raw_customers.
                        schema: main
                        table: raw_customers
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at
                    );

                    SELECT customer_id, plan
                    FROM __source("raw_customers")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'basic' AS plan
                """
            ).strip(),
            command=("--no-color", "build"),
            expected_exit_code=1,
            expected_output_fragments=(
                "customer_snapshot",
                "query output is missing required columns: updated_at",
            ),
        ),
        SnapshotTimestampFailureBuildE2ETestCase(
            description="missing historical observed_at column fails build through CLI",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "snapshot_failure_project"
                    adapter = "duckdb"

                    [connection]
                    database = "snapshot_failure.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customers
                        description: Test source raw_customers.
                        schema: main
                        table: raw_customers
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at,
                      observed_at observed_at,
                      historical_input snapshot
                    );

                    SELECT customer_id, plan, updated_at
                    FROM __source("raw_customers")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'basic' AS plan,
                  TIMESTAMP '2024-01-01' AS updated_at
                """
            ).strip(),
            command=("--no-color", "build"),
            expected_exit_code=1,
            expected_output_fragments=(
                "customer_snapshot",
                "query output is missing required columns: observed_at",
            ),
        ),
        SnapshotTimestampFailureBuildE2ETestCase(
            description="missing snapshot unique key column fails build through CLI",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "snapshot_failure_project"
                    adapter = "duckdb"

                    [connection]
                    database = "snapshot_failure.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customers
                        description: Test source raw_customers.
                        schema: main
                        table: raw_customers
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at
                    );

                    SELECT plan, updated_at
                    FROM __source("raw_customers")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customers AS
                SELECT 'basic' AS plan, TIMESTAMP '2024-01-01' AS updated_at
                """
            ).strip(),
            command=("--no-color", "build"),
            expected_exit_code=1,
            expected_output_fragments=(
                "error[B300]: [B300] sql/models/customer_snapshot.sql:3:15:",
                "unique_key references unknown column 'customer_id'",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_duplicate_timestamp_snapshot_source_when_building_then_cli_reports_failure(
    test_case: SnapshotTimestampFailureBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snapshot_failure_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "snapshot_failure.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(test_case.initial_seed_sql)
    connection.close()

    result: object = run_sqb(command=test_case.command, project_dir=project_dir)

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    output: str = result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_output_fragments:
        assert fragment in output


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotDmlFailureRollbackBuildE2ETestCase(
            description="snapshot dml failure rolls back target changes",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "snapshot_dml_failure_project"
                    adapter = "duckdb"

                    [connection]
                    database = "snapshot_dml_failure.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customers
                        description: Test source raw_customers.
                        schema: main
                        table: raw_customers
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at
                    );

                    SELECT customer_id, loyalty_points, updated_at
                    FROM __source("raw_customers")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 10 AS loyalty_points,
                  TIMESTAMP '2024-01-01' AS updated_at
                """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                    CREATE OR REPLACE TABLE main.raw_customers AS
                    SELECT 1 AS customer_id, 'not_an_integer' AS loyalty_points,
                      TIMESTAMP '2024-01-03' AS updated_at
                    """
                ).strip(),
            ),
            command=("--no-color", "build"),
            expected_initial_exit_code=0,
            expected_failure_exit_code=1,
            expected_error_fragments=("customer_snapshot", "dml"),
            expected_query=(
                "SELECT customer_id, loyalty_points, CAST(valid_from AS VARCHAR), "
                "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_rows_after_failure=((1, 10, "2024-01-01 00:00:00", None),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_snapshot_dml_failure_when_building_then_target_history_is_unchanged(
    test_case: SnapshotDmlFailureRollbackBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snapshot_dml_failure_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "snapshot_dml_failure.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(test_case.initial_seed_sql)
    connection.close()

    initial_result: object = run_sqb(command=test_case.command, project_dir=project_dir)
    assert initial_result.returncode == test_case.expected_initial_exit_code, (
        initial_result.stdout + initial_result.stderr
    )

    connection = duckdb.connect(str(db_path))
    mutation_sql: str
    for mutation_sql in test_case.mutation_sql:
        connection.execute(mutation_sql)
    connection.close()

    failure_result: object = run_sqb(command=test_case.command, project_dir=project_dir)
    assert failure_result.returncode == test_case.expected_failure_exit_code, (
        failure_result.stdout + failure_result.stderr
    )
    output: str = failure_result.stdout + failure_result.stderr
    fragment: str
    for fragment in test_case.expected_error_fragments:
        assert fragment in output

    rows_after_failure: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_query)
    )
    assert rows_after_failure == test_case.expected_rows_after_failure
    assert_snapshot_scd2_invariants(
        db_path=db_path,
        table_name="customer_snapshot",
        key_columns=("customer_id",),
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotCheckBuildE2ETestCase(
            description="current-state check snapshot tracks checked changes across CLI builds",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                name = "check_snapshot_project"
                adapter = "duckdb"

                [connection]
                database = "check_snapshot.duckdb"
                """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                sources:
                  - name: raw_customers
                    description: Test source raw_customers.
                    schema: main
                    table: raw_customers
                """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                MODEL (description "Test model customer_snapshot.",
                  materialized snapshot,
                  unique_key [customer_id],
                  snapshot_strategy check,
                  check_columns [status],
                  valid_from_column effective_from,
                  valid_to_column effective_to
                );

                SELECT customer_id, plan, status
                FROM __source("raw_customers")
                """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
            CREATE TABLE main.raw_customers (
              customer_id INTEGER,
              plan VARCHAR,
              status VARCHAR
            );

            INSERT INTO main.raw_customers VALUES
              (1, 'basic', 'active');
            """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                CREATE OR REPLACE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'pro' AS plan, 'paused' AS status
                UNION ALL
                SELECT 2 AS customer_id, 'basic' AS plan, 'active' AS status
                """
                ).strip(),
            ),
            command=("--no-color", "build"),
            expected_exit_code=0,
            expected_query=(
                "SELECT customer_id, plan, status, effective_to IS NULL "
                "FROM main.customer_snapshot ORDER BY customer_id, effective_to IS NULL, plan"
            ),
            expected_initial_rows=((1, "basic", "active", True),),
            expected_changed_rows=(
                (1, "basic", "active", False),
                (1, "pro", "paused", True),
                (2, "basic", "active", True),
            ),
        ),
        SnapshotCheckBuildE2ETestCase(
            description="current-state check snapshot invalidates hard deletes through CLI",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                name = "check_snapshot_project"
                adapter = "duckdb"

                [connection]
                database = "check_snapshot.duckdb"
                """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                sources:
                  - name: raw_customers
                    description: Test source raw_customers.
                    schema: main
                    table: raw_customers
                """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                MODEL (description "Test model customer_snapshot.",
                  materialized snapshot,
                  unique_key [customer_id],
                  snapshot_strategy check,
                  check_columns [status],
                  invalidate_hard_deletes true
                );

                SELECT customer_id, plan, status
                FROM __source("raw_customers")
                """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
            CREATE TABLE main.raw_customers AS
            SELECT 1 AS customer_id, 'basic' AS plan, 'active' AS status
            UNION ALL
            SELECT 2 AS customer_id, 'pro' AS plan, 'active' AS status
            """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                CREATE OR REPLACE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'basic' AS plan, 'active' AS status
                """
                ).strip(),
            ),
            command=("--no-color", "build"),
            expected_exit_code=0,
            expected_query=(
                "SELECT customer_id, plan, status, valid_to IS NULL "
                "FROM main.customer_snapshot ORDER BY customer_id"
            ),
            expected_initial_rows=((1, "basic", "active", True), (2, "pro", "active", True)),
            expected_changed_rows=((1, "basic", "active", True), (2, "pro", "active", False)),
        ),
        SnapshotCheckBuildE2ETestCase(
            description="current-state check snapshot expands wildcard check columns",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                name = "check_snapshot_project"
                adapter = "duckdb"

                [connection]
                database = "check_snapshot.duckdb"
                """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                sources:
                  - name: raw_customers
                    description: Test source raw_customers.
                    schema: main
                    table: raw_customers
                """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                MODEL (description "Test model customer_snapshot.",
                  materialized snapshot,
                  unique_key [customer_id],
                  snapshot_strategy check,
                  check_columns [*]
                );

                SELECT customer_id, plan, status
                FROM __source("raw_customers")
                """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
            CREATE TABLE main.raw_customers AS
            SELECT 1 AS customer_id, 'basic' AS plan, 'active' AS status
            """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                CREATE OR REPLACE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'pro' AS plan, 'active' AS status
                """
                ).strip(),
            ),
            command=("--no-color", "build"),
            expected_exit_code=0,
            expected_query=(
                "SELECT customer_id, plan, status, valid_to IS NULL "
                "FROM main.customer_snapshot ORDER BY customer_id, valid_from"
            ),
            expected_initial_rows=((1, "basic", "active", True),),
            expected_changed_rows=(
                (1, "basic", "active", False),
                (1, "pro", "active", True),
            ),
        ),
        SnapshotCheckBuildE2ETestCase(
            description="current-state check snapshot wildcard excludes custom validity columns",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                name = "check_snapshot_project"
                adapter = "duckdb"

                [connection]
                database = "check_snapshot.duckdb"
                """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                sources:
                  - name: raw_customers
                    description: Test source raw_customers.
                    schema: main
                    table: raw_customers
                """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                MODEL (description "Test model customer_snapshot.",
                  materialized snapshot,
                  unique_key [customer_id],
                  snapshot_strategy check,
                  check_columns [*],
                  valid_from_column effective_from,
                  valid_to_column effective_to
                );

                SELECT customer_id, plan, status
                FROM __source("raw_customers")
                """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
            CREATE TABLE main.raw_customers AS
            SELECT 1 AS customer_id, 'basic' AS plan, 'active' AS status
            """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                CREATE OR REPLACE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'basic' AS plan, 'paused' AS status
                """
                ).strip(),
            ),
            command=("--no-color", "build"),
            expected_exit_code=0,
            expected_query=(
                "SELECT customer_id, plan, status, effective_to IS NULL "
                "FROM main.customer_snapshot ORDER BY customer_id, effective_from"
            ),
            expected_initial_rows=((1, "basic", "active", True),),
            expected_changed_rows=(
                (1, "basic", "active", False),
                (1, "basic", "paused", True),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_check_snapshot_project_when_rerunning_build_then_tracks_checked_changes(
    test_case: SnapshotCheckBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="check_snapshot_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "check_snapshot.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(test_case.initial_seed_sql)
    connection.close()

    first_result: object = run_sqb(command=test_case.command, project_dir=project_dir)
    assert first_result.returncode == test_case.expected_exit_code, (
        first_result.stdout + first_result.stderr
    )
    rows_after_initial: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_query)
    )

    second_result: object = run_sqb(command=test_case.command, project_dir=project_dir)
    assert second_result.returncode == test_case.expected_exit_code, (
        second_result.stdout + second_result.stderr
    )
    rows_after_unchanged: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_query)
    )

    connection = duckdb.connect(str(db_path))
    statement: str
    for statement in test_case.mutation_sql:
        connection.execute(statement)
    connection.close()

    third_result: object = run_sqb(command=test_case.command, project_dir=project_dir)
    assert third_result.returncode == test_case.expected_exit_code, (
        third_result.stdout + third_result.stderr
    )
    rows_after_changed: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_query)
    )

    assert rows_after_initial == test_case.expected_initial_rows
    assert rows_after_unchanged == test_case.expected_initial_rows
    assert rows_after_changed == test_case.expected_changed_rows


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotHistoricalCheckBuildE2ETestCase(
            description="historical check snapshot tracks observed history through CLI",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "historical_check_snapshot_project"
                    adapter = "duckdb"

                    [connection]
                    database = "historical_check_snapshot.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customer_daily
                        description: Test source raw_customer_daily.
                        schema: main
                        table: raw_customer_daily
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy check,
                      check_columns [plan],
                      observed_at observed_at
                    );

                    SELECT customer_id, plan, observed_at
                    FROM __source("raw_customer_daily")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customer_daily AS
                SELECT 1 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-01-01' AS observed_at
                UNION ALL SELECT 1, 'pro', TIMESTAMP '2024-01-02'
                UNION ALL SELECT 1, 'pro', TIMESTAMP '2024-01-03'
                UNION ALL SELECT 1, 'team', TIMESTAMP '2024-01-04'
                """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                    CREATE OR REPLACE TABLE main.raw_customer_daily AS
                    SELECT 1 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-01-01' AS observed_at
                    UNION ALL SELECT 1, 'pro', TIMESTAMP '2024-01-02'
                    UNION ALL SELECT 1, 'pro', TIMESTAMP '2024-01-03'
                    UNION ALL SELECT 1, 'team', TIMESTAMP '2024-01-04'
                    UNION ALL SELECT 1, 'enterprise', TIMESTAMP '2024-01-05'
                    """
                ).strip(),
            ),
            command=("--no-color", "build"),
            expected_exit_code=0,
            expected_query=(
                "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_initial_rows=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-02 00:00:00"),
                (1, "pro", "2024-01-02 00:00:00", "2024-01-04 00:00:00"),
                (1, "team", "2024-01-04 00:00:00", None),
            ),
            expected_changed_rows=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-02 00:00:00"),
                (1, "pro", "2024-01-02 00:00:00", "2024-01-04 00:00:00"),
                (1, "team", "2024-01-04 00:00:00", "2024-01-05 00:00:00"),
                (1, "enterprise", "2024-01-05 00:00:00", None),
            ),
        ),
        SnapshotHistoricalCheckBuildE2ETestCase(
            description="historical check snapshot invalidates hard deletes through CLI",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "historical_check_snapshot_project"
                    adapter = "duckdb"

                    [connection]
                    database = "historical_check_snapshot.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customer_daily
                        description: Test source raw_customer_daily.
                        schema: main
                        table: raw_customer_daily
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy check,
                      check_columns [plan],
                      observed_at observed_at,
                      invalidate_hard_deletes true
                    );

                    SELECT customer_id, plan, observed_at
                    FROM __source("raw_customer_daily")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customer_daily AS
                SELECT 1 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-01-01' AS observed_at
                UNION ALL SELECT 2, 'basic', TIMESTAMP '2024-01-01'
                UNION ALL SELECT 1, 'pro', TIMESTAMP '2024-01-02'
                """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                    CREATE OR REPLACE TABLE main.raw_customer_daily AS
                    SELECT 1 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-01-01' AS observed_at
                    UNION ALL SELECT 2, 'basic', TIMESTAMP '2024-01-01'
                    UNION ALL SELECT 1, 'pro', TIMESTAMP '2024-01-02'
                    UNION ALL SELECT 2, 'team', TIMESTAMP '2024-01-03'
                    """
                ).strip(),
            ),
            command=("--no-color", "build"),
            expected_exit_code=0,
            expected_query=(
                "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_initial_rows=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-02 00:00:00"),
                (1, "pro", "2024-01-02 00:00:00", None),
                (2, "basic", "2024-01-01 00:00:00", "2024-01-02 00:00:00"),
            ),
            expected_changed_rows=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-02 00:00:00"),
                (1, "pro", "2024-01-02 00:00:00", "2024-01-03 00:00:00"),
                (2, "basic", "2024-01-01 00:00:00", "2024-01-02 00:00:00"),
                (2, "team", "2024-01-03 00:00:00", None),
            ),
        ),
        SnapshotHistoricalCheckBuildE2ETestCase(
            description="historical check snapshot expands wildcard check columns",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "historical_check_snapshot_project"
                    adapter = "duckdb"

                    [connection]
                    database = "historical_check_snapshot.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customer_daily
                        description: Test source raw_customer_daily.
                        schema: main
                        table: raw_customer_daily
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy check,
                      check_columns [*],
                      observed_at observed_at
                    );

                    SELECT customer_id, plan, status, observed_at
                    FROM __source("raw_customer_daily")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customer_daily AS
                SELECT 1 AS customer_id, 'basic' AS plan, 'active' AS status,
                  TIMESTAMP '2024-01-01' AS observed_at
                UNION ALL SELECT 1, 'pro', 'active', TIMESTAMP '2024-01-02'
                UNION ALL SELECT 1, 'pro', 'active', TIMESTAMP '2024-01-03'
                """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                    CREATE OR REPLACE TABLE main.raw_customer_daily AS
                    SELECT 1 AS customer_id, 'basic' AS plan, 'active' AS status,
                      TIMESTAMP '2024-01-01' AS observed_at
                    UNION ALL SELECT 1, 'pro', 'active', TIMESTAMP '2024-01-02'
                    UNION ALL SELECT 1, 'pro', 'active', TIMESTAMP '2024-01-03'
                    UNION ALL SELECT 1, 'pro', 'paused', TIMESTAMP '2024-01-04'
                    """
                ).strip(),
            ),
            command=("--no-color", "build"),
            expected_exit_code=0,
            expected_query=(
                "SELECT customer_id, plan, status, CAST(valid_from AS VARCHAR), "
                "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_initial_rows=(
                (1, "basic", "active", "2024-01-01 00:00:00", "2024-01-02 00:00:00"),
                (1, "pro", "active", "2024-01-02 00:00:00", None),
            ),
            expected_changed_rows=(
                (1, "basic", "active", "2024-01-01 00:00:00", "2024-01-02 00:00:00"),
                (1, "pro", "active", "2024-01-02 00:00:00", "2024-01-04 00:00:00"),
                (1, "pro", "paused", "2024-01-04 00:00:00", None),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_historical_check_snapshot_project_when_rerunning_build_then_tracks_history(
    test_case: SnapshotHistoricalCheckBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="historical_check_snapshot_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "historical_check_snapshot.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(test_case.initial_seed_sql)
    connection.close()

    first_result: object = run_sqb(command=test_case.command, project_dir=project_dir)
    assert first_result.returncode == test_case.expected_exit_code, (
        first_result.stdout + first_result.stderr
    )
    rows_after_initial: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_query)
    )

    connection = duckdb.connect(str(db_path))
    statement: str
    for statement in test_case.mutation_sql:
        connection.execute(statement)
    connection.close()

    second_result: object = run_sqb(command=test_case.command, project_dir=project_dir)
    assert second_result.returncode == test_case.expected_exit_code, (
        second_result.stdout + second_result.stderr
    )
    rows_after_changed: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_query)
    )

    assert rows_after_initial == test_case.expected_initial_rows
    assert rows_after_changed == test_case.expected_changed_rows


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotHistoricalTimestampBuildE2ETestCase(
            description="historical timestamp snapshot tracks updated history through CLI",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "historical_timestamp_snapshot_project"
                    adapter = "duckdb"

                    [connection]
                    database = "historical_timestamp_snapshot.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customer_extracts
                        description: Test source raw_customer_extracts.
                        schema: main
                        table: raw_customer_extracts
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at,
                      observed_at observed_at,
                      historical_input snapshot
                    );

                    SELECT customer_id, plan, updated_at, observed_at
                    FROM __source("raw_customer_extracts")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customer_extracts AS
                SELECT 1 AS customer_id, 'basic' AS plan,
                  TIMESTAMP '2024-01-01' AS updated_at,
                  TIMESTAMP '2024-01-02' AS observed_at
                UNION ALL SELECT 1, 'basic', TIMESTAMP '2024-01-01', TIMESTAMP '2024-01-03'
                UNION ALL SELECT 1, 'pro', TIMESTAMP '2024-01-04', TIMESTAMP '2024-01-06'
                """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                    CREATE OR REPLACE TABLE main.raw_customer_extracts AS
                    SELECT 1 AS customer_id, 'basic' AS plan,
                      TIMESTAMP '2024-01-01' AS updated_at,
                      TIMESTAMP '2024-01-02' AS observed_at
                    UNION ALL SELECT 1, 'basic', TIMESTAMP '2024-01-01', TIMESTAMP '2024-01-03'
                    UNION ALL SELECT 1, 'pro', TIMESTAMP '2024-01-04', TIMESTAMP '2024-01-06'
                    UNION ALL SELECT 1, 'team', TIMESTAMP '2024-01-07', TIMESTAMP '2024-01-08'
                    """
                ).strip(),
            ),
            command=("--no-color", "build"),
            expected_exit_code=0,
            expected_query=(
                "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_initial_rows=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-04 00:00:00"),
                (1, "pro", "2024-01-04 00:00:00", None),
            ),
            expected_changed_rows=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-04 00:00:00"),
                (1, "pro", "2024-01-04 00:00:00", "2024-01-07 00:00:00"),
                (1, "team", "2024-01-07 00:00:00", None),
            ),
        ),
        SnapshotHistoricalTimestampBuildE2ETestCase(
            description="historical timestamp snapshot invalidates hard deletes through CLI",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "historical_timestamp_snapshot_project"
                    adapter = "duckdb"

                    [connection]
                    database = "historical_timestamp_snapshot.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customer_extracts
                        description: Test source raw_customer_extracts.
                        schema: main
                        table: raw_customer_extracts
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at,
                      observed_at observed_at,
                      historical_input snapshot,
                      invalidate_hard_deletes true
                    );

                    SELECT customer_id, plan, updated_at, observed_at
                    FROM __source("raw_customer_extracts")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customer_extracts AS
                SELECT 1 AS customer_id, 'basic' AS plan,
                  TIMESTAMP '2024-01-01' AS updated_at,
                  TIMESTAMP '2024-01-02' AS observed_at
                UNION ALL SELECT 2, 'basic', TIMESTAMP '2024-01-01', TIMESTAMP '2024-01-02'
                UNION ALL SELECT 1, 'pro', TIMESTAMP '2024-01-04', TIMESTAMP '2024-01-05'
                """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                    CREATE OR REPLACE TABLE main.raw_customer_extracts AS
                    SELECT 1 AS customer_id, 'basic' AS plan,
                      TIMESTAMP '2024-01-01' AS updated_at,
                      TIMESTAMP '2024-01-02' AS observed_at
                    UNION ALL SELECT 2, 'basic', TIMESTAMP '2024-01-01', TIMESTAMP '2024-01-02'
                    UNION ALL SELECT 1, 'pro', TIMESTAMP '2024-01-04', TIMESTAMP '2024-01-05'
                    UNION ALL SELECT 2, 'team', TIMESTAMP '2024-01-07', TIMESTAMP '2024-01-08'
                    """
                ).strip(),
            ),
            command=("--no-color", "build"),
            expected_exit_code=0,
            expected_query=(
                "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_initial_rows=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-04 00:00:00"),
                (1, "pro", "2024-01-04 00:00:00", None),
                (2, "basic", "2024-01-01 00:00:00", "2024-01-05 00:00:00"),
            ),
            expected_changed_rows=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-04 00:00:00"),
                (1, "pro", "2024-01-04 00:00:00", "2024-01-08 00:00:00"),
                (2, "basic", "2024-01-01 00:00:00", "2024-01-05 00:00:00"),
                (2, "team", "2024-01-07 00:00:00", None),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_historical_timestamp_snapshot_project_when_rerunning_build_then_tracks_history(
    test_case: SnapshotHistoricalTimestampBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="historical_timestamp_snapshot_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "historical_timestamp_snapshot.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(test_case.initial_seed_sql)
    connection.close()

    first_result: object = run_sqb(command=test_case.command, project_dir=project_dir)
    assert first_result.returncode == test_case.expected_exit_code, (
        first_result.stdout + first_result.stderr
    )
    rows_after_initial: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_query)
    )

    connection = duckdb.connect(str(db_path))
    statement: str
    for statement in test_case.mutation_sql:
        connection.execute(statement)
    connection.close()

    second_result: object = run_sqb(command=test_case.command, project_dir=project_dir)
    assert second_result.returncode == test_case.expected_exit_code, (
        second_result.stdout + second_result.stderr
    )
    rows_after_changed: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_query)
    )

    assert rows_after_initial == test_case.expected_initial_rows
    assert rows_after_changed == test_case.expected_changed_rows


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotHistoricalTimestampBuildE2ETestCase(
            description="historical timestamp changes track updated history through CLI",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "historical_timestamp_changes_project"
                    adapter = "duckdb"

                    [connection]
                    database = "historical_timestamp_changes.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customer_changes
                        description: Test source raw_customer_changes.
                        schema: main
                        table: raw_customer_changes
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at,
                      observed_at observed_at,
                      historical_input changes
                    );

                    SELECT customer_id, plan, updated_at, observed_at
                    FROM __source("raw_customer_changes")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customer_changes AS
                SELECT 1 AS customer_id, 'basic' AS plan,
                  TIMESTAMP '2024-01-01' AS updated_at,
                  TIMESTAMP '2024-01-10' AS observed_at
                UNION ALL SELECT 1, 'pro', TIMESTAMP '2024-01-04', TIMESTAMP '2024-01-10'
                """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                    CREATE OR REPLACE TABLE main.raw_customer_changes AS
                    SELECT 1 AS customer_id, 'basic' AS plan,
                      TIMESTAMP '2024-01-01' AS updated_at,
                      TIMESTAMP '2024-01-10' AS observed_at
                    UNION ALL SELECT 1, 'pro', TIMESTAMP '2024-01-04', TIMESTAMP '2024-01-10'
                    UNION ALL SELECT 1, 'team', TIMESTAMP '2024-01-07', TIMESTAMP '2024-01-11'
                    """
                ).strip(),
            ),
            command=("--no-color", "build"),
            expected_exit_code=0,
            expected_query=(
                "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_initial_rows=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-04 00:00:00"),
                (1, "pro", "2024-01-04 00:00:00", None),
            ),
            expected_changed_rows=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-04 00:00:00"),
                (1, "pro", "2024-01-04 00:00:00", "2024-01-07 00:00:00"),
                (1, "team", "2024-01-07 00:00:00", None),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_historical_timestamp_changes_project_when_rerunning_build_then_tracks_history(
    test_case: SnapshotHistoricalTimestampBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="historical_timestamp_changes_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "historical_timestamp_changes.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(test_case.initial_seed_sql)
    connection.close()

    first_result: object = run_sqb(command=test_case.command, project_dir=project_dir)
    assert first_result.returncode == test_case.expected_exit_code, (
        first_result.stdout + first_result.stderr
    )
    rows_after_initial: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_query)
    )

    connection = duckdb.connect(str(db_path))
    statement: str
    for statement in test_case.mutation_sql:
        connection.execute(statement)
    connection.close()

    second_result: object = run_sqb(command=test_case.command, project_dir=project_dir)
    assert second_result.returncode == test_case.expected_exit_code, (
        second_result.stdout + second_result.stderr
    )
    rows_after_changed: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_query)
    )

    assert rows_after_initial == test_case.expected_initial_rows
    assert rows_after_changed == test_case.expected_changed_rows


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotCheckFailureBuildE2ETestCase(
            description="missing check snapshot output column fails build through CLI",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "check_snapshot_failure_project"
                    adapter = "duckdb"

                    [connection]
                    database = "check_snapshot_failure.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customers
                        description: Test source raw_customers.
                        schema: main
                        table: raw_customers
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy check,
                      check_columns [status]
                    );

                    SELECT customer_id, plan
                    FROM __source("raw_customers")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'basic' AS plan, 'active' AS status
                """
            ).strip(),
            command=("--no-color", "build"),
            expected_exit_code=1,
            expected_output_fragments=(
                "customer_snapshot",
                "query output is missing required columns: status",
            ),
        ),
        SnapshotCheckFailureBuildE2ETestCase(
            description="wildcard check snapshot without data columns fails build through CLI",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "check_snapshot_failure_project"
                    adapter = "duckdb"

                    [connection]
                    database = "check_snapshot_failure.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customers
                        description: Test source raw_customers.
                        schema: main
                        table: raw_customers
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy check,
                      check_columns [*]
                    );

                    SELECT customer_id
                    FROM __source("raw_customers")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customers AS
                SELECT 1 AS customer_id
                """
            ).strip(),
            command=("--no-color", "build"),
            expected_exit_code=1,
            expected_output_fragments=(
                "customer_snapshot",
                "check_columns [*] did not match any data columns",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_missing_check_snapshot_output_column_when_building_then_cli_reports_failure(
    test_case: SnapshotCheckFailureBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="check_snapshot_failure_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "check_snapshot_failure.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(test_case.initial_seed_sql)
    connection.close()

    result: object = run_sqb(command=test_case.command, project_dir=project_dir)

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    output: str = result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_output_fragments:
        assert fragment in output


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotFullRefreshFailureBuildE2ETestCase(
            description="current-state snapshot full refresh is denied by default through CLI",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "snapshot_full_refresh_project"
                    adapter = "duckdb"

                    [connection]
                    database = "snapshot_full_refresh.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customers
                        description: Test source raw_customers.
                        schema: main
                        table: raw_customers
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at
                    );

                    SELECT customer_id, plan, updated_at
                    FROM __source("raw_customers")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'basic' AS plan,
                  TIMESTAMP '2024-01-01 00:00:00' AS updated_at
                """
            ).strip(),
            initial_command=("--no-color", "build"),
            full_refresh_command=("--no-color", "build", "--full-refresh"),
            expected_exit_code=1,
            expected_output_fragments=(
                "full refresh is denied for snapshot model 'customer_snapshot'",
                "snapshot_full_refresh policy",
            ),
        ),
        SnapshotFullRefreshFailureBuildE2ETestCase(
            description="current-state snapshot full refresh is denied by default through run CLI",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "snapshot_full_refresh_project"
                    adapter = "duckdb"

                    [connection]
                    database = "snapshot_full_refresh.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customers
                        description: Test source raw_customers.
                        schema: main
                        table: raw_customers
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at
                    );

                    SELECT customer_id, plan, updated_at
                    FROM __source("raw_customers")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'basic' AS plan,
                  TIMESTAMP '2024-01-01 00:00:00' AS updated_at
                """
            ).strip(),
            initial_command=("--no-color", "build"),
            full_refresh_command=(
                "--no-color",
                "build",
                "--no-tests",
                "--no-audits",
                "--full-refresh",
            ),
            expected_exit_code=1,
            expected_output_fragments=(
                "full refresh is denied for snapshot model 'customer_snapshot'",
                "snapshot_full_refresh policy",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snapshot_full_refresh_default_deny_when_building_then_cli_reports_failure(
    test_case: SnapshotFullRefreshFailureBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snapshot_full_refresh_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "snapshot_full_refresh.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(test_case.initial_seed_sql)
    connection.close()

    initial_result: object = run_sqb(command=test_case.initial_command, project_dir=project_dir)
    assert initial_result.returncode == 0, initial_result.stdout + initial_result.stderr

    result: object = run_sqb(command=test_case.full_refresh_command, project_dir=project_dir)

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    output: str = result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_output_fragments:
        assert fragment in output


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotFullRefreshSuccessBuildE2ETestCase(
            description="current-state timestamp snapshot full refresh rebuilds when allowed",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "historical_snapshot_full_refresh_project"
                    adapter = "duckdb"

                    [connection]
                    database = "historical_snapshot_full_refresh.duckdb"

                    [snapshots]
                    current_state_full_refresh = "allow"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customers
                        description: Test source raw_customers.
                        schema: main
                        table: raw_customers
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at
                    );

                    SELECT customer_id, plan, updated_at
                    FROM __source("raw_customers")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-01-01' AS updated_at
                """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                    CREATE OR REPLACE TABLE main.raw_customers AS
                    SELECT 1 AS customer_id, 'team' AS plan, TIMESTAMP '2024-02-01' AS updated_at
                    """
                ).strip(),
            ),
            initial_command=("--no-color", "build"),
            full_refresh_command=("--no-color", "build", "--full-refresh"),
            expected_exit_code=0,
            expected_query=(
                "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_initial_rows=((1, "basic", "2024-01-01 00:00:00", None),),
            expected_refreshed_rows=((1, "team", "2024-02-01 00:00:00", None),),
        ),
        SnapshotFullRefreshSuccessBuildE2ETestCase(
            description="current-state check snapshot full refresh rebuilds when allowed",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "historical_snapshot_full_refresh_project"
                    adapter = "duckdb"

                    [connection]
                    database = "historical_snapshot_full_refresh.duckdb"

                    [snapshots]
                    current_state_full_refresh = "allow"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customers
                        description: Test source raw_customers.
                        schema: main
                        table: raw_customers
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy check,
                      check_columns [plan]
                    );

                    SELECT customer_id, plan
                    FROM __source("raw_customers")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customers AS
                SELECT 1 AS customer_id, 'basic' AS plan
                """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                    CREATE OR REPLACE TABLE main.raw_customers AS
                    SELECT 1 AS customer_id, 'team' AS plan
                    """
                ).strip(),
            ),
            initial_command=("--no-color", "build"),
            full_refresh_command=("--no-color", "build", "--full-refresh"),
            expected_exit_code=0,
            expected_query=(
                "SELECT customer_id, plan, valid_to FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_initial_rows=((1, "basic", None),),
            expected_refreshed_rows=((1, "team", None),),
        ),
        SnapshotFullRefreshSuccessBuildE2ETestCase(
            description="historical check snapshot full refresh runs with confirmation flag",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "historical_snapshot_full_refresh_project"
                    adapter = "duckdb"

                    [connection]
                    database = "historical_snapshot_full_refresh.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customer_daily
                        description: Test source raw_customer_daily.
                        schema: main
                        table: raw_customer_daily
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy check,
                      check_columns [plan],
                      observed_at observed_at
                    );

                    SELECT customer_id, plan, observed_at
                    FROM __source("raw_customer_daily")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customer_daily AS
                SELECT 1 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-01-01' AS observed_at
                UNION ALL SELECT 1, 'pro', TIMESTAMP '2024-01-02'
                """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                    CREATE OR REPLACE TABLE main.raw_customer_daily AS
                    SELECT 1 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-02-01' AS observed_at
                    UNION ALL SELECT 1, 'team', TIMESTAMP '2024-02-03'
                    """
                ).strip(),
            ),
            initial_command=("--no-color", "build"),
            full_refresh_command=(
                "--no-color",
                "build",
                "--full-refresh",
                "--allow-snapshot-full-refresh",
            ),
            expected_exit_code=0,
            expected_query=(
                "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_initial_rows=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-02 00:00:00"),
                (1, "pro", "2024-01-02 00:00:00", None),
            ),
            expected_refreshed_rows=(
                (1, "basic", "2024-02-01 00:00:00", "2024-02-03 00:00:00"),
                (1, "team", "2024-02-03 00:00:00", None),
            ),
        ),
        SnapshotFullRefreshSuccessBuildE2ETestCase(
            description=(
                "historical check snapshot full refresh rebuilds through build without tests or audits"
            ),
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "historical_snapshot_full_refresh_project"
                    adapter = "duckdb"

                    [connection]
                    database = "historical_snapshot_full_refresh.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customer_daily
                        description: Test source raw_customer_daily.
                        schema: main
                        table: raw_customer_daily
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy check,
                      check_columns [plan],
                      observed_at observed_at
                    );

                    SELECT customer_id, plan, observed_at
                    FROM __source("raw_customer_daily")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customer_daily AS
                SELECT 1 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-01-01' AS observed_at
                UNION ALL SELECT 1, 'pro', TIMESTAMP '2024-01-02'
                """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                    CREATE OR REPLACE TABLE main.raw_customer_daily AS
                    SELECT 1 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-02-01' AS observed_at
                    UNION ALL SELECT 1, 'team', TIMESTAMP '2024-02-03'
                    """
                ).strip(),
            ),
            initial_command=("--no-color", "build"),
            full_refresh_command=(
                "--no-color",
                "build",
                "--full-refresh",
                "--allow-snapshot-full-refresh",
                "--no-tests",
                "--no-audits",
            ),
            expected_exit_code=0,
            expected_query=(
                "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_initial_rows=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-02 00:00:00"),
                (1, "pro", "2024-01-02 00:00:00", None),
            ),
            expected_refreshed_rows=(
                (1, "basic", "2024-02-01 00:00:00", "2024-02-03 00:00:00"),
                (1, "team", "2024-02-03 00:00:00", None),
            ),
        ),
        SnapshotFullRefreshSuccessBuildE2ETestCase(
            description="historical timestamp snapshot full refresh rebuilds through build command",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "historical_snapshot_full_refresh_project"
                    adapter = "duckdb"

                    [connection]
                    database = "historical_snapshot_full_refresh.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customer_extracts
                        description: Test source raw_customer_extracts.
                        schema: main
                        table: raw_customer_extracts
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at,
                      observed_at observed_at,
                      historical_input snapshot
                    );

                    SELECT customer_id, plan, updated_at, observed_at
                    FROM __source("raw_customer_extracts")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customer_extracts AS
                SELECT 1 AS customer_id, 'basic' AS plan,
                  TIMESTAMP '2024-01-01' AS updated_at,
                  TIMESTAMP '2024-01-02' AS observed_at
                UNION ALL SELECT 1, 'pro', TIMESTAMP '2024-01-04', TIMESTAMP '2024-01-06'
                """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                    CREATE OR REPLACE TABLE main.raw_customer_extracts AS
                    SELECT 1 AS customer_id, 'basic' AS plan,
                      TIMESTAMP '2024-02-01' AS updated_at,
                      TIMESTAMP '2024-02-02' AS observed_at
                    UNION ALL SELECT 1, 'team', TIMESTAMP '2024-02-04', TIMESTAMP '2024-02-06'
                    """
                ).strip(),
            ),
            initial_command=("--no-color", "build"),
            full_refresh_command=(
                "--no-color",
                "build",
                "--full-refresh",
                "--allow-snapshot-full-refresh",
            ),
            expected_exit_code=0,
            expected_query=(
                "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_initial_rows=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-04 00:00:00"),
                (1, "pro", "2024-01-04 00:00:00", None),
            ),
            expected_refreshed_rows=(
                (1, "basic", "2024-02-01 00:00:00", "2024-02-04 00:00:00"),
                (1, "team", "2024-02-04 00:00:00", None),
            ),
        ),
        SnapshotFullRefreshSuccessBuildE2ETestCase(
            description=(
                "historical timestamp snapshot full refresh rebuilds through build "
                "without tests or audits"
            ),
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "historical_snapshot_full_refresh_project"
                    adapter = "duckdb"

                    [connection]
                    database = "historical_snapshot_full_refresh.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customer_extracts
                        description: Test source raw_customer_extracts.
                        schema: main
                        table: raw_customer_extracts
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at,
                      observed_at observed_at,
                      historical_input snapshot
                    );

                    SELECT customer_id, plan, updated_at, observed_at
                    FROM __source("raw_customer_extracts")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customer_extracts AS
                SELECT 1 AS customer_id, 'basic' AS plan,
                  TIMESTAMP '2024-01-01' AS updated_at,
                  TIMESTAMP '2024-01-02' AS observed_at
                UNION ALL SELECT 1, 'pro', TIMESTAMP '2024-01-04', TIMESTAMP '2024-01-06'
                """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                    CREATE OR REPLACE TABLE main.raw_customer_extracts AS
                    SELECT 1 AS customer_id, 'basic' AS plan,
                      TIMESTAMP '2024-02-01' AS updated_at,
                      TIMESTAMP '2024-02-02' AS observed_at
                    UNION ALL SELECT 1, 'team', TIMESTAMP '2024-02-04', TIMESTAMP '2024-02-06'
                    """
                ).strip(),
            ),
            initial_command=("--no-color", "build"),
            full_refresh_command=(
                "--no-color",
                "build",
                "--full-refresh",
                "--allow-snapshot-full-refresh",
                "--no-tests",
                "--no-audits",
            ),
            expected_exit_code=0,
            expected_query=(
                "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_initial_rows=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-04 00:00:00"),
                (1, "pro", "2024-01-04 00:00:00", None),
            ),
            expected_refreshed_rows=(
                (1, "basic", "2024-02-01 00:00:00", "2024-02-04 00:00:00"),
                (1, "team", "2024-02-04 00:00:00", None),
            ),
        ),
        SnapshotFullRefreshSuccessBuildE2ETestCase(
            description="historical timestamp changes full refresh rebuilds through build command",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "historical_snapshot_full_refresh_project"
                    adapter = "duckdb"

                    [connection]
                    database = "historical_snapshot_full_refresh.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customer_changes
                        description: Test source raw_customer_changes.
                        schema: main
                        table: raw_customer_changes
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at,
                      observed_at observed_at,
                      historical_input changes
                    );

                    SELECT customer_id, plan, updated_at, observed_at
                    FROM __source("raw_customer_changes")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customer_changes AS
                SELECT 1 AS customer_id, 'basic' AS plan,
                  TIMESTAMP '2024-01-01' AS updated_at,
                  TIMESTAMP '2024-01-10' AS observed_at
                UNION ALL SELECT 1, 'pro', TIMESTAMP '2024-01-04', TIMESTAMP '2024-01-10'
                """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                    CREATE OR REPLACE TABLE main.raw_customer_changes AS
                    SELECT 1 AS customer_id, 'team' AS plan,
                      TIMESTAMP '2024-02-01' AS updated_at,
                      TIMESTAMP '2024-02-10' AS observed_at
                    UNION ALL SELECT 1, 'enterprise', TIMESTAMP '2024-02-04', TIMESTAMP '2024-02-10'
                    """
                ).strip(),
            ),
            initial_command=("--no-color", "build"),
            full_refresh_command=(
                "--no-color",
                "build",
                "--full-refresh",
                "--allow-snapshot-full-refresh",
            ),
            expected_exit_code=0,
            expected_query=(
                "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_initial_rows=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-04 00:00:00"),
                (1, "pro", "2024-01-04 00:00:00", None),
            ),
            expected_refreshed_rows=(
                (1, "team", "2024-02-01 00:00:00", "2024-02-04 00:00:00"),
                (1, "enterprise", "2024-02-04 00:00:00", None),
            ),
        ),
        SnapshotFullRefreshSuccessBuildE2ETestCase(
            description=(
                "historical timestamp changes full refresh rebuilds through build "
                "without tests or audits"
            ),
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "historical_snapshot_full_refresh_project"
                    adapter = "duckdb"

                    [connection]
                    database = "historical_snapshot_full_refresh.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customer_changes
                        description: Test source raw_customer_changes.
                        schema: main
                        table: raw_customer_changes
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at,
                      observed_at observed_at,
                      historical_input changes
                    );

                    SELECT customer_id, plan, updated_at, observed_at
                    FROM __source("raw_customer_changes")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customer_changes AS
                SELECT 1 AS customer_id, 'basic' AS plan,
                  TIMESTAMP '2024-01-01' AS updated_at,
                  TIMESTAMP '2024-01-10' AS observed_at
                UNION ALL SELECT 1, 'pro', TIMESTAMP '2024-01-04', TIMESTAMP '2024-01-10'
                """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                    CREATE OR REPLACE TABLE main.raw_customer_changes AS
                    SELECT 1 AS customer_id, 'team' AS plan,
                      TIMESTAMP '2024-02-01' AS updated_at,
                      TIMESTAMP '2024-02-10' AS observed_at
                    UNION ALL SELECT 1, 'enterprise', TIMESTAMP '2024-02-04', TIMESTAMP '2024-02-10'
                    """
                ).strip(),
            ),
            initial_command=("--no-color", "build"),
            full_refresh_command=(
                "--no-color",
                "build",
                "--full-refresh",
                "--allow-snapshot-full-refresh",
                "--no-tests",
                "--no-audits",
            ),
            expected_exit_code=0,
            expected_query=(
                "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_initial_rows=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-04 00:00:00"),
                (1, "pro", "2024-01-04 00:00:00", None),
            ),
            expected_refreshed_rows=(
                (1, "team", "2024-02-01 00:00:00", "2024-02-04 00:00:00"),
                (1, "enterprise", "2024-02-04 00:00:00", None),
            ),
        ),
        SnapshotFullRefreshSuccessBuildE2ETestCase(
            description="historical hard-delete snapshot full refresh rebuilds through build command",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "historical_snapshot_full_refresh_project"
                    adapter = "duckdb"

                    [connection]
                    database = "historical_snapshot_full_refresh.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customer_daily
                        description: Test source raw_customer_daily.
                        schema: main
                        table: raw_customer_daily
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy check,
                      check_columns [plan],
                      observed_at observed_at,
                      invalidate_hard_deletes true
                    );

                    SELECT customer_id, plan, observed_at
                    FROM __source("raw_customer_daily")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customer_daily AS
                SELECT 1 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-01-01' AS observed_at
                UNION ALL SELECT 2, 'basic', TIMESTAMP '2024-01-01'
                UNION ALL SELECT 1, 'pro', TIMESTAMP '2024-01-02'
                """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                    CREATE OR REPLACE TABLE main.raw_customer_daily AS
                    SELECT 1 AS customer_id, 'team' AS plan, TIMESTAMP '2024-02-01' AS observed_at
                    UNION ALL SELECT 2, 'team', TIMESTAMP '2024-02-01'
                    UNION ALL SELECT 2, 'enterprise', TIMESTAMP '2024-02-03'
                    """
                ).strip(),
            ),
            initial_command=("--no-color", "build"),
            full_refresh_command=(
                "--no-color",
                "build",
                "--full-refresh",
                "--allow-snapshot-full-refresh",
            ),
            expected_exit_code=0,
            expected_query=(
                "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_initial_rows=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-02 00:00:00"),
                (1, "pro", "2024-01-02 00:00:00", None),
                (2, "basic", "2024-01-01 00:00:00", "2024-01-02 00:00:00"),
            ),
            expected_refreshed_rows=(
                (1, "team", "2024-02-01 00:00:00", "2024-02-03 00:00:00"),
                (2, "team", "2024-02-01 00:00:00", "2024-02-03 00:00:00"),
                (2, "enterprise", "2024-02-03 00:00:00", None),
            ),
        ),
        SnapshotFullRefreshSuccessBuildE2ETestCase(
            description=(
                "historical hard-delete snapshot full refresh rebuilds through build "
                "without tests or audits"
            ),
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "historical_snapshot_full_refresh_project"
                    adapter = "duckdb"

                    [connection]
                    database = "historical_snapshot_full_refresh.duckdb"
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_customer_extracts
                        description: Test source raw_customer_extracts.
                        schema: main
                        table: raw_customer_extracts
                    """
                ).strip()
                + "\n",
                "sql/models/customer_snapshot.sql": dedent(
                    """
                    MODEL (description "Test model customer_snapshot.",
                      materialized snapshot,
                      unique_key [customer_id],
                      snapshot_strategy timestamp,
                      updated_at updated_at,
                      observed_at observed_at,
                      historical_input snapshot,
                      invalidate_hard_deletes true
                    );

                    SELECT customer_id, plan, updated_at, observed_at
                    FROM __source("raw_customer_extracts")
                    """
                ).strip()
                + "\n",
            },
            initial_seed_sql=dedent(
                """
                CREATE TABLE main.raw_customer_extracts AS
                SELECT 1 AS customer_id, 'basic' AS plan,
                  TIMESTAMP '2024-01-01' AS updated_at,
                  TIMESTAMP '2024-01-02' AS observed_at
                UNION ALL SELECT 2, 'basic', TIMESTAMP '2024-01-01', TIMESTAMP '2024-01-02'
                UNION ALL SELECT 1, 'pro', TIMESTAMP '2024-01-04', TIMESTAMP '2024-01-06'
                """
            ).strip(),
            mutation_sql=(
                dedent(
                    """
                    CREATE OR REPLACE TABLE main.raw_customer_extracts AS
                    SELECT 1 AS customer_id, 'team' AS plan,
                      TIMESTAMP '2024-02-01' AS updated_at,
                      TIMESTAMP '2024-02-02' AS observed_at
                    UNION ALL SELECT 2, 'team', TIMESTAMP '2024-02-01', TIMESTAMP '2024-02-02'
                    UNION ALL SELECT 2, 'enterprise', TIMESTAMP '2024-02-04', TIMESTAMP '2024-02-06'
                    """
                ).strip(),
            ),
            initial_command=("--no-color", "build"),
            full_refresh_command=(
                "--no-color",
                "build",
                "--full-refresh",
                "--allow-snapshot-full-refresh",
                "--no-tests",
                "--no-audits",
            ),
            expected_exit_code=0,
            expected_query=(
                "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                "ORDER BY customer_id, valid_from"
            ),
            expected_initial_rows=(
                (1, "basic", "2024-01-01 00:00:00", "2024-01-04 00:00:00"),
                (1, "pro", "2024-01-04 00:00:00", None),
                (2, "basic", "2024-01-01 00:00:00", "2024-01-06 00:00:00"),
            ),
            expected_refreshed_rows=(
                (1, "team", "2024-02-01 00:00:00", "2024-02-06 00:00:00"),
                (2, "team", "2024-02-01 00:00:00", "2024-02-04 00:00:00"),
                (2, "enterprise", "2024-02-04 00:00:00", None),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snapshot_full_refresh_allowed_when_building_then_rebuilds_history(
    test_case: SnapshotFullRefreshSuccessBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="historical_snapshot_full_refresh_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "historical_snapshot_full_refresh.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(test_case.initial_seed_sql)
    connection.close()

    initial_result: object = run_sqb(command=test_case.initial_command, project_dir=project_dir)
    assert initial_result.returncode == 0, initial_result.stdout + initial_result.stderr
    rows_after_initial: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_query)
    )

    connection = duckdb.connect(str(db_path))
    statement: str
    for statement in test_case.mutation_sql:
        connection.execute(statement)
    connection.close()

    refresh_result: object = run_sqb(
        command=test_case.full_refresh_command, project_dir=project_dir
    )
    assert refresh_result.returncode == test_case.expected_exit_code, (
        refresh_result.stdout + refresh_result.stderr
    )
    rows_after_refresh: tuple[tuple[object, ...], ...] = tuple(
        tuple(row) for row in query_duckdb(db_path=db_path, sql=test_case.expected_query)
    )

    assert rows_after_initial == test_case.expected_initial_rows
    assert rows_after_refresh == test_case.expected_refreshed_rows
