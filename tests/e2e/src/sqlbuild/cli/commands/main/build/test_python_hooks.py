"""E2E tests for Python lifecycle hooks in sqb build."""

from __future__ import annotations

import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    LongPythonHookNameBuildE2ETestCase,
    PythonHookFailureBuildE2ETestCase,
    PythonHooksBuildE2ETestCase,
    PythonHookSkipBuildE2ETestCase,
    PythonHooksLifecycleMatrixBuildE2ETestCase,
    SnapshotPythonHooksBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    query_duckdb,
    run_sqb,
    table_exists,
)

_FAIL_HOOK_SOURCE: str = (
    "from sqlbuild.hooks import hook\n\n\n"
    "@hook\n"
    "def fail_hook(ctx, message):\n"
    "    '''Test hook fail_hook.'''\n    raise RuntimeError(message)\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonHookSkipBuildE2ETestCase(
            description="pre hook skip skips model and downstream",
            expected_exit_code=0,
            expected_output_fragments=(
                "pre_hook  python  skip_model",
                "soft skip: source disabled",
                "SKIP=2",
            ),
            expected_present_tables=(),
            expected_absent_tables=("upstream_orders", "downstream_orders"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_pre_hook_returns_skip_when_building_then_model_and_downstream_skip(
    test_case: PythonHookSkipBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_pre_hook_skip_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "python_pre_hook_skip_project"
                adapter = "duckdb"

                [connection]
                database = "python_pre_hook_skip_project.duckdb"
                """
            ).strip()
            + "\n",
            "hooks/python/skips.py": dedent(
                """
                from sqlbuild.hooks import hook


                @hook
                def skip_model(ctx):
                    '''Test hook skip_model.'''
                    return ctx.skip(reason="source disabled")
                """
            ).strip()
            + "\n",
            "models/upstream_orders.sql": dedent(
                """
                MODEL (description "Test model upstream_orders.",
                  materialized table,
                  pre_hooks [python("skip_model")]
                );

                SELECT 1 AS order_id
                """
            ).strip()
            + "\n",
            "models/downstream_orders.sql": dedent(
                """
                MODEL (description "Test model downstream_orders.", materialized table);

                SELECT order_id FROM __ref("upstream_orders")
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--select", "upstream_orders+"),
        project_dir=project_dir,
    )
    db_path: Path = project_dir / "python_pre_hook_skip_project.duckdb"

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    for fragment in test_case.expected_output_fragments:
        assert fragment in result.stdout
    for table_name in test_case.expected_present_tables:
        assert table_exists(db_path=db_path, table_name=table_name)
    for table_name in test_case.expected_absent_tables:
        assert not table_exists(db_path=db_path, table_name=table_name)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonHookSkipBuildE2ETestCase(
            description="post hook skip keeps model relation and skips downstream",
            expected_exit_code=0,
            expected_output_fragments=(
                "post_hook python  skip_downstream",
                "soft skip: publish disabled",
                "SKIP=2",
            ),
            expected_present_tables=("upstream_orders",),
            expected_absent_tables=("downstream_orders",),
            expected_rows=((1,),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_post_hook_skip_when_building_then_keeps_relation_and_skips_downstream(
    test_case: PythonHookSkipBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_post_hook_skip_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "python_post_hook_skip_project"
                adapter = "duckdb"

                [connection]
                database = "python_post_hook_skip_project.duckdb"
                """
            ).strip()
            + "\n",
            "hooks/python/skips.py": dedent(
                """
                from sqlbuild.hooks import hook


                @hook
                def skip_downstream(ctx):
                    '''Test hook skip_downstream.'''
                    return ctx.skip(reason="publish disabled")
                """
            ).strip()
            + "\n",
            "models/upstream_orders.sql": dedent(
                """
                MODEL (description "Test model upstream_orders.",
                  materialized table,
                  post_hooks [python("skip_downstream")]
                );

                SELECT 1 AS order_id
                """
            ).strip()
            + "\n",
            "models/downstream_orders.sql": dedent(
                """
                MODEL (description "Test model downstream_orders.", materialized table);

                SELECT order_id FROM __ref("upstream_orders")
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--select", "upstream_orders+"),
        project_dir=project_dir,
    )
    db_path: Path = project_dir / "python_post_hook_skip_project.duckdb"

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    for fragment in test_case.expected_output_fragments:
        assert fragment in result.stdout
    for table_name in test_case.expected_present_tables:
        assert table_exists(db_path=db_path, table_name=table_name)
    for table_name in test_case.expected_absent_tables:
        assert not table_exists(db_path=db_path, table_name=table_name)
    assert tuple(query_duckdb(db_path=db_path, sql="SELECT order_id FROM upstream_orders")) == (
        test_case.expected_rows
    )


@pytest.mark.parametrize(
    "test_case",
    [
        PythonHooksBuildE2ETestCase(
            description="build executes Python pre and post hooks",
            expected_exit_code=0,
            expected_orders_rows=((42, "created by hook"),),
            expected_hook_log_rows=(("orders", "orders", "post"),),
            expected_output_fragments=(
                "pre_hook  sql     select_one",
                "pre_hook  python  create_hook_data",
                "post_hook python  record_hook_completion",
                "post_hook sql     SELECT 1",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_project_with_python_hooks_when_building_then_hooks_execute(
    test_case: PythonHooksBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_hooks_build_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "python_hooks_build_project"
                adapter = "duckdb"

                [connection]
                database = "python_hooks_build_project.duckdb"
                """
            ).strip()
            + "\n",
            "hooks/python/lifecycle.py": dedent(
                """
                from sqlbuild.hooks import hook


                @hook
                def create_hook_data(ctx, value):
                    '''Test hook create_hook_data.'''
                    ctx.execute_sql(
                        f"CREATE TABLE {ctx.destination.schema}.hook_data AS "
                        f"SELECT {value} AS id, 'created by hook' AS label"
                    )


                @hook
                def record_hook_completion(ctx, phase):
                    '''Test hook record_hook_completion.'''
                    ctx.execute_sql(
                        f"CREATE TABLE {ctx.destination.schema}.hook_log AS "
                        f"SELECT '{ctx.model_name}' AS model_name, "
                        f"'{ctx.destination.name}' AS relation_name, '{phase}' AS phase"
                    )
                """
            ).strip()
            + "\n",
            "hooks/sql/select_one.sql": dedent(
                """
                HOOK (description "Execute a parameterized SQL payload");

                SELECT @value;
                SELECT @value + 1
                """
            ).strip()
            + "\n",
            "models/orders.sql": dedent(
                """
                MODEL (description "Test model orders.",
                  materialized table,
                  pre_hooks [sql("select_one", value: 1), python("create_hook_data", value: 42)],
                  post_hooks [python("record_hook_completion", phase: "post"), inline_sql("SELECT 1")]
                );

                SELECT id, label FROM main.hook_data
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stderr
    for fragment in test_case.expected_output_fragments:
        assert fragment in result.stdout
    assert query_duckdb(
        db_path=project_dir / "python_hooks_build_project.duckdb",
        sql="SELECT id, label FROM main.orders",
    ) == list(test_case.expected_orders_rows)
    assert query_duckdb(
        db_path=project_dir / "python_hooks_build_project.duckdb",
        sql="SELECT model_name, relation_name, phase FROM main.hook_log",
    ) == list(test_case.expected_hook_log_rows)
    assert query_duckdb(
        db_path=project_dir / "python_hooks_build_project.duckdb",
        sql=(
            "SELECT node_type, node_name FROM main._sqlbuild_fingerprints "
            "WHERE node_type = 'hook' ORDER BY node_name"
        ),
    ) == [("hook", "create_hook_data"), ("hook", "record_hook_completion")]


@pytest.mark.parametrize(
    "test_case",
    [
        PythonHooksLifecycleMatrixBuildE2ETestCase(
            description="build executes Python hooks across materialization kinds",
            expected_exit_code=0,
            expected_output_fragments=(
                "view      stg_orders",
                "table     fact_orders",
                "table     order_status_index  (delete_insert)",
                "table     hourly_order_activity  (delete_insert)",
                "customer_snapshot",
                "custom    custom_orders",
                "pre_hook  python  log_hook",
                "post_hook python  log_hook",
            ),
            expected_hook_log_rows=(
                ("custom_orders", "post_hooks", 1),
                ("custom_orders", "pre_hooks", 1),
                ("customer_snapshot", "post_hooks", 1),
                ("customer_snapshot", "pre_hooks", 1),
                ("fact_orders", "post_hooks", 1),
                ("fact_orders", "pre_hooks", 1),
                ("hourly_order_activity", "post_hooks", 1),
                ("hourly_order_activity", "pre_hooks", 1),
                ("order_status_index", "post_hooks", 1),
                ("order_status_index", "pre_hooks", 1),
                ("stg_orders", "post_hooks", 1),
                ("stg_orders", "pre_hooks", 1),
            ),
            expected_query_results=(
                (
                    "SELECT COUNT(*) FROM main.fact_orders",
                    ((3,),),
                ),
                (
                    "SELECT order_id, customer_id FROM main.order_status_index ORDER BY order_id",
                    ((1, 10), (2, 11), (3, 10)),
                ),
                (
                    (
                        "SELECT CAST(activity_hour AS VARCHAR), orders_placed "
                        "FROM main.hourly_order_activity ORDER BY activity_hour"
                    ),
                    (
                        ("2026-01-01 00:00:00", 1),
                        ("2026-01-01 01:00:00", 1),
                        ("2026-01-02 02:00:00", 1),
                    ),
                ),
                (
                    (
                        "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), "
                        "CAST(valid_to AS VARCHAR) FROM main.customer_snapshot "
                        "ORDER BY customer_id"
                    ),
                    ((1, "basic", "2026-01-01 00:00:00", None),),
                ),
                (
                    "SELECT id, amount_cents FROM main.custom_orders ORDER BY id",
                    ((1, 100), (2, 200), (3, 150)),
                ),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_hooks_lifecycle_matrix_when_building_then_all_materializations_run_hooks(
    test_case: PythonHooksLifecycleMatrixBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_hooks_lifecycle_matrix_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "python_hooks_lifecycle_matrix_project"
                adapter = "duckdb"

                [connection]
                database = "python_hooks_lifecycle_matrix_project.duckdb"

                [defaults]
                materialized = "table"
                """
            ).strip()
            + "\n",
            "seed_raw_data.sql": dedent(
                """
                CREATE TABLE raw_orders (
                  id INTEGER,
                  customer_id INTEGER,
                  quantity INTEGER,
                  ordered_at TIMESTAMP,
                  status VARCHAR,
                  amount_cents INTEGER
                );

                INSERT INTO raw_orders VALUES
                  (1, 10, 1, '2026-01-01 00:30:00', 'completed', 100),
                  (2, 11, 2, '2026-01-01 01:30:00', 'completed', 200),
                  (3, 10, 1, '2026-01-02 02:00:00', 'placed', 150);

                CREATE TABLE raw_customers AS
                SELECT 1 AS customer_id, 'basic' AS plan,
                  TIMESTAMP '2026-01-01 00:00:00' AS updated_at;

                CREATE TABLE hook_log (model_name VARCHAR, phase VARCHAR);
                """
            ).strip()
            + "\n",
            "sources/raw.yml": dedent(
                """
                sources:
                  - name: raw_orders
                    description: Test source raw_orders.
                    schema: main
                    table: raw_orders
                  - name: raw_customers
                    description: Test source raw_customers.
                    schema: main
                    table: raw_customers
                """
            ).strip()
            + "\n",
            "hooks/python/lifecycle.py": dedent(
                """
                from sqlbuild.hooks import hook


                @hook
                def log_hook(ctx):
                    '''Test hook log_hook.'''
                    ctx.execute_sql(
                        f"INSERT INTO {ctx.destination.schema}.hook_log VALUES "
                        f"('{ctx.model_name}', '{ctx.phase}')"
                    )
                """
            ).strip()
            + "\n",
            "materializations/copy_table.py": dedent(
                """
                from sqlbuild.executor.custom.models import (
                    MaterializationContext,
                    MaterializationResult,
                )


                def materialize(ctx: MaterializationContext) -> MaterializationResult:
                    ctx.execute_sql(f"CREATE TABLE {ctx.destination} AS {ctx.sql}")
                    return MaterializationResult(relation=ctx.destination)
                """
            ).strip()
            + "\n",
            "models/staging/stg_orders.sql": dedent(
                """
                MODEL (description "Test model stg_orders.",
                  materialized view,
                  pre_hooks [python("log_hook")],
                  post_hooks [python("log_hook")]
                );

                SELECT
                  id AS order_id,
                  customer_id,
                  quantity,
                  ordered_at,
                  status,
                  amount_cents
                FROM __source("raw_orders")
                """
            ).strip()
            + "\n",
            "models/marts/fact_orders.sql": dedent(
                """
                MODEL (description "Test model fact_orders.",
                  materialized table,
                  pre_hooks [python("log_hook")],
                  post_hooks [python("log_hook")]
                );

                SELECT
                  order_id,
                  customer_id,
                  quantity,
                  ordered_at,
                  status AS order_status,
                  amount_cents AS line_total_cents
                FROM __ref("stg_orders")
                """
            ).strip()
            + "\n",
            "models/intermediate/order_status_index.sql": dedent(
                """
                MODEL (description "Test model order_status_index.",
                  materialized incremental,
                  incremental_strategy delete_insert,
                  cursor order_id,
                  cursor_type integer,
                  cursor_inputs (
                    fact_orders order_id,
                  ),
                  pre_hooks [python("log_hook")],
                  post_hooks [python("log_hook")]
                );

                SELECT
                  order_id,
                  customer_id,
                  order_status,
                  ordered_at,
                  line_total_cents
                FROM __ref("fact_orders")
                """
            ).strip()
            + "\n",
            "models/marts/hourly_order_activity.sql": dedent(
                """
                MODEL (description "Test model hourly_order_activity.",
                  materialized incremental,
                  incremental_strategy delete_insert,
                  cursor activity_hour,
                  cursor_type timestamp,
                  cursor_grain hour,
                  cursor_inputs (
            fact_orders (column ordered_at, roles [filter, watermark]),
          ),
                  incremental_mode microbatch,
          microbatch_strategy watermark,
          cursor_watermark_mode all,
                  batch_size 1d,
                  pre_hooks [python("log_hook")],
                  post_hooks [python("log_hook")]
                );

                SELECT
                  DATE_TRUNC('hour', ordered_at) AS activity_hour,
                  COUNT(*) AS orders_placed,
                  SUM(quantity) AS quantity_total,
                  SUM(line_total_cents) AS revenue_cents
                FROM __ref("fact_orders")
                GROUP BY DATE_TRUNC('hour', ordered_at)
                """
            ).strip()
            + "\n",
            "models/snapshots/customer_snapshot.sql": dedent(
                """
                MODEL (description "Test model customer_snapshot.",
                  materialized snapshot,
                  unique_key [customer_id],
                  snapshot_strategy timestamp,
                  updated_at updated_at,
                  pre_hooks [python("log_hook")],
                  post_hooks [python("log_hook")]
                );

                SELECT customer_id, plan, updated_at
                FROM __source("raw_customers")
                """
            ).strip()
            + "\n",
            "models/custom/custom_orders.sql": dedent(
                """
                MODEL (description "Test model custom_orders.",
                  materialized copy_table,
                  pre_hooks [python("log_hook")],
                  post_hooks [python("log_hook")]
                );

                SELECT order_id AS id, line_total_cents AS amount_cents
                FROM __ref("fact_orders")
                """
            ).strip()
            + "\n",
        },
    )
    db_path: Path = project_dir / "python_hooks_lifecycle_matrix_project.duckdb"
    execute_duckdb(
        db_path=db_path,
        sql=(project_dir / "seed_raw_data.sql").read_text(encoding="utf-8"),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--concurrency", "4"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    for fragment in test_case.expected_output_fragments:
        assert fragment in result.stdout
    assert query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT model_name, phase, COUNT(*) FROM main.hook_log "
            "GROUP BY model_name, phase ORDER BY model_name, phase"
        ),
    ) == list(test_case.expected_hook_log_rows)
    for query, expected_rows in test_case.expected_query_results:
        assert query_duckdb(db_path=db_path, sql=query) == list(expected_rows)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonHookFailureBuildE2ETestCase(
            description="post hook failure blocks downstream model",
            expected_exit_code=1,
            expected_output_fragments=(
                "post_hook python  fail_hook",
                'post_hooks[0] python("fail_hook") failed: intentional post failure',
                "orders",
                "downstream_orders",
            ),
            expected_present_tables=("orders",),
            expected_absent_tables=("downstream_orders",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_post_hook_failure_when_building_graph_then_downstream_is_blocked(
    test_case: PythonHookFailureBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_hook_failure_build_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "python_hook_failure_build_project"
                adapter = "duckdb"

                [connection]
                database = "python_hook_failure_build_project.duckdb"
                """
            ).strip()
            + "\n",
            "hooks/python/lifecycle.py": dedent(
                """
                from sqlbuild.hooks import hook


                @hook
                def fail_hook(ctx, message):
                    '''Test hook fail_hook.'''
                    raise RuntimeError(message)
                """
            ).strip()
            + "\n",
            "models/orders.sql": dedent(
                """
                MODEL (description "Test model orders.",
                  materialized table,
                  post_hooks [python("fail_hook", message: "intentional post failure")]
                );

                SELECT 1 AS id
                """
            ).strip()
            + "\n",
            "models/downstream_orders.sql": dedent(
                """
                MODEL (description "Test model downstream_orders.", materialized table);

                SELECT id FROM __ref("orders")
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--select", "orders+"),
        project_dir=project_dir,
    )
    db_path: Path = project_dir / "python_hook_failure_build_project.duckdb"

    assert result.returncode == test_case.expected_exit_code, result.stderr
    for fragment in test_case.expected_output_fragments:
        assert fragment in result.stdout or fragment in result.stderr
    for table_name in test_case.expected_present_tables:
        assert table_exists(db_path=db_path, table_name=table_name)
    for table_name in test_case.expected_absent_tables:
        assert not table_exists(db_path=db_path, table_name=table_name)


@pytest.mark.parametrize(
    "test_case",
    (
        PythonHookFailureBuildE2ETestCase(
            description="pre hook failure shows failing Python hook row",
            model_sql=dedent(
                """
            MODEL (description "Test model.",
              materialized table,
              pre_hooks [python("fail_hook", message: "intentional pre failure")]
            );

            SELECT 1 AS id
            """
            ).strip()
            + "\n",
            expected_exit_code=1,
            expected_output_fragments=(
                "pre_hook  python  fail_hook",
                'pre_hooks[0] python("fail_hook") failed: intentional pre failure',
            ),
            expected_present_tables=(),
            expected_absent_tables=("orders",),
            hook_files={"hooks/python/lifecycle.py": _FAIL_HOOK_SOURCE},
        ),
        PythonHookFailureBuildE2ETestCase(
            description="pre hook failure shows failing SQL hook row",
            model_sql=dedent(
                """
            MODEL (description "Test model lifecycle.",
              materialized table,
              pre_hooks [inline_sql("SELECT * FROM missing_hook_table")]
            );

            SELECT 1 AS id
            """
            ).strip()
            + "\n",
            expected_exit_code=1,
            expected_output_fragments=(
                "pre_hook  sql     SELECT * FROM missing_hook_table",
                "missing_hook_table",
            ),
            expected_present_tables=(),
            expected_absent_tables=("orders",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_pre_hook_failure_when_building_then_cli_shows_failing_hook_row(
    test_case: PythonHookFailureBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    assert test_case.model_sql is not None
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_hook_pre_failure_build_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "python_hook_pre_failure_build_project"
                adapter = "duckdb"

                [connection]
                database = "python_hook_pre_failure_build_project.duckdb"
                """
            ).strip()
            + "\n",
            **test_case.hook_files,
            "models/orders.sql": test_case.model_sql,
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
    )
    db_path: Path = project_dir / "python_hook_pre_failure_build_project.duckdb"

    assert result.returncode == test_case.expected_exit_code, result.stderr
    for fragment in test_case.expected_output_fragments:
        assert fragment in result.stdout or fragment in result.stderr
    for table_name in test_case.expected_present_tables:
        assert table_exists(db_path=db_path, table_name=table_name)
    for table_name in test_case.expected_absent_tables:
        assert not table_exists(db_path=db_path, table_name=table_name)


@pytest.mark.parametrize(
    "test_case",
    [
        LongPythonHookNameBuildE2ETestCase(
            description="long Python hook name is truncated at display cap",
            expected_exit_code=0,
            expected_output_fragments=(
                "pre_hook  python  publish_customer_metadata_to_external_catalog_after_s... OK",
            ),
            unexpected_output_fragments=(
                "publish_customer_metadata_to_external_catalog_after_successful_materialization",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_long_python_hook_name_when_building_then_cli_truncates_label_at_cap(
    test_case: LongPythonHookNameBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="long_python_hook_name_build_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "long_python_hook_name_build_project"
                adapter = "duckdb"

                [connection]
                database = "long_python_hook_name_build_project.duckdb"
                """
            ).strip()
            + "\n",
            "hooks/python/lifecycle.py": (
                "from sqlbuild.hooks import hook\n\n\n"
                "@hook\n"
                "def publish_customer_metadata_to_external_catalog_after_successful_"
                "materialization(ctx):\n"
                '    """Test hook publish_customer_metadata_to_external_catalog_after_successful_materialization."""\n    ctx.log.info("long hook ran")\n'
            ),
            "models/orders.sql": (
                "MODEL (description 'Test model orders.',\n"
                "  materialized table,\n"
                '  pre_hooks [python("publish_customer_metadata_to_external_catalog_after_'
                'successful_materialization")]\n'
                ");\n\n"
                "SELECT 1 AS id\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stderr
    for fragment in test_case.expected_output_fragments:
        assert fragment in result.stdout
    for fragment in test_case.unexpected_output_fragments:
        assert fragment not in result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotPythonHooksBuildE2ETestCase(
            description="snapshot build executes Python pre and post hooks",
            expected_exit_code=0,
            expected_snapshot_rows=((1, "basic", "2026-01-01 00:00:00", None),),
            expected_hook_log_rows=(("customer_snapshot", "post_hooks"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_snapshot_with_python_hooks_when_building_then_hooks_execute(
    test_case: SnapshotPythonHooksBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snapshot_python_hooks_build_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "snapshot_python_hooks_build_project"
                adapter = "duckdb"

                [connection]
                database = "snapshot_python_hooks_build_project.duckdb"
                """
            ).strip()
            + "\n",
            "hooks/python/snapshot_hooks.py": dedent(
                """
                from sqlbuild.hooks import hook


                @hook
                def create_snapshot_source(ctx):
                    '''Test hook create_snapshot_source.'''
                    ctx.execute_sql(
                        "CREATE TABLE main.raw_customers AS "
                        "SELECT 1 AS customer_id, 'basic' AS plan, "
                        "TIMESTAMP '2026-01-01 00:00:00' AS updated_at"
                    )


                @hook
                def log_snapshot_hook(ctx):
                    '''Test hook log_snapshot_hook.'''
                    ctx.execute_sql(
                        f"CREATE TABLE {ctx.destination.schema}.snapshot_hook_log AS "
                        f"SELECT '{ctx.model_name}' AS model_name, '{ctx.phase}' AS phase"
                    )
                """
            ).strip()
            + "\n",
            "models/customer_snapshot.sql": dedent(
                """
                MODEL (description "Test model customer_snapshot.",
                  materialized snapshot,
                  unique_key [customer_id],
                  snapshot_strategy timestamp,
                  updated_at updated_at,
                  pre_hooks [python("create_snapshot_source")],
                  post_hooks [python("log_snapshot_hook")]
                );

                SELECT customer_id, plan, updated_at FROM main.raw_customers
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
    )
    db_path: Path = project_dir / "snapshot_python_hooks_build_project.duckdb"

    assert result.returncode == test_case.expected_exit_code, result.stderr
    assert query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT customer_id, plan, CAST(valid_from AS VARCHAR), CAST(valid_to AS VARCHAR) "
            "FROM main.customer_snapshot ORDER BY customer_id"
        ),
    ) == list(test_case.expected_snapshot_rows)
    assert query_duckdb(
        db_path=db_path,
        sql="SELECT model_name, phase FROM main.snapshot_hook_log",
    ) == list(test_case.expected_hook_log_rows)
