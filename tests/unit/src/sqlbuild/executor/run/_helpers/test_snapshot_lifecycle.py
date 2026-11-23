"""Tests for snapshot execution lifecycle side effects."""

from __future__ import annotations

from pathlib import Path
from typing import Any, cast

import pytest

from sqlbuild._native import (
    FINGERPRINT_TABLE_NAME,
    NODE_TYPE_MODEL,
    DiscoveredHookFunction,
    ModelPlanEntry,
    PythonHookEntry,
    SnapshotsConfig,
    SqlHookEntry,
)
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.models import ColumnInfo
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.run._helpers.materializations.snapshot import (
    _apply_snapshot_schema_change,
    execute_snapshot_entry,
)
from sqlbuild.executor.run.models import ModelExecutionResult, ModelMaterializationContext
from sqlbuild.executor.run.types import ExecutionPhase
from sqlbuild.executor.scheduling.types import ExecutionStatus
from tests.unit.src.sqlbuild.executor.run._helpers._test_types import (
    SnapshotHookFailureTestCase,
    SnapshotLifecycleTestCase,
    SnapshotRuntimeContractErrorTestCase,
    SnapshotSchemaChangeTestCase,
)
from tests.unit.src.sqlbuild.executor.run._helpers.helpers import (
    build_snapshot_execution_plan_entry,
    fail_snapshot_hook,
    insert_snapshot_hook_log,
)


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotLifecycleTestCase(
            description="snapshot runs SQL and Python hooks and cleans delta",
            run_id="snapshot_lifecycle_run",
            pre_hook=(
                SqlHookEntry(statement="CREATE TABLE main.snapshot_hook_log (phase VARCHAR)"),
                SqlHookEntry(statement="INSERT INTO main.snapshot_hook_log VALUES ('sql_pre')"),
                PythonHookEntry(name="insert_hook_log", kwargs={"phase": "python_pre"}),
            ),
            post_hook=(
                PythonHookEntry(name="insert_hook_log", kwargs={"phase": "python_post"}),
                SqlHookEntry(statement="INSERT INTO main.snapshot_hook_log VALUES ('sql_post')"),
            ),
            expected_hook_events=(
                "CREATE TABLE main.snapshot_hook_log (phase VARCHAR)",
                "INSERT INTO main.snapshot_hook_log VALUES ('sql_pre')",
                "INSERT INTO main.snapshot_hook_log VALUES ('python_pre')",
                "INSERT INTO main.snapshot_hook_log VALUES ('python_post')",
                "INSERT INTO main.snapshot_hook_log VALUES ('sql_post')",
            ),
            expected_model_name="customer_snapshot",
            expected_target_name="customer_snapshot",
            hook_functions=(
                DiscoveredHookFunction(
                    file_path=Path(__file__),
                    relative_path=Path("sql/hooks/python/snapshot.py"),
                    name="insert_hook_log",
                    function=insert_snapshot_hook_log,
                ),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_successful_snapshot_when_executing_then_runs_lifecycle_side_effects(
    test_case: SnapshotLifecycleTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    entry: ModelPlanEntry = build_snapshot_execution_plan_entry(
        pre_hooks=test_case.pre_hook,
        post_hooks=test_case.post_hook,
    )
    assert all(
        isinstance(hook_function, DiscoveredHookFunction)
        for hook_function in test_case.hook_functions
    )

    result: ModelExecutionResult = execute_snapshot_entry(
        context=ModelMaterializationContext(
            entry=entry,
            adapter=adapter,
            connection=connection,
            model_locations={},
            seed_locations={},
            source_map={},
            python_source_read_map={},
            model_audits=(),
            run_id=test_case.run_id,
            query_change_tracking=True,
            hook_functions=cast(tuple[DiscoveredHookFunction, ...], test_case.hook_functions),
        ),
    )
    hook_rows: tuple[tuple[object, ...], ...] = tuple(
        tuple(row)
        for row in connection.execute("SELECT phase FROM main.snapshot_hook_log").fetchall()
    )
    fingerprint_rows: tuple[tuple[object, ...], ...] = tuple(
        tuple(row)
        for row in connection.execute(
            f"SELECT node_name, target_name, run_id FROM main.{FINGERPRINT_TABLE_NAME} "
            f"WHERE node_type = '{NODE_TYPE_MODEL}'"
        ).fetchall()
    )
    lifecycle_sql: tuple[str, ...] = tuple(event.content for event in result.lifecycle_events)

    assert result.status == ExecutionStatus.SUCCESS
    assert hook_rows == (("sql_pre",), ("python_pre",), ("python_post",), ("sql_post",))
    assert fingerprint_rows == (
        (test_case.expected_model_name, test_case.expected_target_name, test_case.run_id),
    )
    assert (
        adapter.relation_exists(
            connection=connection,
            database=None,
            schema="main",
            name="customer_snapshot__snapshot_delta",
        )
        is False
    )
    expected_hook_event: str
    for expected_hook_event in test_case.expected_hook_events:
        assert expected_hook_event in lifecycle_sql

    adapter.close(connection)


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotHookFailureTestCase(
            description="python pre hook failure",
            pre_hooks=(
                PythonHookEntry(name="fail_hook", kwargs={"message": "snapshot pre failed"}),
            ),
            post_hooks=None,
            expected_phase=ExecutionPhase.PRE_HOOK,
            expected_error_fragment='pre_hooks[0] python("fail_hook") failed: snapshot pre failed',
        ),
        SnapshotHookFailureTestCase(
            description="python post hook failure",
            pre_hooks=None,
            post_hooks=(
                PythonHookEntry(name="fail_hook", kwargs={"message": "snapshot post failed"}),
            ),
            expected_phase=ExecutionPhase.POST_HOOK,
            expected_error_fragment='post_hooks[0] python("fail_hook") failed: snapshot post failed',
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_snapshot_hook_failure_when_executing_then_reports_hook_phase(
    test_case: SnapshotHookFailureTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    entry: ModelPlanEntry = build_snapshot_execution_plan_entry(
        pre_hooks=test_case.pre_hooks,
        post_hooks=test_case.post_hooks,
    )

    result: ModelExecutionResult = execute_snapshot_entry(
        context=ModelMaterializationContext(
            entry=entry,
            adapter=adapter,
            connection=connection,
            model_locations={},
            seed_locations={},
            source_map={},
            python_source_read_map={},
            model_audits=(),
            run_id="snapshot_hook_failure_run",
            query_change_tracking=True,
            hook_functions=(
                DiscoveredHookFunction(
                    file_path=Path(__file__),
                    relative_path=Path("sql/hooks/python/snapshot.py"),
                    name="fail_hook",
                    function=fail_snapshot_hook,
                ),
            ),
        ),
    )

    assert result.status == ExecutionStatus.FAILED
    assert result.failed_phase == test_case.expected_phase
    assert result.error_message is not None
    assert test_case.expected_error_fragment in result.error_message

    adapter.close(connection)


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotSchemaChangeTestCase(
            description="snowflake timestamp alias is not a snapshot type change",
            target_columns=(ColumnInfo(name="updated_at", type="TIMESTAMP"),),
            delta_columns=(ColumnInfo(name="updated_at", type="TIMESTAMP_NTZ"),),
            expected_valid=True,
        ),
        SnapshotSchemaChangeTestCase(
            description="snowflake narrower varchar delta is not a snapshot type change",
            target_columns=(ColumnInfo(name="plan", type="VARCHAR(5)"),),
            delta_columns=(ColumnInfo(name="plan", type="VARCHAR(3)"),),
            expected_valid=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snapshot_schema_change_when_checking_compatible_snowflake_types_then_it_is_valid(
    test_case: SnapshotSchemaChangeTestCase,
) -> None:
    adapter: SnowflakeAdapter = SnowflakeAdapter()
    entry: ModelPlanEntry = build_snapshot_execution_plan_entry()

    _apply_snapshot_schema_change(
        adapter=adapter,
        connection=object(),
        entry=entry,
        snapshots=SnapshotsConfig(),
        target_qualified="analytics.customer_snapshot",
        target_columns=test_case.target_columns,
        delta_columns=test_case.delta_columns,
        allow_snapshot_schema_change=False,
        statement_recorder=StatementRecorder(),
    )

    assert test_case.expected_valid is True


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotSchemaChangeTestCase(
            description="snowflake timestamp time zone variant remains a type change",
            target_columns=(ColumnInfo(name="updated_at", type="TIMESTAMP_NTZ"),),
            delta_columns=(ColumnInfo(name="updated_at", type="TIMESTAMP_TZ"),),
            expected_valid=False,
            expected_error_fragment="type changes: updated_at",
        ),
        SnapshotSchemaChangeTestCase(
            description="snowflake wider varchar delta remains a type change",
            target_columns=(ColumnInfo(name="plan", type="VARCHAR(3)"),),
            delta_columns=(ColumnInfo(name="plan", type="VARCHAR(5)"),),
            expected_valid=False,
            expected_error_fragment="type changes: plan",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snapshot_schema_change_when_checking_incompatible_snowflake_types_then_it_raises(
    test_case: SnapshotSchemaChangeTestCase,
) -> None:
    adapter: SnowflakeAdapter = SnowflakeAdapter()
    entry: ModelPlanEntry = build_snapshot_execution_plan_entry()

    with pytest.raises(ExecutorInputError, match=test_case.expected_error_fragment):
        _apply_snapshot_schema_change(
            adapter=adapter,
            connection=object(),
            entry=entry,
            snapshots=SnapshotsConfig(),
            target_qualified="analytics.customer_snapshot",
            target_columns=test_case.target_columns,
            delta_columns=test_case.delta_columns,
            allow_snapshot_schema_change=False,
            statement_recorder=StatementRecorder(),
        )


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotRuntimeContractErrorTestCase(
            description="snapshot delta has extra runtime column",
            contract_columns=(
                ColumnInfo(name="customer_id", type="INTEGER"),
                ColumnInfo(name="updated_at", type="TIMESTAMP"),
            ),
            run_id="runtime_contract_failure",
            expected_error_fragment="runtime contract has extra columns: plan",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_snapshot_contract_violation_when_executing_then_fails_before_target_update(
    test_case: SnapshotRuntimeContractErrorTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    entry: ModelPlanEntry = build_snapshot_execution_plan_entry(
        contract_enforced=True,
        contract_columns=test_case.contract_columns,
    )

    result: ModelExecutionResult = execute_snapshot_entry(
        context=ModelMaterializationContext(
            entry=entry,
            adapter=adapter,
            connection=connection,
            model_locations={},
            seed_locations={},
            source_map={},
            python_source_read_map={},
            model_audits=(),
            run_id=test_case.run_id,
            query_change_tracking=True,
        ),
    )

    assert result.status == ExecutionStatus.FAILED
    assert test_case.expected_error_fragment in (result.error_message or "")
    assert (
        adapter.relation_exists(
            connection=connection,
            database=None,
            schema="main",
            name="customer_snapshot",
        )
        is False
    )

    adapter.close(connection)
