"""Integration tests for single-model table execution lifecycle."""

from __future__ import annotations

from pathlib import Path
from typing import Any, ClassVar

import pytest

from sqlbuild._native import DiscoveredHookFunction, PythonHookEntry, SqlHookEntry
from sqlbuild.adapter.contract.types import TablePromotionMode
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.run.models import ModelExecutionResult
from sqlbuild.executor.run.types import ExecutionPhase
from tests.integration.src.sqlbuild.executor.run._test_types import (
    RebuiltRowCountTestCase,
    TableFailureTestCase,
    TableSuccessTestCase,
)
from tests.integration.src.sqlbuild.executor.run.helpers import (
    ExtraAuditDefinition,
    counted_rows,
    create_python_hook_data,
    fail_table_hook,
    insert_table_hook_log,
    run_failure_test,
    run_success_test,
    unreadable_rows,
    verify_failure_warehouse_state,
    verify_success_warehouse_state,
)


class ZeroCopyDuckDbAdapter(DuckDbAdapter):
    """DuckDB test adapter that exercises the cheap-reuse executor path."""

    adapter_name: ClassVar[str] = "duckdb_zero_copy_test"

    def supports_zero_copy_clone(self) -> bool:
        return True


@pytest.mark.parametrize(
    "test_case",
    [
        TableSuccessTestCase(
            description="staged table creates target from query",
            setup_sql=(),
            model_sql="SELECT 1 AS id, 'alice' AS name",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.STAGED,
            expected_row_count=1,
        ),
        TableSuccessTestCase(
            description="staged table replaces existing target via swap",
            setup_sql=("CREATE TABLE staging.orders AS SELECT 1 AS id, 'old' AS name",),
            model_sql="SELECT 2 AS id, 'new' AS name",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.STAGED,
            expected_row_count=1,
        ),
        TableSuccessTestCase(
            description="staged table with type enforcement casts columns",
            setup_sql=(),
            model_sql="SELECT 1 AS id, 'alice' AS name",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.STAGED,
            type_enforcement=True,
            declared_columns=(("id", "BIGINT"), ("name", "VARCHAR")),
            expected_row_count=1,
        ),
        TableSuccessTestCase(
            description="staged table with passing audit succeeds",
            setup_sql=(),
            model_sql="SELECT 1 AS id, 'alice' AS name",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.STAGED,
            audit_sql='SELECT id FROM __ref("orders") WHERE id IS NULL',
            audit_severity="error",
            expected_row_count=1,
            expected_audit_count=1,
        ),
        TableSuccessTestCase(
            description="staged table with failing warn audit allows promotion",
            setup_sql=(),
            model_sql="SELECT NULL AS id, 'alice' AS name",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.STAGED,
            audit_sql='SELECT id FROM __ref("orders") WHERE id IS NULL',
            audit_severity="warn",
            expected_row_count=1,
            expected_audit_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_staged_table_when_executing_then_succeeds(
    test_case: TableSuccessTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    result: ModelExecutionResult = run_success_test(
        test_case=test_case, adapter=adapter, connection=connection
    )

    assert len(result.audit_results) == test_case.expected_audit_count
    verify_success_warehouse_state(
        result=result, test_case=test_case, adapter=adapter, connection=connection
    )


@pytest.mark.parametrize(
    "test_case",
    [
        TableFailureTestCase(
            description="staged table with failing error audit blocks promotion",
            setup_sql=(),
            model_sql="SELECT NULL AS id, 'alice' AS name",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.STAGED,
            audit_sql='SELECT id FROM __ref("orders") WHERE id IS NULL',
            audit_severity="error",
            expected_failed_phase=ExecutionPhase.AUDIT,
            expected_error_fragment="final audit for 'orders' failed before replacing target table",
            expected_staging_relation="staging.orders__staging",
            expected_audit_count=1,
        ),
        TableFailureTestCase(
            description="staged CTAS failure on bad SQL returns STAGING phase error",
            setup_sql=(),
            model_sql="SELECT * FROM staging.nonexistent_table",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.STAGED,
            expected_failed_phase=ExecutionPhase.STAGING,
            expected_staging_relation="staging.orders__staging",
        ),
        TableFailureTestCase(
            description="multiple audits all run even when first errors",
            setup_sql=(),
            model_sql="SELECT NULL AS id, 'alice' AS name",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.STAGED,
            audit_sql='SELECT id FROM __ref("orders") WHERE id IS NULL',
            audit_severity="error",
            extra_audits=(
                ExtraAuditDefinition(
                    name="name_not_null",
                    audit_sql='SELECT name FROM __ref("orders") WHERE name IS NULL',
                    severity="error",
                ),
            ),
            expected_failed_phase=ExecutionPhase.AUDIT,
            expected_staging_relation="staging.orders__staging",
            expected_audit_count=2,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_staged_table_when_executing_then_fails(
    test_case: TableFailureTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    result: ModelExecutionResult = run_failure_test(
        test_case=test_case, adapter=adapter, connection=connection
    )

    assert result.failed_phase == test_case.expected_failed_phase
    verify_failure_warehouse_state(
        result=result, test_case=test_case, adapter=adapter, connection=connection
    )


@pytest.mark.parametrize(
    "test_case",
    [
        TableSuccessTestCase(
            description="immediate table creates target from query",
            setup_sql=(),
            model_sql="SELECT 1 AS id, 'alice' AS name",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.IMMEDIATE,
            expected_row_count=1,
        ),
        TableSuccessTestCase(
            description="immediate table with passing audit succeeds end-to-end",
            setup_sql=(),
            model_sql="SELECT 1 AS id, 'alice' AS name",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.IMMEDIATE,
            audit_sql='SELECT id FROM __ref("orders") WHERE id IS NULL',
            audit_severity="error",
            expected_row_count=1,
            expected_audit_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_immediate_table_when_executing_then_succeeds(
    test_case: TableSuccessTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    result: ModelExecutionResult = run_success_test(
        test_case=test_case, adapter=adapter, connection=connection
    )

    assert len(result.audit_results) == test_case.expected_audit_count
    verify_success_warehouse_state(
        result=result, test_case=test_case, adapter=adapter, connection=connection
    )


@pytest.mark.parametrize(
    "test_case",
    [
        TableFailureTestCase(
            description="immediate table with type enforcement fails",
            setup_sql=(),
            model_sql="SELECT 1 AS id",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.IMMEDIATE,
            type_enforcement=True,
            declared_columns=(("id", "BIGINT"),),
            expected_failed_phase=ExecutionPhase.TYPE_ENFORCEMENT,
            expected_error_fragment="requires staged promotion mode",
        ),
        TableFailureTestCase(
            description="immediate failing post_hook marks model failed with promoted relation",
            setup_sql=(),
            model_sql="SELECT 1 AS id",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.IMMEDIATE,
            post_hook=[SqlHookEntry(statement="THIS IS NOT VALID SQL")],
            expected_failed_phase=ExecutionPhase.POST_HOOK,
            expected_promoted_relation="staging.orders",
            expected_row_count=1,
        ),
        TableFailureTestCase(
            description="immediate failing error audit reports target already updated",
            setup_sql=(),
            model_sql="SELECT NULL AS id, 'alice' AS name",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.IMMEDIATE,
            audit_sql='SELECT id FROM __ref("orders") WHERE id IS NULL',
            audit_severity="error",
            expected_failed_phase=ExecutionPhase.AUDIT,
            expected_error_fragment="final audit for 'orders' failed after target table was replaced",
            expected_promoted_relation="staging.orders",
            expected_audit_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_immediate_table_when_executing_then_fails(
    test_case: TableFailureTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    result: ModelExecutionResult = run_failure_test(
        test_case=test_case, adapter=adapter, connection=connection
    )

    assert result.failed_phase == test_case.expected_failed_phase
    verify_failure_warehouse_state(
        result=result, test_case=test_case, adapter=adapter, connection=connection
    )


@pytest.mark.parametrize(
    "test_case",
    [
        TableSuccessTestCase(
            description="pre_hook runs before materialization",
            setup_sql=(),
            model_sql="SELECT * FROM staging.hook_data",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.STAGED,
            pre_hook=[SqlHookEntry(statement="CREATE TABLE staging.hook_data AS SELECT 42 AS val")],
            expected_row_count=1,
        ),
        TableSuccessTestCase(
            description="post_hook runs after promotion",
            setup_sql=(),
            model_sql="SELECT 1 AS id",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.STAGED,
            post_hook=[
                SqlHookEntry(statement="CREATE TABLE staging.post_hook_ran AS SELECT 1 AS marker")
            ],
            expected_row_count=1,
        ),
        TableSuccessTestCase(
            description="list pre_hook executes all entries in order",
            setup_sql=(),
            model_sql="SELECT * FROM staging.hook_step_2",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.STAGED,
            pre_hook=[
                SqlHookEntry(statement="CREATE TABLE staging.hook_step_1 AS SELECT 42 AS val"),
                SqlHookEntry(
                    statement="CREATE TABLE staging.hook_step_2 AS SELECT * FROM staging.hook_step_1"
                ),
            ],
            expected_row_count=1,
        ),
        TableSuccessTestCase(
            description="python pre_hook runs before table materialization",
            setup_sql=(),
            model_sql="SELECT * FROM staging.python_hook_data",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.STAGED,
            pre_hook=[PythonHookEntry(name="create_data", kwargs={"value": 42})],
            hook_functions=(
                DiscoveredHookFunction(
                    file_path=Path(__file__),
                    relative_path=Path("sql/hooks/python/table.py"),
                    name="create_data",
                    function=create_python_hook_data,
                ),
            ),
            expected_row_count=1,
            expected_lifecycle_event_fragments=(
                "CREATE TABLE staging.python_hook_data AS SELECT 42 AS val",
                "python pre-hook created data for orders",
            ),
        ),
        TableSuccessTestCase(
            description="mixed SQL and Python hooks execute around table materialization",
            setup_sql=("CREATE TABLE staging.hook_log (phase VARCHAR)",),
            model_sql="SELECT 1 AS id",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.STAGED,
            pre_hook=[
                SqlHookEntry(statement="INSERT INTO staging.hook_log VALUES ('sql_pre')"),
                PythonHookEntry(name="insert_hook_log", kwargs={"phase": "python_pre"}),
            ],
            post_hook=[
                PythonHookEntry(name="insert_hook_log", kwargs={"phase": "python_post"}),
                SqlHookEntry(statement="INSERT INTO staging.hook_log VALUES ('sql_post')"),
            ],
            hook_functions=(
                DiscoveredHookFunction(
                    file_path=Path(__file__),
                    relative_path=Path("sql/hooks/python/table.py"),
                    name="insert_hook_log",
                    function=insert_table_hook_log,
                ),
            ),
            expected_row_count=1,
            expected_query_results=(
                (
                    "SELECT phase FROM staging.hook_log ORDER BY rowid",
                    (("sql_pre",), ("python_pre",), ("python_post",), ("sql_post",)),
                ),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_hook_config_when_executing_table_then_succeeds(
    test_case: TableSuccessTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    result: ModelExecutionResult = run_success_test(
        test_case=test_case, adapter=adapter, connection=connection
    )

    assert len(result.audit_results) == test_case.expected_audit_count
    verify_success_warehouse_state(
        result=result, test_case=test_case, adapter=adapter, connection=connection
    )


@pytest.mark.parametrize(
    "test_case",
    [
        TableFailureTestCase(
            description="failing pre_hook blocks model",
            setup_sql=(),
            model_sql="SELECT 1 AS id",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.STAGED,
            pre_hook=[SqlHookEntry(statement="THIS IS NOT VALID SQL")],
            expected_failed_phase=ExecutionPhase.PRE_HOOK,
        ),
        TableFailureTestCase(
            description="failing post_hook marks model failed after promotion",
            setup_sql=(),
            model_sql="SELECT 1 AS id",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.STAGED,
            post_hook=[SqlHookEntry(statement="THIS IS NOT VALID SQL")],
            expected_failed_phase=ExecutionPhase.POST_HOOK,
            expected_promoted_relation="staging.orders",
            expected_row_count=1,
        ),
        TableFailureTestCase(
            description="python post_hook failure marks table failed after promotion",
            setup_sql=(),
            model_sql="SELECT 1 AS id",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.STAGED,
            post_hook=[PythonHookEntry(name="fail_hook", kwargs={"message": "table post failed"})],
            hook_functions=(
                DiscoveredHookFunction(
                    file_path=Path(__file__),
                    relative_path=Path("sql/hooks/python/table.py"),
                    name="fail_hook",
                    function=fail_table_hook,
                ),
            ),
            expected_failed_phase=ExecutionPhase.POST_HOOK,
            expected_error_fragment='post_hooks[0] python("fail_hook") failed: table post failed',
            expected_promoted_relation="staging.orders",
            expected_row_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_hook_config_when_executing_table_then_fails(
    test_case: TableFailureTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    result: ModelExecutionResult = run_failure_test(
        test_case=test_case, adapter=adapter, connection=connection
    )

    assert result.failed_phase == test_case.expected_failed_phase
    verify_failure_warehouse_state(
        result=result, test_case=test_case, adapter=adapter, connection=connection
    )


@pytest.mark.parametrize(
    "test_case",
    [
        TableFailureTestCase(
            description="staged audit failure retains staging relation",
            setup_sql=(),
            model_sql="SELECT NULL AS id",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.STAGED,
            audit_sql='SELECT id FROM __ref("orders") WHERE id IS NULL',
            audit_severity="error",
            expected_failed_phase=ExecutionPhase.AUDIT,
            expected_staging_relation="staging.orders__staging",
            expected_audit_count=1,
        ),
        TableFailureTestCase(
            description="staged audit failure preserves existing target unchanged",
            setup_sql=("CREATE TABLE staging.orders AS SELECT 99 AS id, 'original' AS name",),
            model_sql="SELECT NULL AS id, 'new' AS name",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.STAGED,
            audit_sql='SELECT id FROM __ref("orders") WHERE id IS NULL',
            audit_severity="error",
            expected_failed_phase=ExecutionPhase.AUDIT,
            expected_staging_relation="staging.orders__staging",
            expected_audit_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failure_condition_when_executing_table_then_cleanup_matches(
    test_case: TableFailureTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    result: ModelExecutionResult = run_failure_test(
        test_case=test_case, adapter=adapter, connection=connection
    )

    assert result.staging_relation == test_case.expected_staging_relation
    verify_failure_warehouse_state(
        result=result, test_case=test_case, adapter=adapter, connection=connection
    )


@pytest.mark.parametrize(
    "test_case",
    [
        TableSuccessTestCase(
            description="type enforcement casts subset of columns and preserves extras",
            setup_sql=(),
            model_sql="SELECT 1 AS id, 'alice' AS name, 42.5 AS score",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.STAGED,
            type_enforcement=True,
            declared_columns=(("id", "BIGINT"),),
            expected_row_count=1,
            expected_column_names=("id", "name", "score"),
            expected_column_types=("BIGINT",),
        ),
        TableSuccessTestCase(
            description="type enforcement preserves produced column order",
            setup_sql=(),
            model_sql="SELECT 'alice' AS name, 1 AS id, TRUE AS active",
            target_schema="staging",
            target_name="orders",
            promotion_mode=TablePromotionMode.STAGED,
            type_enforcement=True,
            declared_columns=(("id", "BIGINT"), ("name", "VARCHAR")),
            expected_row_count=1,
            expected_column_names=("name", "id", "active"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_type_enforcement_when_executing_staged_table_then_columns_match(
    test_case: TableSuccessTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    result: ModelExecutionResult = run_success_test(
        test_case=test_case, adapter=adapter, connection=connection
    )

    assert len(result.audit_results) == test_case.expected_audit_count
    verify_success_warehouse_state(
        result=result, test_case=test_case, adapter=adapter, connection=connection
    )


@pytest.mark.parametrize(
    "test_case",
    [
        TableSuccessTestCase(
            description="missing target schema warns when query tracking is enabled",
            setup_sql=(),
            model_sql="SELECT 1 AS id",
            target_schema=None,
            target_name="orders",
            promotion_mode=TablePromotionMode.STAGED,
            expected_row_count=1,
            expected_warning_fragment="target schema is missing",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_fingerprint_failure_when_executing_table_then_succeeds_with_warning(
    test_case: TableSuccessTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    result: ModelExecutionResult = run_success_test(
        test_case=test_case, adapter=adapter, connection=connection
    )

    assert len(result.audit_results) == test_case.expected_audit_count
    verify_success_warehouse_state(
        result=result, test_case=test_case, adapter=adapter, connection=connection
    )


@pytest.mark.parametrize(
    "test_case",
    [
        RebuiltRowCountTestCase(
            "a staged rebuild reports the adapter's count",
            TablePromotionMode.STAGED,
            counted_rows,
            7,
        ),
        RebuiltRowCountTestCase(
            "a staged rebuild whose count read fails still succeeds",
            TablePromotionMode.STAGED,
            unreadable_rows,
            None,
        ),
        RebuiltRowCountTestCase(
            "an immediate rebuild whose count read fails still succeeds",
            TablePromotionMode.IMMEDIATE,
            unreadable_rows,
            None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_rebuilt_table_when_its_row_count_read_fails_then_the_node_succeeds_without_a_count(
    test_case: RebuiltRowCountTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(adapter, "rebuilt_table_row_count", test_case.row_count)

    result: ModelExecutionResult = run_success_test(
        test_case=TableSuccessTestCase(
            description=test_case.description,
            setup_sql=(),
            model_sql="SELECT 1 AS id",
            target_schema="main",
            target_name="orders",
            promotion_mode=test_case.promotion_mode,
            expected_row_count=1,
        ),
        adapter=adapter,
        connection=connection,
    )

    assert result.table_rows == test_case.expected_table_rows
