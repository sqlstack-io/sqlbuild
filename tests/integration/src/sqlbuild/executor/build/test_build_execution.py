"""Integration tests for build plan execution."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.build.models import BuildExecutionResult
from sqlbuild.executor.build.types import BuildStatus
from sqlbuild.executor.scheduling.types import ExecutionStatus
from tests.integration.src.sqlbuild.executor.build._test_types import (
    BuildExecutionTestCase,
)
from tests.integration.src.sqlbuild.executor.build.helpers import (
    run_build_for_project,
    verify_audit_counts,
    verify_function_statuses,
    verify_model_statuses,
    verify_test_counts,
    verify_warehouse_state,
)

_PROJECT_YML: str = (
    'name = "demo"\n'
    'adapter = "duckdb"\n\n'
    "[connection]\n"
    'database = ":memory:"\n\n'
    "[settings]\n"
    'default_audit_severity = "error"\n'
)

_PROJECT_YML_WARN: str = (
    'name = "demo"\n'
    'adapter = "duckdb"\n\n'
    "[connection]\n"
    'database = ":memory:"\n\n'
    "[settings]\n"
    'default_audit_severity = "warn"\n'
)

_PROJECT_YML_IMMEDIATE: str = (
    'name = "demo"\n'
    'adapter = "duckdb"\n\n'
    "[connection]\n"
    'database = ":memory:"\n\n'
    "[settings]\n"
    'default_audit_severity = "error"\n'
    'table_promotion_mode = "immediate"\n'
)

_RAW_ORDERS_WITH_NOT_NULL_AUDIT: str = (
    "sources:\n"
    "  - name: raw_orders\n    description: Test source raw_orders.\n"
    "    schema: main\n"
    "    table: raw_orders\n"
    "    columns:\n"
    "      - name: id\n"
    "        audits:\n"
    "          - not_null\n"
)
_NOT_NULL_AUDIT: str = 'AUDIT ();\n\nSELECT @column FROM __ref("@model") WHERE @column IS NULL'
_TABLE_WITH_ID_NOT_NULL_AUDIT: str = (
    "MODEL (description 'Test model.', materialized table, columns (id (audits [not_null])));\n\n"
)
_VIEW_WITH_ID_NOT_NULL_AUDIT: str = (
    "MODEL (description 'Test model.', materialized view, columns (id (audits [not_null])));\n\n"
)


_PASSING_TEST_SQL: str = (
    "TEST ();\n\n"
    "WITH\n"
    "__ref__stg_orders AS (\n"
    "  SELECT 1 AS id, 'alice' AS name\n"
    "),\n"
    "__expected__stg_orders AS (\n"
    "  SELECT 1 AS id, 'alice' AS name\n"
    ")\n"
    "SELECT 1\n"
)

_FAILING_TEST_SQL: str = (
    "TEST ();\n\n"
    "WITH\n"
    "__ref__stg_orders AS (\n"
    "  SELECT 1 AS id, 'alice' AS name\n"
    "),\n"
    "__expected__stg_orders AS (\n"
    "  SELECT 1 AS id, 'wrong_name' AS name\n"
    ")\n"
    "SELECT 1\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        BuildExecutionTestCase(
            description="sql udf builds before dependent model",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "functions/sql/is_positive_int.sql": (
                    "FUNCTION (description 'Test function is_positive_int.',\n"
                    "  arguments (a_string VARCHAR),\n"
                    "  returns BOOLEAN\n"
                    ");\n\n"
                    "regexp_matches(a_string, '^[0-9]+$')"
                ),
                "models/validated_orders.sql": (
                    "MODEL (description 'Test model validated_orders.', materialized table);\n\n"
                    "WITH input(value) AS (VALUES ('123'), ('abc'))\n"
                    'SELECT input.value, __udf("is_positive_int")(input.value) AS is_positive '
                    "FROM input"
                ),
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=2,
            expected_model_statuses=(("validated_orders", ExecutionStatus.SUCCESS),),
            expected_query_results=(
                (
                    "SELECT value, is_positive FROM main.validated_orders ORDER BY value DESC",
                    (("abc", False), ("123", True)),
                ),
                (
                    "SELECT node_name FROM main._sqlbuild_fingerprints "
                    "WHERE node_name = 'is_positive_int'",
                    (("is_positive_int",),),
                ),
            ),
        ),
        BuildExecutionTestCase(
            description="python udf builds before dependent model",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "functions/python/is_positive_int.py": (
                    "from sqlbuild.functions import udf\n\n"
                    "@udf(\n"
                    "    arguments={'a_string': 'VARCHAR'},\n"
                    "    returns='BOOLEAN',\n"
                    "    runtime_version='3.11',\n"
                    ")\n"
                    "def main(a_string):\n"
                    "    '''Test function main.'''\n    return bool(a_string and a_string.isdigit())\n"
                ),
                "models/validated_orders.sql": (
                    "MODEL (description 'Test model validated_orders.', materialized table);\n\n"
                    "WITH input(value) AS (VALUES ('123'), ('abc'))\n"
                    'SELECT input.value, __udf("is_positive_int")(input.value) AS is_positive '
                    "FROM input"
                ),
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=2,
            expected_model_statuses=(("validated_orders", ExecutionStatus.SUCCESS),),
            expected_query_results=(
                (
                    "SELECT value, is_positive FROM main.validated_orders ORDER BY value DESC",
                    (("abc", False), ("123", True)),
                ),
            ),
        ),
        BuildExecutionTestCase(
            description="duckdb python udf ignores inherited environment schema",
            project_files={
                "sqlbuild_project.toml": (
                    'name = "demo"\n'
                    'adapter = "duckdb"\n'
                    'default_target = "dev"\n\n'
                    "[connection]\n"
                    'database = ":memory:"\n\n'
                    "[targets.dev]\n"
                    'schema = "dev"\n'
                ),
                "functions/python/is_positive_int.py": (
                    "from sqlbuild.functions import udf\n\n"
                    "@udf(\n"
                    "    arguments={'a_string': 'VARCHAR'},\n"
                    "    returns='BOOLEAN',\n"
                    "    runtime_version='3.11',\n"
                    ")\n"
                    "def main(a_string):\n"
                    "    '''Test function main.'''\n    return bool(a_string and a_string.isdigit())\n"
                ),
                "models/validated_orders.sql": (
                    "MODEL (description 'Test model validated_orders.', materialized table);\n\n"
                    "WITH input(value) AS (VALUES ('123'), ('abc'))\n"
                    'SELECT input.value, __udf("is_positive_int")(input.value) AS is_positive '
                    "FROM input"
                ),
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=2,
            expected_model_statuses=(("validated_orders", ExecutionStatus.SUCCESS),),
            expected_function_statuses=(("is_positive_int", ExecutionStatus.SUCCESS),),
            expected_query_results=(
                (
                    "SELECT value, is_positive FROM dev.validated_orders ORDER BY value DESC",
                    (("abc", False), ("123", True)),
                ),
                (
                    "SELECT node_name FROM dev._sqlbuild_fingerprints "
                    "WHERE node_name = 'is_positive_int'",
                    (("is_positive_int",),),
                ),
            ),
        ),
        BuildExecutionTestCase(
            description="two independent models both succeed",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/orders.sql": "MODEL (description 'Test model orders.', materialized table);\n\nSELECT 1 AS id, 'alice' AS name",
                "models/payments.sql": (
                    "MODEL (description 'Test model payments.', materialized table);\n\nSELECT 10 AS payment_id, 500 AS amount"
                ),
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=2,
            expected_model_statuses=(
                ("orders", ExecutionStatus.SUCCESS),
                ("payments", ExecutionStatus.SUCCESS),
            ),
            expected_query_results=(
                ("SELECT id, name FROM main.orders", ((1, "alice"),)),
                ("SELECT payment_id, amount FROM main.payments", ((10, 500),)),
            ),
        ),
        BuildExecutionTestCase(
            description="dependent models execute in topo order and downstream reads upstream",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/stg_orders.sql": (
                    "MODEL (description 'Test model stg_orders.', materialized table);\n\nSELECT 42 AS id, 'bob' AS name"
                ),
                "models/orders.sql": (
                    "MODEL (description 'Test model orders.', materialized table);\n\n"
                    'SELECT id, name FROM __ref("stg_orders") WHERE id = 42'
                ),
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=2,
            expected_model_statuses=(
                ("stg_orders", ExecutionStatus.SUCCESS),
                ("orders", ExecutionStatus.SUCCESS),
            ),
            expected_query_results=(
                ("SELECT id, name FROM main.stg_orders", ((42, "bob"),)),
                ("SELECT id, name FROM main.orders", ((42, "bob"),)),
            ),
        ),
        BuildExecutionTestCase(
            description="environment schema is auto-created during build",
            project_files={
                "sqlbuild_project.toml": (
                    'name = "demo"\n'
                    'adapter = "duckdb"\n'
                    'default_target = "dev"\n\n'
                    "[connection]\n"
                    'database = ":memory:"\n\n'
                    "[targets.dev]\n"
                    'schema = "dev_schema"\n'
                ),
                "sqlbuild_local.toml": 'target = "dev"\n',
                "models/orders.sql": "MODEL (description 'Test model orders.', materialized table);\n\nSELECT 1 AS id, 'alice' AS name",
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=1,
            expected_model_statuses=(("orders", ExecutionStatus.SUCCESS),),
            expected_query_results=(("SELECT id, name FROM dev_schema.orders", ((1, "alice"),)),),
        ),
        BuildExecutionTestCase(
            description="run_audits false skips all audits but tables still built",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/orders.sql": _TABLE_WITH_ID_NOT_NULL_AUDIT + "SELECT 1 AS id",
                "audits/generic/not_null.sql": _NOT_NULL_AUDIT,
            },
            run_audits=False,
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=1,
            expected_model_audit_count=0,
            expected_query_results=(("SELECT id FROM main.orders", ((1,),)),),
        ),
        BuildExecutionTestCase(
            description="passing model audit does not block and table is promoted",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/orders.sql": _TABLE_WITH_ID_NOT_NULL_AUDIT + "SELECT 1 AS id",
                "audits/generic/not_null.sql": _NOT_NULL_AUDIT,
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=1,
            expected_model_audit_count=1,
            expected_query_results=(("SELECT id FROM main.orders", ((1,),)),),
        ),
        BuildExecutionTestCase(
            description="built-in not null audit does not require project generic definition",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/orders.sql": _TABLE_WITH_ID_NOT_NULL_AUDIT + "SELECT 1 AS id",
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=1,
            expected_model_audit_count=1,
            expected_query_results=(("SELECT id FROM main.orders", ((1,),)),),
        ),
        BuildExecutionTestCase(
            description="built-in unique audit ignores duplicate null values",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/orders.sql": (
                    "MODEL (description 'Test model orders.', materialized table, columns (id (audits [unique])));\n\n"
                    "WITH input(id) AS (VALUES (1), (NULL), (NULL))\nSELECT input.id FROM input"
                ),
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=1,
            expected_model_audit_count=1,
            expected_query_results=(
                ("SELECT id FROM main.orders ORDER BY id NULLS LAST", ((1,), (None,), (None,))),
            ),
        ),
        BuildExecutionTestCase(
            description="built-in accepted values audit ignores nulls and allows listed values",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/orders.sql": (
                    "MODEL (description 'Test model orders.', materialized table, columns (status (audits [accepted_values "
                    '(values ["placed", "completed"])])));\n\n'
                    "WITH input(status) AS (VALUES ('placed'), ('completed'), (NULL))\nSELECT input.status FROM input"
                ),
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=1,
            expected_model_audit_count=1,
            expected_query_results=(
                (
                    "SELECT status FROM main.orders ORDER BY status NULLS LAST",
                    (("completed",), ("placed",), (None,)),
                ),
            ),
        ),
        BuildExecutionTestCase(
            description="built-in relationships audit allows matching referenced values",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/customers.sql": "MODEL (description 'Test model customers.', materialized table);\n\nSELECT 1 AS id",
                "models/orders.sql": (
                    "MODEL (description 'Test model orders.', materialized table, columns (customer_id (audits [relationships "
                    '(to __ref("customers"), field id)])));\n\n'
                    'SELECT id AS customer_id FROM __ref("customers") UNION ALL SELECT NULL'
                ),
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=2,
            expected_model_audit_count=1,
            expected_query_results=(
                (
                    "SELECT customer_id FROM main.orders ORDER BY customer_id NULLS LAST",
                    ((1,), (None,)),
                ),
            ),
        ),
        BuildExecutionTestCase(
            description="warn audit records warning but build succeeds",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML_WARN,
                "models/orders.sql": _TABLE_WITH_ID_NOT_NULL_AUDIT + "SELECT NULL AS id",
                "audits/generic/not_null.sql": _NOT_NULL_AUDIT,
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=1,
            expected_model_audit_count=1,
            expected_warning_count=1,
            expected_query_results=(("SELECT id FROM main.orders", ((None,),)),),
        ),
        BuildExecutionTestCase(
            description="source audit warn does not block dependent models",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML_WARN,
                "models/orders.sql": (
                    'MODEL (description "Test model orders.", materialized table);\n\nSELECT id FROM __source("raw_orders")'
                ),
                "sources/raw.yml": _RAW_ORDERS_WITH_NOT_NULL_AUDIT,
            },
            setup_sql=("CREATE TABLE main.raw_orders AS SELECT NULL AS id",),
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=1,
            expected_source_audit_count=1,
            expected_warning_count=1,
            expected_query_results=(("SELECT id FROM main.orders", ((None,),)),),
        ),
        BuildExecutionTestCase(
            description="end audit warn succeeds build with warning",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML_WARN,
                "models/orders.sql": (
                    "MODEL (description 'Test model orders.', materialized table);\n\nSELECT 1 AS id"
                ),
                "models/payments.sql": (
                    "MODEL (description 'Test model payments.', materialized table);\n\nSELECT 2 AS payment_id"
                ),
                "audits/singular/cross_check.sql": (
                    'AUDIT ();\n\nSELECT o.id FROM __ref("orders") o CROSS JOIN __ref("payments") p'
                ),
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=2,
            expected_end_audit_count=1,
            expected_warning_count=1,
            expected_query_results=(
                ("SELECT id FROM main.orders", ((1,),)),
                ("SELECT payment_id FROM main.payments", ((2,),)),
            ),
        ),
        BuildExecutionTestCase(
            description="run_audits false skips source model and end audits but tables still built",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/orders.sql": (
                    'MODEL (description "Test model orders.", materialized table);\n\nSELECT id FROM __source("raw_orders")'
                ),
                "models/payments.sql": (
                    "MODEL (description 'Test model payments.', materialized table);\n\nSELECT 1 AS payment_id"
                ),
                "sources/raw.yml": _RAW_ORDERS_WITH_NOT_NULL_AUDIT,
                "audits/singular/cross_check.sql": (
                    'AUDIT ();\n\nSELECT o.id FROM __ref("orders") o CROSS JOIN __ref("payments") p'
                ),
            },
            setup_sql=("CREATE TABLE main.raw_orders AS SELECT NULL AS id",),
            run_audits=False,
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=2,
            expected_model_audit_count=0,
            expected_source_audit_count=0,
            expected_end_audit_count=0,
            expected_query_results=(
                ("SELECT id FROM main.orders", ((None,),)),
                ("SELECT payment_id FROM main.payments", ((1,),)),
            ),
        ),
        BuildExecutionTestCase(
            description="immediate promotion warn audit records warning and build succeeds",
            project_files={
                "sqlbuild_project.toml": (
                    'name = "demo"\n'
                    'adapter = "duckdb"\n\n'
                    "[connection]\n"
                    'database = ":memory:"\n\n'
                    "[settings]\n"
                    'default_audit_severity = "warn"\n'
                    'table_promotion_mode = "immediate"\n'
                ),
                "models/orders.sql": _TABLE_WITH_ID_NOT_NULL_AUDIT + "SELECT NULL AS id",
                "audits/generic/not_null.sql": _NOT_NULL_AUDIT,
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=1,
            expected_model_audit_count=1,
            expected_warning_count=1,
            expected_query_results=(("SELECT id FROM main.orders", ((None,),)),),
        ),
        BuildExecutionTestCase(
            description="build writes fingerprints when query tracking is enabled",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/orders.sql": "MODEL (description 'Test model orders.', materialized table);\n\nSELECT 1 AS id",
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=1,
            expected_query_results=(
                ("SELECT id FROM main.orders", ((1,),)),
                (
                    "SELECT node_name FROM main._sqlbuild_fingerprints ORDER BY node_name",
                    (("orders",),),
                ),
            ),
        ),
        BuildExecutionTestCase(
            description="immediate promotion with passing audit creates table",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML_IMMEDIATE,
                "models/orders.sql": _TABLE_WITH_ID_NOT_NULL_AUDIT + "SELECT 5 AS id",
                "audits/generic/not_null.sql": _NOT_NULL_AUDIT,
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=1,
            expected_model_audit_count=1,
            expected_query_results=(("SELECT id FROM main.orders", ((5,),)),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_build_plan_when_executing_then_succeeds(
    test_case: BuildExecutionTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    result: BuildExecutionResult = run_build_for_project(
        test_case=test_case, project_dir=tmp_path, adapter=adapter, connection=connection
    )

    assert result.status == test_case.expected_status
    assert result.success_count == test_case.expected_success_count
    assert result.failure_count == test_case.expected_failure_count
    assert result.skipped_count == test_case.expected_skipped_count
    verify_model_statuses(result=result, test_case=test_case)
    verify_function_statuses(result=result, test_case=test_case)
    verify_audit_counts(result=result, test_case=test_case)
    verify_warehouse_state(connection=connection, test_case=test_case)


@pytest.mark.parametrize(
    "test_case",
    [
        BuildExecutionTestCase(
            description="view model creates view and downstream table reads it",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/stg_orders.sql": (
                    "MODEL (description 'Test model stg_orders.', materialized view);\n\nSELECT 1 AS id, 'alice' AS name"
                ),
                "models/dim_orders.sql": (
                    'MODEL (description "Test model dim_orders.", materialized table);\n\nSELECT id, name FROM __ref("stg_orders")'
                ),
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=2,
            expected_model_statuses=(
                ("stg_orders", ExecutionStatus.SUCCESS),
                ("dim_orders", ExecutionStatus.SUCCESS),
            ),
            expected_query_results=(("SELECT id, name FROM main.dim_orders", ((1, "alice"),)),),
        ),
        BuildExecutionTestCase(
            description="view with audit succeeds when audit passes",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/stg_orders.sql": _VIEW_WITH_ID_NOT_NULL_AUDIT
                + "SELECT 1 AS id, 'alice' AS name",
                "audits/generic/not_null.sql": _NOT_NULL_AUDIT,
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=1,
            expected_model_audit_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_view_build_plan_when_executing_then_succeeds(
    test_case: BuildExecutionTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    result: BuildExecutionResult = run_build_for_project(
        test_case=test_case, project_dir=tmp_path, adapter=adapter, connection=connection
    )

    assert result.status == test_case.expected_status
    assert result.success_count == test_case.expected_success_count
    verify_model_statuses(result=result, test_case=test_case)
    verify_function_statuses(result=result, test_case=test_case)
    verify_audit_counts(result=result, test_case=test_case)
    verify_warehouse_state(connection=connection, test_case=test_case)


@pytest.mark.parametrize(
    "test_case",
    [
        BuildExecutionTestCase(
            description="view failure blocks downstream table",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/bad_view.sql": (
                    "MODEL (description 'Test model bad_view.', materialized view);\n\nSELECT * FROM nonexistent_source_table"
                ),
                "models/downstream.sql": (
                    'MODEL (description "Test model downstream.", materialized table);\n\nSELECT * FROM __ref("bad_view")'
                ),
            },
            expected_status=BuildStatus.FAILED,
            expected_failure_count=1,
            expected_skipped_count=1,
            expected_model_statuses=(
                ("bad_view", ExecutionStatus.FAILED),
                ("downstream", ExecutionStatus.SKIPPED),
            ),
            expected_missing_relations=("main.downstream",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_view_build_plan_when_executing_then_fails(
    test_case: BuildExecutionTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    result: BuildExecutionResult = run_build_for_project(
        test_case=test_case, project_dir=tmp_path, adapter=adapter, connection=connection
    )

    assert result.status == test_case.expected_status
    assert result.failure_count == test_case.expected_failure_count
    assert result.skipped_count == test_case.expected_skipped_count
    verify_model_statuses(result=result, test_case=test_case)
    verify_warehouse_state(connection=connection, test_case=test_case)


@pytest.mark.parametrize(
    "test_case",
    [
        BuildExecutionTestCase(
            description="view with run_audits false still creates view",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/stg_orders.sql": _VIEW_WITH_ID_NOT_NULL_AUDIT
                + "SELECT 1 AS id, 'alice' AS name",
                "audits/generic/not_null.sql": _NOT_NULL_AUDIT,
            },
            run_audits=False,
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=1,
            expected_model_audit_count=0,
            expected_query_results=(("SELECT id, name FROM main.stg_orders", ((1, "alice"),)),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_view_with_run_audits_false_when_executing_then_succeeds(
    test_case: BuildExecutionTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    result: BuildExecutionResult = run_build_for_project(
        test_case=test_case, project_dir=tmp_path, adapter=adapter, connection=connection
    )

    assert result.status == test_case.expected_status
    assert result.success_count == test_case.expected_success_count
    verify_audit_counts(result=result, test_case=test_case)
    verify_warehouse_state(connection=connection, test_case=test_case)


@pytest.mark.parametrize(
    "test_case",
    [
        BuildExecutionTestCase(
            description="model failure blocks downstream and blocked table does not exist",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/stg_orders.sql": (
                    "MODEL (description 'Test model stg_orders.', materialized table);\n\nSELECT * FROM nonexistent_table"
                ),
                "models/orders.sql": (
                    'MODEL (description "Test model orders.", materialized table);\n\nSELECT id FROM __ref("stg_orders")'
                ),
            },
            expected_status=BuildStatus.FAILED,
            expected_failure_count=1,
            expected_skipped_count=1,
            expected_model_statuses=(
                ("stg_orders", ExecutionStatus.FAILED),
                ("orders", ExecutionStatus.SKIPPED),
            ),
            expected_model_error_codes=(("stg_orders", "R002"),),
            expected_missing_relations=("main.stg_orders", "main.orders"),
        ),
        BuildExecutionTestCase(
            description="table runtime contract failure blocks promotion",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/orders.sql": (
                    "MODEL (description 'Test model orders.',\n"
                    "  materialized table,\n"
                    "  contract enforced,\n"
                    "  columns (id (type INTEGER)),\n"
                    ");\n\n"
                    "SELECT * FROM raw_orders"
                ),
            },
            setup_sql=("CREATE TABLE raw_orders AS SELECT 1 AS id, 'extra' AS status",),
            expected_status=BuildStatus.FAILED,
            expected_failure_count=1,
            expected_model_statuses=(("orders", ExecutionStatus.FAILED),),
            expected_model_error_fragments=(
                ("orders", "runtime contract has extra columns: status"),
            ),
            expected_model_error_codes=(("orders", "K009"),),
            expected_missing_relations=("main.orders",),
        ),
        BuildExecutionTestCase(
            description="snapshot runtime contract failure blocks target creation",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/customer_snapshot.sql": (
                    "MODEL (description 'Test model customer_snapshot.',\n"
                    "  materialized snapshot,\n"
                    "  contract enforced,\n"
                    "  columns (\n"
                    "    customer_id (type INTEGER),\n"
                    "    updated_at (type TIMESTAMP),\n"
                    "  ),\n"
                    "  unique_key [customer_id],\n"
                    "  snapshot_strategy timestamp,\n"
                    "  updated_at updated_at,\n"
                    ");\n\n"
                    "SELECT * FROM raw_customers"
                ),
            },
            setup_sql=(
                "CREATE TABLE raw_customers AS "
                "SELECT 1 AS customer_id, TIMESTAMP '2024-01-01' AS updated_at, 'pro' AS plan",
            ),
            expected_status=BuildStatus.FAILED,
            expected_failure_count=1,
            expected_model_statuses=(("customer_snapshot", ExecutionStatus.FAILED),),
            expected_model_error_fragments=(
                ("customer_snapshot", "runtime contract has extra columns: plan"),
            ),
            expected_model_error_codes=(("customer_snapshot", "K009"),),
            expected_missing_relations=("main.customer_snapshot",),
        ),
        BuildExecutionTestCase(
            description="incremental runtime contract failure blocks dml",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/orders.sql": (
                    "MODEL (description 'Test model orders.',\n"
                    "  materialized incremental,\n"
                    "  contract enforced,\n"
                    "  columns (\n"
                    "    id (type INTEGER),\n"
                    "    updated_at (type TIMESTAMP),\n"
                    "  ),\n"
                    "  incremental_strategy delete_insert,\n"
                    "  cursor updated_at,\n"
                    "  cursor_type timestamp,\n"
                    "  cursor_grain second,\n"
                    ");\n\n"
                    'SELECT * FROM __source("raw_orders")'
                ),
                "sources/raw.yml": (
                    "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
                    "    schema: main\n    table: raw_orders\n"
                ),
            },
            setup_sql=(
                "CREATE TABLE main.orders AS SELECT 1 AS id, TIMESTAMP '2024-01-01' AS updated_at",
                "CREATE TABLE raw_orders AS "
                "SELECT 2 AS id, TIMESTAMP '2024-01-02' AS updated_at, 'extra' AS status",
            ),
            expected_status=BuildStatus.FAILED,
            expected_failure_count=1,
            expected_model_statuses=(("orders", ExecutionStatus.FAILED),),
            expected_model_error_fragments=(
                ("orders", "runtime contract has extra columns: status"),
            ),
            expected_model_error_codes=(("orders", "K009"),),
            expected_query_results=(
                (
                    "SELECT id, CAST(updated_at AS VARCHAR) FROM main.orders",
                    ((1, "2024-01-01 00:00:00"),),
                ),
            ),
        ),
        BuildExecutionTestCase(
            description="duckdb python udf with explicit schema fails clearly",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "functions/python/is_positive_int.py": (
                    "from sqlbuild.functions import udf\n\n"
                    "@udf(\n"
                    "    arguments={'a_string': 'VARCHAR'},\n"
                    "    returns='BOOLEAN',\n"
                    "    runtime_version='3.11',\n"
                    "    schema='udfs',\n"
                    ")\n"
                    "def main(a_string):\n"
                    "    '''Test function main.'''\n    return bool(a_string and a_string.isdigit())\n"
                ),
                "models/validated_orders.sql": (
                    "MODEL (description 'Test model validated_orders.', materialized table);\n\n"
                    'SELECT __udf("is_positive_int")('
                    "'123'"
                    ") AS is_positive"
                ),
            },
            expected_status=BuildStatus.FAILED,
            expected_failure_count=1,
            expected_skipped_count=1,
            expected_function_statuses=(("is_positive_int", ExecutionStatus.FAILED),),
            expected_function_error_fragments=(
                (
                    "is_positive_int",
                    "DuckDB Python UDF 'is_positive_int' cannot set database or schema",
                ),
            ),
            expected_function_error_codes=(("is_positive_int", "F004"),),
            expected_model_statuses=(("validated_orders", ExecutionStatus.SKIPPED),),
            expected_missing_relations=("main.validated_orders",),
        ),
        BuildExecutionTestCase(
            description="independent branch succeeds with real data despite sibling failure",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/stg_orders.sql": (
                    "MODEL (description 'Test model stg_orders.', materialized table);\n\nSELECT * FROM nonexistent_table"
                ),
                "models/orders.sql": (
                    'MODEL (description "Test model orders.", materialized table);\n\nSELECT id FROM __ref("stg_orders")'
                ),
                "models/payments.sql": (
                    "MODEL (description 'Test model payments.', materialized table);\n\nSELECT 99 AS payment_id, 750 AS amount"
                ),
            },
            expected_status=BuildStatus.FAILED,
            expected_failure_count=1,
            expected_skipped_count=1,
            expected_success_count=1,
            expected_model_statuses=(
                ("stg_orders", ExecutionStatus.FAILED),
                ("orders", ExecutionStatus.SKIPPED),
                ("payments", ExecutionStatus.SUCCESS),
            ),
            expected_query_results=(
                ("SELECT payment_id, amount FROM main.payments", ((99, 750),)),
            ),
            expected_missing_relations=("main.stg_orders", "main.orders"),
        ),
        BuildExecutionTestCase(
            description="failing error audit blocks promotion and table has no final data",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/orders.sql": _TABLE_WITH_ID_NOT_NULL_AUDIT + "SELECT NULL AS id",
                "audits/generic/not_null.sql": _NOT_NULL_AUDIT,
            },
            expected_status=BuildStatus.FAILED,
            expected_failure_count=1,
            expected_model_audit_count=1,
            expected_model_statuses=(("orders", ExecutionStatus.FAILED),),
            expected_missing_relations=("main.orders",),
        ),
        BuildExecutionTestCase(
            description="built-in unique audit blocks duplicate non-null values",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/orders.sql": (
                    "MODEL (description 'Test model orders.', materialized table, columns (id (audits [unique])));\n\n"
                    "WITH input(id) AS (VALUES (1), (1), (NULL))\nSELECT input.id FROM input"
                ),
            },
            expected_status=BuildStatus.FAILED,
            expected_failure_count=1,
            expected_model_audit_count=1,
            expected_model_statuses=(("orders", ExecutionStatus.FAILED),),
            expected_missing_relations=("main.orders",),
        ),
        BuildExecutionTestCase(
            description="built-in accepted values audit blocks unlisted values",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/orders.sql": (
                    "MODEL (description 'Test model orders.', materialized table, columns (status (audits [accepted_values "
                    '(values ["placed", "completed"])])));\n\n'
                    "SELECT 'cancelled' AS status"
                ),
            },
            expected_status=BuildStatus.FAILED,
            expected_failure_count=1,
            expected_model_audit_count=1,
            expected_model_statuses=(("orders", ExecutionStatus.FAILED),),
            expected_missing_relations=("main.orders",),
        ),
        BuildExecutionTestCase(
            description="built-in relationships audit blocks missing referenced values",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/customers.sql": "MODEL (description 'Test model customers.', materialized table);\n\nSELECT 1 AS id",
                "models/orders.sql": (
                    "MODEL (description 'Test model orders.', materialized table, columns (customer_id (audits [relationships "
                    '(to __ref("customers"), field id)])));\n\n'
                    'SELECT 2 AS customer_id FROM __ref("customers")'
                ),
            },
            expected_status=BuildStatus.FAILED,
            expected_success_count=1,
            expected_failure_count=1,
            expected_model_audit_count=1,
            expected_model_statuses=(
                ("customers", ExecutionStatus.SUCCESS),
                ("orders", ExecutionStatus.FAILED),
            ),
            expected_missing_relations=("main.orders",),
        ),
        BuildExecutionTestCase(
            description="end audit error fails build but completed tables are preserved",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/orders.sql": (
                    "MODEL (description 'Test model orders.', materialized table);\n\nSELECT 7 AS id, 'carol' AS name"
                ),
                "models/payments.sql": (
                    "MODEL (description 'Test model payments.', materialized table);\n\nSELECT 1 AS payment_id"
                ),
                "audits/singular/cross_check.sql": (
                    'AUDIT ();\n\nSELECT o.id FROM __ref("orders") o CROSS JOIN __ref("payments") p'
                ),
            },
            expected_status=BuildStatus.FAILED,
            expected_success_count=2,
            expected_end_audit_count=1,
            expected_query_results=(
                ("SELECT id, name FROM main.orders", ((7, "carol"),)),
                ("SELECT payment_id FROM main.payments", ((1,),)),
            ),
        ),
        BuildExecutionTestCase(
            description="fail_fast stops after first failure and second model never materialized",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/aaa_broken.sql": (
                    "MODEL (description 'Test model aaa_broken.', materialized table);\n\nSELECT * FROM nonexistent_table"
                ),
                "models/zzz_healthy.sql": (
                    "MODEL (description 'Test model zzz_healthy.', materialized table);\n\nSELECT 1 AS id"
                ),
            },
            fail_fast=True,
            expected_status=BuildStatus.FAILED,
            expected_failure_count=1,
            expected_success_count=0,
            expected_skipped_count=1,
            expected_missing_relations=("main.aaa_broken", "main.zzz_healthy"),
        ),
        BuildExecutionTestCase(
            description="source audit error blocks dependent models and tables do not exist",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/orders.sql": (
                    'MODEL (description "Test model orders.", materialized table);\n\nSELECT id FROM __source("raw_orders")'
                ),
                "sources/raw.yml": _RAW_ORDERS_WITH_NOT_NULL_AUDIT,
            },
            setup_sql=("CREATE TABLE main.raw_orders AS SELECT NULL AS id",),
            expected_status=BuildStatus.FAILED,
            expected_failure_count=1,
            expected_skipped_count=1,
            expected_source_audit_count=1,
            expected_model_statuses=(("orders", ExecutionStatus.SKIPPED),),
            expected_missing_relations=("main.orders",),
        ),
        BuildExecutionTestCase(
            description="source audit error transitively blocks entire dependent chain",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/stg_orders.sql": (
                    'MODEL (description "Test model stg_orders.", materialized table);\n\nSELECT id FROM __source("raw_orders")'
                ),
                "models/orders.sql": (
                    'MODEL (description "Test model orders.", materialized table);\n\nSELECT id FROM __ref("stg_orders")'
                ),
                "sources/raw.yml": _RAW_ORDERS_WITH_NOT_NULL_AUDIT,
            },
            setup_sql=("CREATE TABLE main.raw_orders AS SELECT NULL AS id",),
            expected_status=BuildStatus.FAILED,
            expected_failure_count=1,
            expected_skipped_count=2,
            expected_source_audit_count=1,
            expected_model_statuses=(
                ("stg_orders", ExecutionStatus.SKIPPED),
                ("orders", ExecutionStatus.SKIPPED),
            ),
            expected_missing_relations=("main.stg_orders", "main.orders"),
        ),
        BuildExecutionTestCase(
            description="immediate promotion failing audit leaves target updated but build fails",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML_IMMEDIATE,
                "models/orders.sql": _TABLE_WITH_ID_NOT_NULL_AUDIT + "SELECT NULL AS id",
                "audits/generic/not_null.sql": _NOT_NULL_AUDIT,
            },
            expected_status=BuildStatus.FAILED,
            expected_failure_count=1,
            expected_model_audit_count=1,
            expected_model_statuses=(("orders", ExecutionStatus.FAILED),),
            expected_query_results=(("SELECT id FROM main.orders", ((None,),)),),
        ),
        BuildExecutionTestCase(
            description="pre_hook failure blocks model and downstream table does not exist",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/stg_orders.sql": (
                    "MODEL (description 'Test model stg_orders.',\n  materialized table\n"
                    "  pre_hooks [inline_sql('SELECT * FROM missing_hook_table')]\n);\n\n"
                    "SELECT 1 AS id"
                ),
                "models/orders.sql": (
                    'MODEL (description "Test model orders.", materialized table);\n\nSELECT id FROM __ref("stg_orders")'
                ),
            },
            expected_status=BuildStatus.FAILED,
            expected_failure_count=1,
            expected_skipped_count=1,
            expected_model_statuses=(
                ("stg_orders", ExecutionStatus.FAILED),
                ("orders", ExecutionStatus.SKIPPED),
            ),
            expected_missing_relations=("main.stg_orders", "main.orders"),
        ),
        BuildExecutionTestCase(
            description="post_hook failure blocks downstream but failed model table exists",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/stg_orders.sql": (
                    "MODEL (description 'Test model stg_orders.',\n  materialized table\n"
                    "  post_hooks [inline_sql('SELECT * FROM missing_hook_table')]\n);\n\n"
                    "SELECT 88 AS id"
                ),
                "models/orders.sql": (
                    'MODEL (description "Test model orders.", materialized table);\n\nSELECT id FROM __ref("stg_orders")'
                ),
            },
            expected_status=BuildStatus.FAILED,
            expected_failure_count=1,
            expected_skipped_count=1,
            expected_model_statuses=(
                ("stg_orders", ExecutionStatus.FAILED),
                ("orders", ExecutionStatus.SKIPPED),
            ),
            expected_query_results=(("SELECT id FROM main.stg_orders", ((88,),)),),
            expected_missing_relations=("main.orders",),
        ),
        BuildExecutionTestCase(
            description="two independent failures both recorded in non fail_fast",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/broken_a.sql": (
                    "MODEL (description 'Test model broken_a.', materialized table);\n\nSELECT * FROM nonexistent_a"
                ),
                "models/broken_b.sql": (
                    "MODEL (description 'Test model broken_b.', materialized table);\n\nSELECT * FROM nonexistent_b"
                ),
            },
            expected_status=BuildStatus.FAILED,
            expected_failure_count=2,
            expected_success_count=0,
            expected_model_statuses=(
                ("broken_a", ExecutionStatus.FAILED),
                ("broken_b", ExecutionStatus.FAILED),
            ),
            expected_missing_relations=("main.broken_a", "main.broken_b"),
        ),
        BuildExecutionTestCase(
            description="staged audit failure preserves existing target with old data",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/orders.sql": _TABLE_WITH_ID_NOT_NULL_AUDIT + "SELECT NULL AS id",
                "audits/generic/not_null.sql": _NOT_NULL_AUDIT,
            },
            setup_sql=("CREATE TABLE main.orders AS SELECT 999 AS id",),
            expected_status=BuildStatus.FAILED,
            expected_failure_count=1,
            expected_model_audit_count=1,
            expected_model_statuses=(("orders", ExecutionStatus.FAILED),),
            expected_query_results=(("SELECT id FROM main.orders", ((999,),)),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_build_plan_when_executing_then_fails(
    test_case: BuildExecutionTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    result: BuildExecutionResult = run_build_for_project(
        test_case=test_case, project_dir=tmp_path, adapter=adapter, connection=connection
    )

    assert result.status == test_case.expected_status
    assert result.success_count == test_case.expected_success_count
    assert result.failure_count == test_case.expected_failure_count
    assert result.skipped_count == test_case.expected_skipped_count
    verify_model_statuses(result=result, test_case=test_case)
    verify_audit_counts(result=result, test_case=test_case)
    verify_warehouse_state(connection=connection, test_case=test_case)


@pytest.mark.parametrize(
    "test_case",
    [
        BuildExecutionTestCase(
            description="passing unit test allows model to materialize",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/stg_orders.sql": (
                    "MODEL (description 'Test model stg_orders.', materialized table);\n\nSELECT 1 AS id, 'alice' AS name"
                ),
                "tests/unit/test_stg_orders.sql": _PASSING_TEST_SQL,
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=2,
            expected_test_count=1,
            expected_model_statuses=(("stg_orders", ExecutionStatus.SUCCESS),),
            expected_query_results=(("SELECT id, name FROM main.stg_orders", ((1, "alice"),)),),
        ),
        BuildExecutionTestCase(
            description="failing unit test blocks model materialization",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/stg_orders.sql": (
                    "MODEL (description 'Test model stg_orders.', materialized table);\n\nSELECT 1 AS id, 'alice' AS name"
                ),
                "tests/unit/test_stg_orders.sql": _FAILING_TEST_SQL,
            },
            expected_status=BuildStatus.FAILED,
            expected_failure_count=1,
            expected_skipped_count=1,
            expected_test_count=1,
            expected_test_error_codes=(("test_stg_orders", "T003"),),
            expected_model_statuses=(("stg_orders", ExecutionStatus.SKIPPED),),
            expected_missing_relations=("main.stg_orders",),
        ),
        BuildExecutionTestCase(
            description="run_tests false skips test and model materializes",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/stg_orders.sql": (
                    "MODEL (description 'Test model stg_orders.', materialized table);\n\nSELECT 1 AS id, 'alice' AS name"
                ),
                "tests/unit/test_stg_orders.sql": _FAILING_TEST_SQL,
            },
            run_tests=False,
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=1,
            expected_test_count=0,
            expected_model_statuses=(("stg_orders", ExecutionStatus.SUCCESS),),
            expected_query_results=(("SELECT id, name FROM main.stg_orders", ((1, "alice"),)),),
        ),
        BuildExecutionTestCase(
            description="failing test blocks target model and downstream chain",
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "models/stg_orders.sql": (
                    "MODEL (description 'Test model stg_orders.', materialized table);\n\nSELECT 1 AS id, 'alice' AS name"
                ),
                "models/dim_orders.sql": (
                    'MODEL (description "Test model dim_orders.", materialized table);\n\nSELECT id, name FROM __ref("stg_orders")'
                ),
                "tests/unit/test_stg_orders.sql": _FAILING_TEST_SQL,
            },
            expected_status=BuildStatus.FAILED,
            expected_failure_count=1,
            expected_skipped_count=2,
            expected_test_count=1,
            expected_model_statuses=(
                ("stg_orders", ExecutionStatus.SKIPPED),
                ("dim_orders", ExecutionStatus.SKIPPED),
            ),
            expected_missing_relations=("main.stg_orders", "main.dim_orders"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sql_test_in_build_when_executing_then_matches_expected(
    test_case: BuildExecutionTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    result: BuildExecutionResult = run_build_for_project(
        test_case=test_case, project_dir=tmp_path, adapter=adapter, connection=connection
    )

    assert result.status == test_case.expected_status
    assert result.success_count == test_case.expected_success_count
    assert result.failure_count == test_case.expected_failure_count
    assert result.skipped_count == test_case.expected_skipped_count
    verify_test_counts(result=result, test_case=test_case)
    verify_model_statuses(result=result, test_case=test_case)
    verify_warehouse_state(connection=connection, test_case=test_case)
