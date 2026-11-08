"""Local scenario replay entrypoint."""

from __future__ import annotations

from dataclasses import replace
from pathlib import Path
from typing import Any

from sqlbuild._native import (
    CompiledRelationLocation,
    FunctionPlanEntry,
    ModelPlanEntry,
    ScenarioArtifactKind,
    ScenarioAssertionExpectationPlan,
    ScenarioExecutionPlan,
    ScenarioExpectedExpectationPlan,
    ScenarioRelationPlan,
    SourceEntry,
    error_code,
    error_help,
    error_message,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.duckdb_backed_adapter import DuckDbBackedAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.types import TablePromotionMode
from sqlbuild.compiler.compile.types import FunctionLanguage
from sqlbuild.executor.build.models import FunctionExecutionResult
from sqlbuild.executor.functions.main._execute import execute_function
from sqlbuild.executor.run.models import ModelExecutionResult
from sqlbuild.executor.scenario._helpers.execution.model_execution import execute_scenario_models
from sqlbuild.executor.scenario._helpers.lifecycle.expectations import (
    execute_scenario_assertion_expectations,
    execute_scenario_expected_expectations,
)
from sqlbuild.executor.scenario._helpers.lifecycle.failures import first_failure_details
from sqlbuild.executor.scenario._helpers.local.snapshots import (
    load_scenario_snapshot_into_duckdb,
    local_snapshot_table_name,
)
from sqlbuild.executor.scenario._helpers.local.sql import (
    convert_sql_for_local_duckdb,
    function_sql_for_local_duckdb,
    replace_local_relations,
)
from sqlbuild.executor.scenario._helpers.snapshots.core import (
    build_scenario_snapshot_input_fingerprint,
    build_scenario_snapshot_input_specs,
    classify_scenario_snapshot_state,
)
from sqlbuild.executor.scenario.classes.running_scenarios import RunningScenarios
from sqlbuild.executor.scenario.constants import (
    SCENARIO_EXEC_ASSERTION_ERRORED,
    SCENARIO_EXEC_ASSERTION_FAILED,
    SCENARIO_EXEC_EXPECTED_ERRORED,
    SCENARIO_EXEC_EXPECTED_FAILED,
    SCENARIO_LOCAL_FUNCTION_FAILED,
    SCENARIO_LOCAL_INTERNAL,
    SCENARIO_LOCAL_MANIFEST_INVALID,
    SCENARIO_LOCAL_MODEL_FAILED,
    SCENARIO_LOCAL_SNAPSHOT_MISSING,
    SCENARIO_LOCAL_SNAPSHOT_STALE,
)
from sqlbuild.executor.scenario.models import (
    ScenarioAssertionExpectationExecutionResult,
    ScenarioExpectedExpectationExecutionResult,
    ScenarioFailureDetails,
    ScenarioLocalSnapshotLoadedRelation,
    ScenarioLocalSnapshotLoadResult,
    ScenarioRunResult,
    ScenarioSnapshotStateResult,
)
from sqlbuild.executor.scenario.types import (
    LocalSqlColumns,
    ScenarioLocalRunStatus,
    ScenarioSnapshotState,
)
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.runtime.observability.classes.resource_attempt_lifecycle import (
    ResourceAttemptLifecycle,
)

_LOCAL_STEP_FAILED_MESSAGE: str = "local scenario step failed"


def execute_local_scenario_load_only_run(
    *,
    project_dir: Path,
    scenario_plan: ScenarioExecutionPlan,
    adapter: BaseAdapter,
    strict: bool,
    promotion_mode: TablePromotionMode,
    capture_adapter: str | None = None,
    capture_dialect: str | None = None,
    running: RunningScenarios | None = None,
) -> ScenarioRunResult:
    """Run one scenario locally against a run-scoped DuckDB database."""

    snapshot_state: ScenarioSnapshotStateResult = classify_scenario_snapshot_state(
        project_dir=project_dir,
        scenario_plan=scenario_plan,
        capture_adapter=capture_adapter,
        capture_dialect=capture_dialect,
    )
    if snapshot_state.state == ScenarioSnapshotState.MISSING:
        return _local_snapshot_unavailable_result(
            scenario_name=scenario_plan.name,
            status=ScenarioLocalRunStatus.ERROR if strict else ScenarioLocalRunStatus.SKIP,
            code=SCENARIO_LOCAL_SNAPSHOT_MISSING,
            message=(
                f"Scenario '{scenario_plan.name}' is missing local snapshot manifest "
                f"'{snapshot_state.manifest_path.as_posix()}'."
            ),
            help=f"Run `sqb scenario capture {scenario_plan.name}` to create the snapshot.",
        )
    if snapshot_state.state == ScenarioSnapshotState.STALE:
        return _local_snapshot_unavailable_result(
            scenario_name=scenario_plan.name,
            status=ScenarioLocalRunStatus.ERROR if strict else ScenarioLocalRunStatus.SKIP,
            code=SCENARIO_LOCAL_SNAPSHOT_STALE,
            message=(
                f"Scenario '{scenario_plan.name}' local snapshot "
                f"'{snapshot_state.manifest_path.as_posix()}' is stale."
            ),
            help=f"Run `sqb scenario capture {scenario_plan.name}` to refresh the snapshot.",
        )
    if snapshot_state.state == ScenarioSnapshotState.INVALID:
        return _local_snapshot_unavailable_result(
            scenario_name=scenario_plan.name,
            status=ScenarioLocalRunStatus.ERROR,
            code=snapshot_state.error_code or SCENARIO_LOCAL_MANIFEST_INVALID,
            message=snapshot_state.error_message
            or (
                f"Scenario '{scenario_plan.name}' has invalid local snapshot manifest "
                f"'{snapshot_state.manifest_path.as_posix()}'."
            ),
            help="Fix scenario.json or regenerate it with `sqb scenario capture`.",
        )

    run_dir: Path = project_dir / "target" / "run" / "scenarios" / scenario_plan.name
    run_dir.mkdir(parents=True, exist_ok=True)
    duckdb_path: Path = run_dir / "local.duckdb"
    _remove_local_duckdb_files(duckdb_path)
    connection: Any = (
        adapter.connect_local_replay(duckdb_path)
        if isinstance(adapter, DuckDbBackedAdapter)
        else adapter.connect({"database": str(duckdb_path)})
    )
    if running is not None:
        running.start(scenario_plan=scenario_plan, connection=connection)
    try:
        input_fingerprint: str = build_scenario_snapshot_input_fingerprint(
            scenario_name=scenario_plan.name,
            input_specs=build_scenario_snapshot_input_specs(scenario_plan=scenario_plan),
            capture_adapter=capture_adapter,
            capture_dialect=capture_dialect,
        )
        load_result: ScenarioLocalSnapshotLoadResult = load_scenario_snapshot_into_duckdb(
            project_dir=project_dir,
            scenario_name=scenario_plan.name,
            current_input_fingerprint=input_fingerprint,
            connection=connection,
        )
        local_plan: ScenarioExecutionPlan = _build_local_execution_plan(
            scenario_plan=scenario_plan,
            load_result=load_result,
            source_dialect=load_result.manifest.capture_dialect,
        )
        result: ScenarioRunResult = _execute_local_plan(
            scenario_plan=local_plan,
            adapter=adapter,
            connection=connection,
            run_id=f"local-{scenario_plan.name}",
            promotion_mode=promotion_mode,
            duckdb_path=duckdb_path,
            loaded_relations=load_result.relations,
        )
    except Exception as exc:
        return ScenarioRunResult(
            scenario_name=scenario_plan.name,
            status=ExecutionStatus.FAILED,
            local_status=ScenarioLocalRunStatus.ERROR,
            retained=True,
            local_duckdb_path=duckdb_path,
            error_code=error_code(error=exc, fallback_code=SCENARIO_LOCAL_INTERNAL),
            error_help=error_help(exc),
            error_message=error_message(exc),
        )
    finally:
        if running is not None:
            running.finish(connection=connection)
        adapter.close(connection)

    return result


def _build_local_execution_plan(
    *,
    scenario_plan: ScenarioExecutionPlan,
    load_result: ScenarioLocalSnapshotLoadResult,
    source_dialect: str | None,
) -> ScenarioExecutionPlan:
    relation_replacements: dict[str, str] = _build_relation_replacements(
        scenario_plan=scenario_plan,
        load_result=load_result,
    )
    relation_plan: ScenarioRelationPlan = _build_local_relation_plan(
        scenario_plan=scenario_plan,
        load_result=load_result,
    )
    schema: dict[str, LocalSqlColumns] = _snapshot_schema(load_result)

    function_entries: list[FunctionPlanEntry] = []
    function_entry: FunctionPlanEntry
    for function_entry in scenario_plan.function_entries:
        function_sql: str = replace_local_relations(
            sql=function_entry.body_sql,
            relation_replacements=relation_replacements,
        )
        if function_entry.language == FunctionLanguage.SQL:
            function_sql = function_sql_for_local_duckdb(
                sql=function_sql,
                source_dialect=source_dialect,
                scenario_name=scenario_plan.name,
                resource_kind=str(function_entry.key.resource_type),
                resource_name=function_entry.name,
            )
        function_entries.append(replace(function_entry, body_sql=function_sql))

    model_entries: list[ModelPlanEntry] = []
    entry: ModelPlanEntry
    for entry in scenario_plan.model_entries:
        local_sql: str = replace_local_relations(
            sql=entry.resolved_sql,
            relation_replacements=relation_replacements,
        )
        local_columns: LocalSqlColumns
        local_sql, local_columns = convert_sql_for_local_duckdb(
            sql=local_sql,
            source_dialect=source_dialect,
            schema=schema,
            scenario_name=scenario_plan.name,
            resource_kind="model",
            resource_name=entry.name,
        )
        schema[_snowflake_stored_name(relation_plan.model_locations[entry.name].name)] = (
            local_columns
        )
        model_entries.append(
            replace(
                entry,
                destination=relation_plan.model_locations[entry.name],
                resolved_sql=local_sql,
                pre_hooks=None,
                post_hooks=None,
                type_enforcement=False,
            )
        )

    expected_expectations: list[ScenarioExpectedExpectationPlan] = []
    expected_expectation: ScenarioExpectedExpectationPlan
    for expected_expectation in scenario_plan.expected_expectations:
        expected_sql: str = replace_local_relations(
            sql=expected_expectation.expected_sql,
            relation_replacements=relation_replacements,
        )
        expected_sql, _ = convert_sql_for_local_duckdb(
            sql=expected_sql,
            source_dialect=source_dialect,
            schema=schema,
            scenario_name=scenario_plan.name,
            resource_kind="expected comparison",
            resource_name=expected_expectation.model_name,
        )
        expected_expectations.append(
            replace(
                expected_expectation,
                actual_destination=relation_plan.model_locations[expected_expectation.model_name],
                expected_sql=expected_sql,
            )
        )

    assertion_expectations: list[ScenarioAssertionExpectationPlan] = []
    assertion_expectation: ScenarioAssertionExpectationPlan
    for assertion_expectation in scenario_plan.assertion_expectations:
        assertion_sql: str = replace_local_relations(
            sql=assertion_expectation.sql,
            relation_replacements=relation_replacements,
        )
        assertion_sql, _ = convert_sql_for_local_duckdb(
            sql=assertion_sql,
            source_dialect=source_dialect,
            schema=schema,
            scenario_name=scenario_plan.name,
            resource_kind="assertion",
            resource_name=assertion_expectation.name,
        )
        assertion_expectations.append(replace(assertion_expectation, sql=assertion_sql))

    return replace(
        scenario_plan,
        relation_plan=relation_plan,
        fixture_plans=(),
        seed_entries=(),
        function_entries=tuple(function_entries),
        model_entries=tuple(model_entries),
        expected_expectations=tuple(expected_expectations),
        assertion_expectations=tuple(assertion_expectations),
    )


def _execute_local_plan(
    *,
    scenario_plan: ScenarioExecutionPlan,
    adapter: BaseAdapter,
    connection: Any,
    run_id: str,
    promotion_mode: TablePromotionMode,
    duckdb_path: Path,
    loaded_relations: tuple[ScenarioLocalSnapshotLoadedRelation, ...],
) -> ScenarioRunResult:
    function_results: tuple[FunctionExecutionResult, ...] = _execute_local_functions(
        scenario_plan=scenario_plan,
        adapter=adapter,
        connection=connection,
        run_id=run_id,
    )
    if _has_failed(function_results):
        return _local_result(
            scenario_plan=scenario_plan,
            local_status=ScenarioLocalRunStatus.ERROR,
            retained=True,
            duckdb_path=duckdb_path,
            loaded_relations=loaded_relations,
            function_results=function_results,
            failure=first_failure_details(
                results=function_results,
                fallback_message=_LOCAL_STEP_FAILED_MESSAGE,
                fallback_code=SCENARIO_LOCAL_FUNCTION_FAILED,
            ),
        )

    model_results: tuple[ModelExecutionResult, ...] = execute_scenario_models(
        scenario_plan=scenario_plan,
        adapter=adapter,
        connection=connection,
        run_id=run_id,
        promotion_mode=promotion_mode,
    )
    if _has_failed(model_results):
        local_model_results: tuple[ModelExecutionResult, ...] = _with_local_model_error_codes(
            model_results
        )
        return _local_result(
            scenario_plan=scenario_plan,
            local_status=ScenarioLocalRunStatus.ERROR,
            retained=True,
            duckdb_path=duckdb_path,
            loaded_relations=loaded_relations,
            function_results=function_results,
            model_results=local_model_results,
            failure=first_failure_details(
                results=local_model_results,
                fallback_message=_LOCAL_STEP_FAILED_MESSAGE,
                fallback_code=SCENARIO_LOCAL_MODEL_FAILED,
            ),
        )

    expected_results: tuple[ScenarioExpectedExpectationExecutionResult, ...]
    expected_results = _with_local_expected_check_error_codes(
        execute_scenario_expected_expectations(
            scenario_plan=scenario_plan,
            adapter=adapter,
            connection=connection,
        )
    )
    assertion_results: tuple[ScenarioAssertionExpectationExecutionResult, ...]
    assertion_results = _with_local_assertion_check_error_codes(
        execute_scenario_assertion_expectations(
            scenario_plan=scenario_plan,
            adapter=adapter,
            connection=connection,
        )
    )
    check_results: tuple[object, ...] = (*expected_results, *assertion_results)
    if _has_local_check_error(check_results):
        return _local_result(
            scenario_plan=scenario_plan,
            local_status=ScenarioLocalRunStatus.ERROR,
            retained=True,
            duckdb_path=duckdb_path,
            loaded_relations=loaded_relations,
            function_results=function_results,
            model_results=model_results,
            expected_results=expected_results,
            assertion_results=assertion_results,
            failure=first_failure_details(
                results=check_results,
                fallback_message=_LOCAL_STEP_FAILED_MESSAGE,
                fallback_code=SCENARIO_LOCAL_MODEL_FAILED,
            ),
        )
    if _has_failed(check_results):
        return _local_result(
            scenario_plan=scenario_plan,
            local_status=ScenarioLocalRunStatus.FAIL,
            retained=True,
            duckdb_path=duckdb_path,
            loaded_relations=loaded_relations,
            function_results=function_results,
            model_results=model_results,
            expected_results=expected_results,
            assertion_results=assertion_results,
            failure=first_failure_details(
                results=check_results,
                fallback_message=_LOCAL_STEP_FAILED_MESSAGE,
            ),
        )
    return _local_result(
        scenario_plan=scenario_plan,
        local_status=ScenarioLocalRunStatus.PASS,
        retained=True,
        duckdb_path=duckdb_path,
        loaded_relations=loaded_relations,
        function_results=function_results,
        model_results=model_results,
        expected_results=expected_results,
        assertion_results=assertion_results,
    )


def _execute_local_functions(
    *,
    scenario_plan: ScenarioExecutionPlan,
    adapter: BaseAdapter,
    connection: Any,
    run_id: str,
) -> tuple[FunctionExecutionResult, ...]:
    results: list[FunctionExecutionResult] = []
    function_entry: FunctionPlanEntry
    for function_entry in scenario_plan.function_entries:
        resource_kind: str = str(function_entry.key.resource_type)
        with ResourceAttemptLifecycle(
            resource_id=(f"scenario:{scenario_plan.name}:{resource_kind}:{function_entry.name}"),
            resource_kind=resource_kind,
            resource_name=function_entry.name,
            run_id=run_id,
        ) as lifecycle:
            result: FunctionExecutionResult = execute_function(
                function_entry=function_entry,
                adapter=adapter,
                connection=connection,
                statement_recorder=StatementRecorder(),
                run_id=run_id,
                query_change_tracking=False,
            )
            if result.status == ExecutionStatus.FAILED:
                result = replace(
                    result,
                    error_code=SCENARIO_LOCAL_FUNCTION_FAILED,
                    error_help=(
                        "Inspect the retained local DuckDB database and function definition."
                    ),
                    error_message=(
                        f"local function '{result.function_name}' failed: {result.error_message}"
                        if result.error_message is not None
                        else f"local function '{result.function_name}' failed"
                    ),
                )
                lifecycle.failed(error_code=result.error_code)
        results.append(result)
        if result.status == ExecutionStatus.FAILED:
            break
    return tuple(results)


def _snapshot_schema(load_result: ScenarioLocalSnapshotLoadResult) -> dict[str, LocalSqlColumns]:
    columns: dict[tuple[ScenarioArtifactKind, str], LocalSqlColumns] = {
        (relation.kind, relation.logical_name): tuple(
            (_snowflake_stored_name(column.name), column.warehouse_type)
            for column in relation.columns
        )
        for relation in load_result.manifest.relations
    }
    return {
        _snowflake_stored_name(loaded.table_name): columns[(loaded.kind, loaded.logical_name)]
        for loaded in load_result.relations
    }


def _snowflake_stored_name(name: str) -> str:
    """The name Snowflake stores for a logical name: a quoted name as written, else upper case."""

    if name.startswith('"') and name.endswith('"'):
        return name[1:-1].replace('""', '"')
    return name.upper()


def _build_local_relation_plan(
    *, scenario_plan: ScenarioExecutionPlan, load_result: ScenarioLocalSnapshotLoadResult
) -> ScenarioRelationPlan:
    source_map: dict[str, SourceEntry] = dict(scenario_plan.relation_plan.source_map)
    source_fixture_locations: dict[str, CompiledRelationLocation] = {}
    ref_fixture_locations: dict[str, CompiledRelationLocation] = {}
    dbt_ref_fixture_locations: dict[str, CompiledRelationLocation] = {}
    seed_fixture_locations: dict[str, CompiledRelationLocation] = {}
    seed_locations: dict[str, CompiledRelationLocation] = {}
    model_locations: dict[str, CompiledRelationLocation] = {}

    loaded_targets: dict[tuple[ScenarioArtifactKind, str], CompiledRelationLocation] = {
        (relation.kind, relation.logical_name): _local_target(relation.table_name)
        for relation in load_result.relations
    }
    source_name: str
    for source_name in scenario_plan.graph_plan.source_fixture_names:
        target: CompiledRelationLocation = loaded_targets[
            (ScenarioArtifactKind.SOURCE, source_name)
        ]
        source_fixture_locations[source_name] = target
        source_entry: SourceEntry = source_map[source_name]
        source_map[source_name] = replace(
            source_entry,
            database=None,
            schema=None,
            table=None,
            expression=target.name,
            type_enforcement=False,
        )
    ref_name: str
    for ref_name in scenario_plan.graph_plan.ref_fixture_names:
        target = loaded_targets[(ScenarioArtifactKind.REF, ref_name)]
        ref_fixture_locations[ref_name] = target
        model_locations[ref_name] = target
    dbt_ref_name: str
    for dbt_ref_name in scenario_plan.graph_plan.dbt_ref_fixture_names:
        dbt_ref_fixture_locations[dbt_ref_name] = loaded_targets[
            (ScenarioArtifactKind.DBT_REF, dbt_ref_name)
        ]
    seed_name: str
    for seed_name in scenario_plan.graph_plan.seed_names:
        target = loaded_targets[(ScenarioArtifactKind.SEED, seed_name)]
        seed_locations[seed_name] = target
        if seed_name in scenario_plan.graph_plan.seed_fixture_names:
            seed_fixture_locations[seed_name] = target
    model_name: str
    for model_name in scenario_plan.graph_plan.model_names:
        model_locations[model_name] = _local_target(
            local_snapshot_table_name(kind=ScenarioArtifactKind.MODEL, logical_name=model_name)
        )

    return replace(
        scenario_plan.relation_plan,
        model_locations=model_locations,
        seed_locations=seed_locations,
        source_map=source_map,
        source_fixture_locations=source_fixture_locations,
        ref_fixture_locations=ref_fixture_locations,
        dbt_ref_fixture_locations=dbt_ref_fixture_locations,
        seed_fixture_locations=seed_fixture_locations,
    )


def _build_relation_replacements(
    *, scenario_plan: ScenarioExecutionPlan, load_result: ScenarioLocalSnapshotLoadResult
) -> dict[str, str]:
    replacements: dict[str, str] = {}
    loaded_names: dict[tuple[ScenarioArtifactKind, str], str] = {
        (relation.kind, relation.logical_name): relation.table_name
        for relation in load_result.relations
    }
    source_name: str
    for source_name, target in scenario_plan.relation_plan.source_fixture_locations.items():
        replacements.update(
            _target_replacements(
                target=target,
                local_name=loaded_names[(ScenarioArtifactKind.SOURCE, source_name)],
            )
        )
    ref_name: str
    for ref_name, target in scenario_plan.relation_plan.ref_fixture_locations.items():
        replacements.update(
            _target_replacements(
                target=target,
                local_name=loaded_names[(ScenarioArtifactKind.REF, ref_name)],
            )
        )
    dbt_ref_name: str
    for dbt_ref_name, target in scenario_plan.relation_plan.dbt_ref_fixture_locations.items():
        replacements.update(
            _target_replacements(
                target=target,
                local_name=loaded_names[(ScenarioArtifactKind.DBT_REF, dbt_ref_name)],
            )
        )
    seed_name: str
    for seed_name, target in scenario_plan.relation_plan.seed_locations.items():
        replacements.update(
            _target_replacements(
                target=target,
                local_name=loaded_names[(ScenarioArtifactKind.SEED, seed_name)],
            )
        )
    model_name: str
    for model_name, target in scenario_plan.relation_plan.model_locations.items():
        if model_name in scenario_plan.graph_plan.ref_fixture_names:
            continue
        replacements.update(
            _target_replacements(
                target=target,
                local_name=local_snapshot_table_name(
                    kind=ScenarioArtifactKind.MODEL,
                    logical_name=model_name,
                ),
            )
        )
    return replacements


def _local_snapshot_unavailable_result(
    *,
    scenario_name: str,
    status: ScenarioLocalRunStatus,
    code: str,
    message: str,
    help: str,
) -> ScenarioRunResult:
    return ScenarioRunResult(
        scenario_name=scenario_name,
        status=ExecutionStatus.SKIPPED
        if status == ScenarioLocalRunStatus.SKIP
        else ExecutionStatus.FAILED,
        local_status=status,
        retained=False,
        error_code=code,
        error_help=help,
        error_message=message,
    )


def _local_result(
    *,
    scenario_plan: ScenarioExecutionPlan,
    local_status: ScenarioLocalRunStatus,
    retained: bool,
    duckdb_path: Path | None,
    loaded_relations: tuple[ScenarioLocalSnapshotLoadedRelation, ...] = (),
    function_results: tuple[FunctionExecutionResult, ...] = (),
    model_results: tuple[ModelExecutionResult, ...] = (),
    expected_results: tuple[ScenarioExpectedExpectationExecutionResult, ...] = (),
    assertion_results: tuple[ScenarioAssertionExpectationExecutionResult, ...] = (),
    failure: ScenarioFailureDetails | None = None,
) -> ScenarioRunResult:
    resolved_failure: ScenarioFailureDetails = (
        failure if failure is not None else ScenarioFailureDetails()
    )
    return ScenarioRunResult(
        scenario_name=scenario_plan.name,
        status=ExecutionStatus.SUCCESS
        if local_status == ScenarioLocalRunStatus.PASS
        else ExecutionStatus.FAILED,
        local_status=local_status,
        retained=retained,
        local_duckdb_path=duckdb_path,
        local_execution_plan=scenario_plan,
        local_snapshot_relations=loaded_relations,
        relation_map=None,
        function_results=function_results,
        model_results=model_results,
        expected_results=expected_results,
        assertion_results=assertion_results,
        error_code=resolved_failure.error_code,
        error_help=resolved_failure.error_help,
        error_message=resolved_failure.error_message,
    )


def _local_target(table_name: str) -> CompiledRelationLocation:
    return CompiledRelationLocation(
        database=None,
        schema=None,
        name=table_name,
        qualified_name=None,
    )


def _target_replacements(*, target: CompiledRelationLocation, local_name: str) -> dict[str, str]:
    return {original: local_name for original in _target_name_variants(target)}


def _target_name_variants(target: CompiledRelationLocation) -> tuple[str, ...]:
    values: list[str] = [target.name, f'"{target.name}"']
    if target.qualified_name is not None:
        values.append(target.qualified_name)
        values.append(f'"{target.qualified_name}"')
    if target.schema is not None:
        values.append(f"{target.schema}.{target.name}")
        values.append(f'"{target.schema}"."{target.name}"')
    if target.database is not None and target.schema is not None:
        values.append(f"{target.database}.{target.schema}.{target.name}")
        values.append(f'"{target.database}"."{target.schema}"."{target.name}"')
    return tuple(dict.fromkeys(values))


def _with_local_model_error_codes(
    results: tuple[ModelExecutionResult, ...],
) -> tuple[ModelExecutionResult, ...]:
    return tuple(
        replace(
            result,
            error_code=SCENARIO_LOCAL_MODEL_FAILED,
            error_help=result.error_help or "Inspect the retained local DuckDB database.",
            error_message=(
                f"local model '{result.model_name}' failed: {result.error_message}"
                if result.error_message is not None
                else f"local model '{result.model_name}' failed"
            ),
        )
        if result.status == ExecutionStatus.FAILED
        else result
        for result in results
    )


def _with_local_expected_check_error_codes(
    results: tuple[ScenarioExpectedExpectationExecutionResult, ...],
) -> tuple[ScenarioExpectedExpectationExecutionResult, ...]:
    remapped_results: list[ScenarioExpectedExpectationExecutionResult] = []
    result: ScenarioExpectedExpectationExecutionResult
    for result in results:
        if result.error_code == SCENARIO_EXEC_EXPECTED_ERRORED:
            remapped_results.append(
                replace(
                    result,
                    error_code=SCENARIO_LOCAL_MODEL_FAILED,
                    error_help="Inspect the retained local DuckDB database and local SQL.",
                )
            )
            continue
        remapped_results.append(result)
    return tuple(remapped_results)


def _with_local_assertion_check_error_codes(
    results: tuple[ScenarioAssertionExpectationExecutionResult, ...],
) -> tuple[ScenarioAssertionExpectationExecutionResult, ...]:
    remapped_results: list[ScenarioAssertionExpectationExecutionResult] = []
    result: ScenarioAssertionExpectationExecutionResult
    for result in results:
        if result.error_code == SCENARIO_EXEC_ASSERTION_ERRORED:
            remapped_results.append(
                replace(
                    result,
                    error_code=SCENARIO_LOCAL_MODEL_FAILED,
                    error_help="Inspect the retained local DuckDB database and local SQL.",
                )
            )
            continue
        remapped_results.append(result)
    return tuple(remapped_results)


def _has_failed(results: tuple[object, ...]) -> bool:
    return any(getattr(result, "status", None) == ExecutionStatus.FAILED for result in results)


def _has_local_check_error(results: tuple[object, ...]) -> bool:
    return any(
        getattr(result, "error_code", None)
        not in (None, SCENARIO_EXEC_EXPECTED_FAILED, SCENARIO_EXEC_ASSERTION_FAILED)
        for result in results
        if getattr(result, "status", None) == ExecutionStatus.FAILED
    )


def _remove_local_duckdb_files(duckdb_path: Path) -> None:
    duckdb_path.unlink(missing_ok=True)
    duckdb_path.with_name(f"{duckdb_path.name}.wal").unlink(missing_ok=True)
