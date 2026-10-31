"""Test execution pipeline."""

from __future__ import annotations

import queue
import threading
import time
from collections.abc import Callable, Iterable, Mapping
from concurrent.futures import Future, ThreadPoolExecutor
from contextvars import copy_context
from dataclasses import dataclass, field, replace
from pathlib import Path
from typing import Any, cast
from uuid import uuid4

from sqlbuild._native import (
    CompiledObjectKey,
    CompiledRelationLocation,
    FunctionPlanEntry,
    PlanOutput,
    SqlTestPlanEntry,
    sql_test_resource_id,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.models import FunctionInfo
from sqlbuild.compiler.compile.types import FunctionLanguage
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.build.models import FunctionExecutionResult
from sqlbuild.executor.functions.main._execute import execute_function
from sqlbuild.executor.pipeline._helpers.connections import (
    close_connections,
    open_worker_connections,
)
from sqlbuild.executor.pipeline._helpers.temporary_functions import (
    called_function_keys,
    create_temporary_test_functions,
    install_temporary_test_functions,
    rewrite_test_entry,
)
from sqlbuild.executor.pipeline.exceptions import TemporaryTestFunctionError
from sqlbuild.executor.pipeline.main.raise_warehouse_connection_failure import (
    raise_warehouse_connection_failure,
)
from sqlbuild.executor.pipeline.models import TemporaryTestFunctions, TestPipelineCallbacks
from sqlbuild.executor.scheduling.classes.recovering_connection_pool import RecoveringConnectionPool
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.executor.testing.constants import (
    LOCAL_TEST_ENGINE_CODE,
    SOURCE_DIALECT,
    SQL_TEST_EXECUTION_ERROR_CODE,
)
from sqlbuild.executor.testing.main._execute import execute_sql_test
from sqlbuild.executor.testing.main._execute_local import execute_local_sql_test
from sqlbuild.executor.testing.main._local_engine import resolve_local_test_engine
from sqlbuild.executor.testing.models import (
    LocalTestEngine,
    LocalTestOptions,
    SqlTestExecutionResult,
    StepResult,
)
from sqlbuild.executor.testing.types import SqlTestOutcome
from sqlbuild.runtime.execution_limits.classes.connection_recovery import ConnectionRecovery
from sqlbuild.runtime.execution_limits.models import RunItemTimeouts
from sqlbuild.runtime.observability.classes.operation_lifecycle import OperationLifecycle
from sqlbuild.runtime.observability.classes.resource_attempt_lifecycle import (
    ResourceAttemptLifecycle,
)
from sqlbuild.runtime.observability.models import OperationAttributes


@dataclass
class _AuthoredStartCoordinator:
    condition: threading.Condition = field(default_factory=threading.Condition)
    next_index: int = 0

    def wait(self, *, index: int) -> None:
        with self.condition:
            _ = self.condition.wait_for(lambda: self.next_index == index)
            self.next_index += 1
            self.condition.notify_all()


def run_test_pipeline(
    *,
    plan: PlanOutput,
    connection_config: dict[str, object],
    adapter: BaseAdapter,
    callbacks: TestPipelineCallbacks | None = None,
    run_id: str | None = None,
    max_concurrency: int = 1,
    timeouts: RunItemTimeouts | None = None,
    local: LocalTestOptions | None = None,
) -> tuple[SqlTestExecutionResult, ...]:
    """Execute all SQL unit tests from a compiled plan, on the warehouse or locally."""

    entries: tuple[SqlTestPlanEntry, ...] = plan.test_entries
    if not entries:
        return ()
    resolved_callbacks: TestPipelineCallbacks = callbacks or TestPipelineCallbacks()
    if local is not None:
        return _run_local_test_pipeline(
            plan=plan,
            adapter=adapter,
            project_dir=local.project_dir,
            strict=local.strict,
            session_parameters=_session_parameters(connection_config),
            callbacks=resolved_callbacks,
            run_id=run_id or uuid4().hex,
            max_concurrency=max_concurrency,
            timeouts=timeouts or RunItemTimeouts(),
        )
    worker_count: int = min(max(1, max_concurrency), len(entries))
    if resolved_callbacks.on_connection_start is not None:
        resolved_callbacks.on_connection_start(worker_count)
    canonical_run_id: str = run_id or uuid4().hex
    start: float = time.monotonic()
    try:
        connections: tuple[Any, ...] = open_worker_connections(
            adapter=adapter,
            connection_config=connection_config,
            connection_count=worker_count,
        )
    except BaseException as error:
        if resolved_callbacks.on_connection_error is not None:
            resolved_callbacks.on_connection_error(
                worker_count, elapsed_seconds=time.monotonic() - start
            )
        raise_warehouse_connection_failure(adapter=adapter, error=error)
    if resolved_callbacks.on_connection_complete is not None:
        resolved_callbacks.on_connection_complete(
            worker_count, elapsed_seconds=time.monotonic() - start
        )
    active_error: BaseException | None = None
    installed_functions: list[TemporaryTestFunctions] = []
    recovery: ConnectionRecovery = ConnectionRecovery(
        adapter=adapter,
        connect=lambda: _connect_test_connection(
            adapter=adapter,
            connection_config=connection_config,
            installed_functions=installed_functions,
        ),
    )
    try:
        preflight_start: float = time.monotonic()
        if resolved_callbacks.on_progress is not None:
            resolved_callbacks.on_progress("Preparing test functions...")
        missing_functions_by_test: dict[CompiledObjectKey, tuple[str, ...]] = {}
        if any(entry.function_deps for entry in plan.test_entries):
            with OperationLifecycle(
                operation_kind="quality",
                operation_name="sql_test_setup",
                metadata={"item_count": len(plan.test_entries)},
                attributes=OperationAttributes(phase="setup", target_kind="sql_test"),
            ):
                missing_functions_by_test, temporary_functions = _prepare_test_functions(
                    plan=plan,
                    adapter=adapter,
                    connection=connections[0],
                )
                for ordinal, connection in enumerate(connections[1:], start=2):
                    label: str = f"test connection {ordinal} of {len(connections)}"
                    install_temporary_test_functions(
                        adapter=adapter,
                        connection=connection,
                        functions=temporary_functions,
                        connection_label=label,
                    )
                    _install_python_test_functions(
                        adapter=adapter,
                        connection=connection,
                        functions=temporary_functions,
                        connection_label=label,
                    )
                installed_functions.append(temporary_functions)
                entries = tuple(
                    rewrite_test_entry(
                        entry=entry,
                        replacements=temporary_functions.replacements,
                        syntax=adapter.sql_lexical_syntax.native_mapping,
                    )
                    for entry in entries
                )
        if resolved_callbacks.on_progress is not None:
            resolved_callbacks.on_progress(
                f"Prepared test functions. ({time.monotonic() - preflight_start:.2f}s)"
            )
        results: tuple[SqlTestExecutionResult, ...] = _run_tests(
            entries=entries,
            adapter=adapter,
            connections=connections,
            worker_count=worker_count,
            missing_functions_by_test=missing_functions_by_test,
            on_test_start=resolved_callbacks.on_test_start,
            on_test_complete=resolved_callbacks.on_test_complete,
            run_id=canonical_run_id,
            timeouts=timeouts or RunItemTimeouts(),
            recovery=recovery,
        )
        return results
    except BaseException as error:
        active_error = error
        raise
    finally:
        recovery.close_replacements()
        close_connections(adapter=adapter, connections=connections, active_error=active_error)


def _run_tests(
    *,
    entries: tuple[SqlTestPlanEntry, ...],
    adapter: BaseAdapter,
    connections: tuple[Any, ...],
    worker_count: int,
    missing_functions_by_test: dict[CompiledObjectKey, tuple[str, ...]],
    on_test_start: Callable[[SqlTestPlanEntry], None] | None,
    on_test_complete: Callable[[SqlTestExecutionResult], None] | None,
    run_id: str,
    timeouts: RunItemTimeouts,
    recovery: ConnectionRecovery,
) -> tuple[SqlTestExecutionResult, ...]:
    if worker_count == 1:
        serial_results: list[SqlTestExecutionResult] = []
        serial_connection: Any = connections[0]
        for entry in entries:
            result: SqlTestExecutionResult = _execute_test_entry(
                entry=entry,
                adapter=adapter,
                connection=serial_connection,
                missing_functions=missing_functions_by_test.get(entry.key, ()),
                on_test_start=on_test_start,
                before_test_start=None,
                run_id=run_id,
                timeouts=timeouts,
            )
            serial_results.append(result)
            serial_connection = recovery.usable(serial_connection)
            if on_test_complete is not None:
                on_test_complete(result)
        return tuple(serial_results)

    connection_pool: queue.Queue[Any] = RecoveringConnectionPool(recovery=recovery)
    for connection in connections:
        connection_pool.put(connection)
    start_coordinator: _AuthoredStartCoordinator = _AuthoredStartCoordinator()

    with ThreadPoolExecutor(max_workers=worker_count) as executor:
        submitted_futures: list[Future[SqlTestExecutionResult]] = []
        for index, entry in enumerate(entries):
            submitted_futures.append(
                cast(
                    Future[SqlTestExecutionResult],
                    executor.submit(
                        copy_context().run,
                        _execute_test_entry_with_pooled_connection,
                        entry=entry,
                        adapter=adapter,
                        connection_pool=connection_pool,
                        missing_functions=missing_functions_by_test.get(entry.key, ()),
                        on_test_start=on_test_start,
                        before_test_start=lambda index=index: start_coordinator.wait(index=index),
                        run_id=run_id,
                        timeouts=timeouts,
                    ),
                )
            )
        concurrent_results: list[SqlTestExecutionResult] = []
        for future in submitted_futures:
            result = future.result()
            concurrent_results.append(result)
            if on_test_complete is not None:
                on_test_complete(result)
        return tuple(concurrent_results)


def _execute_test_entry_with_pooled_connection(
    *,
    entry: SqlTestPlanEntry,
    adapter: BaseAdapter,
    connection_pool: queue.Queue[Any],
    missing_functions: tuple[str, ...],
    on_test_start: Callable[[SqlTestPlanEntry], None] | None,
    before_test_start: Callable[[], None] | None,
    run_id: str,
    timeouts: RunItemTimeouts,
) -> SqlTestExecutionResult:
    connection: Any = connection_pool.get()
    try:
        return _execute_test_entry(
            entry=entry,
            adapter=adapter,
            connection=connection,
            missing_functions=missing_functions,
            on_test_start=on_test_start,
            before_test_start=before_test_start,
            run_id=run_id,
            timeouts=timeouts,
        )
    finally:
        connection_pool.put(connection)


def _execute_test_entry(
    *,
    entry: SqlTestPlanEntry,
    adapter: BaseAdapter,
    connection: Any,
    missing_functions: tuple[str, ...],
    on_test_start: Callable[[SqlTestPlanEntry], None] | None,
    before_test_start: Callable[[], None] | None,
    run_id: str,
    timeouts: RunItemTimeouts,
) -> SqlTestExecutionResult:
    with ResourceAttemptLifecycle(
        resource_id=sql_test_resource_id(
            test_name=entry.name,
            source_path=entry.source_path,
            block_index=entry.block_index,
            case_name=entry.case_name,
        ),
        resource_kind="test",
        resource_name=_test_resource_name(entry),
        run_id=run_id,
    ) as lifecycle:
        if before_test_start is not None:
            before_test_start()
        if on_test_start is not None:
            on_test_start(entry)
        result: SqlTestExecutionResult = (
            _build_missing_function_result(
                test_entry=entry,
                missing_functions=missing_functions,
            )
            if missing_functions
            else execute_sql_test(
                test_entry=entry,
                adapter=adapter,
                connection=connection,
                timeouts=timeouts,
            )
        )
        if result.outcome != SqlTestOutcome.PASS:
            lifecycle.failed(error_code=result.error_code)
        return result


def _session_parameters(connection_config: dict[str, object]) -> dict[str, object] | None:
    parameters: object = connection_config.get("session_parameters")
    return cast(dict[str, object], parameters) if isinstance(parameters, dict) else None


def _test_resource_name(entry: SqlTestPlanEntry) -> str:
    return entry.name


def _connect_test_connection(
    *,
    adapter: BaseAdapter,
    connection_config: dict[str, object],
    installed_functions: list[TemporaryTestFunctions],
) -> Any:
    connection: Any = adapter.connect(dict(connection_config))
    try:
        functions: TemporaryTestFunctions
        for functions in installed_functions:
            install_temporary_test_functions(
                adapter=adapter,
                connection=connection,
                functions=functions,
                connection_label="a replacement test connection",
            )
            _install_python_test_functions(
                adapter=adapter,
                connection=connection,
                functions=functions,
                connection_label="a replacement test connection",
            )
    except BaseException:
        adapter.close(connection)
        raise
    return connection


def _prepare_test_functions(
    *,
    plan: PlanOutput,
    adapter: BaseAdapter,
    connection: Any,
) -> tuple[dict[CompiledObjectKey, tuple[str, ...]], TemporaryTestFunctions]:
    function_entries: dict[CompiledObjectKey, FunctionPlanEntry] = {
        entry.key: entry for entry in plan.function_entries
    }
    distinct_deps: set[tuple[CompiledObjectKey, ...]] = {
        test_entry.function_deps for test_entry in plan.test_entries
    }
    closures: dict[tuple[CompiledObjectKey, ...], tuple[CompiledObjectKey, ...]] = {
        deps: _function_closure(plan=plan, keys=deps, adapter=adapter) for deps in distinct_deps
    }
    deps_by_test: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]] = {
        test_entry.key: closures[test_entry.function_deps] for test_entry in plan.test_entries
    }
    required_targets_by_key: dict[CompiledObjectKey, CompiledRelationLocation] = {}
    test_entry: SqlTestPlanEntry
    for test_entry in plan.test_entries:
        dep: CompiledObjectKey
        for dep in deps_by_test[test_entry.key]:
            function_target: CompiledRelationLocation | None = plan.function_locations.get(dep.name)
            if function_target is not None:
                required_targets_by_key[dep] = function_target
    if not required_targets_by_key:
        return {}, TemporaryTestFunctions()
    temporary_functions: TemporaryTestFunctions = create_temporary_test_functions(
        function_entries=plan.function_entries,
        required_keys=tuple(required_targets_by_key),
        adapter=adapter,
        connection=connection,
    )
    created_key: CompiledObjectKey
    for created_key in temporary_functions.created_keys:
        _ = required_targets_by_key.pop(created_key, None)
    if not required_targets_by_key:
        return {}, temporary_functions

    existing_function_names: set[tuple[str | None, str | None, str]] = set()
    for database, schema, names in _group_function_names(required_targets_by_key.values()):
        function_infos: tuple[FunctionInfo, ...] = adapter.list_functions(
            connection=connection,
            database=database,
            schemas=(schema,) if schema is not None else None,
            names=names,
        )
        existing_function_names.update(
            (
                _normalize_name(info.database),
                _normalize_name(info.schema),
                _normalize_required_name(info.name),
            )
            for info in function_infos
        )

    missing_by_key: dict[CompiledObjectKey, str] = {}
    key: CompiledObjectKey
    target: CompiledRelationLocation
    for key, target in required_targets_by_key.items():
        function_key: tuple[str | None, str | None, str] = (
            _normalize_name(target.database),
            _normalize_name(target.schema),
            _normalize_required_name(target.name),
        )
        if function_key not in existing_function_names:
            missing_by_key[key] = _missing_function_label(
                name=key.name, reason=temporary_functions.failures.get(key)
            )

    registered: tuple[FunctionPlanEntry, ...] = _register_connection_scoped_python_functions(
        missing_by_key=missing_by_key,
        function_entries=function_entries,
        adapter=adapter,
        connection=connection,
    )
    temporary_functions = replace(temporary_functions, python_functions=registered)

    missing_by_test: dict[CompiledObjectKey, tuple[str, ...]] = {}
    for test_entry in plan.test_entries:
        missing_names: tuple[str, ...] = tuple(
            missing_by_key[dep] for dep in deps_by_test[test_entry.key] if dep in missing_by_key
        )
        if missing_names:
            missing_by_test[test_entry.key] = missing_names
    return missing_by_test, temporary_functions


def _function_closure(
    *, plan: PlanOutput, keys: tuple[CompiledObjectKey, ...], adapter: BaseAdapter
) -> tuple[CompiledObjectKey, ...]:
    called: tuple[CompiledObjectKey, ...] = called_function_keys(
        function_entries=plan.function_entries, keys=keys, adapter=adapter
    )
    return tuple(dict.fromkeys((*keys, *called)))


def _missing_function_label(*, name: str, reason: str | None) -> str:
    if reason is None:
        return name
    return f"{name} ({reason})"


def _register_connection_scoped_python_functions(
    *,
    missing_by_key: dict[CompiledObjectKey, str],
    function_entries: dict[CompiledObjectKey, FunctionPlanEntry],
    adapter: BaseAdapter,
    connection: Any,
) -> tuple[FunctionPlanEntry, ...]:
    if adapter.persists_python_functions():
        return ()
    registered: list[FunctionPlanEntry] = []
    key: CompiledObjectKey
    for key in tuple(missing_by_key):
        function_entry: FunctionPlanEntry | None = function_entries.get(key)
        if function_entry is None or function_entry.language != FunctionLanguage.PYTHON:
            continue
        result: FunctionExecutionResult = _register_python_function(
            adapter=adapter, connection=connection, function_entry=function_entry
        )
        if result.status == ExecutionStatus.SUCCESS:
            del missing_by_key[key]
            registered.append(function_entry)
    return tuple(registered)


def _register_python_function(
    *, adapter: BaseAdapter, connection: Any, function_entry: FunctionPlanEntry
) -> FunctionExecutionResult:
    return execute_function(
        function_entry=function_entry,
        adapter=adapter,
        connection=connection,
        statement_recorder=StatementRecorder(),
        run_id="test",
        query_change_tracking=False,
    )


def _install_python_test_functions(
    *,
    adapter: BaseAdapter,
    connection: Any,
    functions: TemporaryTestFunctions,
    connection_label: str,
) -> None:
    """Register the first connection's Python functions on another, or fail with T006."""

    function_entry: FunctionPlanEntry
    for function_entry in functions.python_functions:
        result: FunctionExecutionResult = _register_python_function(
            adapter=adapter, connection=connection, function_entry=function_entry
        )
        if result.status == ExecutionStatus.SUCCESS or _function_exists(
            adapter=adapter, connection=connection, function_entry=function_entry
        ):
            continue
        raise TemporaryTestFunctionError(
            function_name=function_entry.name,
            connection=connection_label,
            reason=result.error_message or result.status.value,
        )


def _function_exists(
    *, adapter: BaseAdapter, connection: Any, function_entry: FunctionPlanEntry
) -> bool:
    """Whether the connection already sees the function, as connections to one database file do."""

    destination: CompiledRelationLocation = function_entry.destination
    return bool(
        adapter.list_functions(
            connection=connection,
            database=destination.database,
            schemas=(destination.schema,) if destination.schema is not None else None,
            names=(destination.name,),
        )
    )


def _group_function_names(
    targets: Iterable[CompiledRelationLocation],
) -> tuple[tuple[str | None, str | None, tuple[str, ...]], ...]:
    grouped: dict[tuple[str | None, str | None], list[str]] = {}
    target: CompiledRelationLocation
    for target in targets:
        grouped.setdefault((target.database, target.schema), []).append(target.name)
    return tuple((database, schema, tuple(names)) for (database, schema), names in grouped.items())


def _normalize_name(value: str | None) -> str | None:
    if value is None:
        return None
    return value.lower()


def _normalize_required_name(value: str) -> str:
    return value.lower()


def _build_missing_function_result(
    *,
    test_entry: SqlTestPlanEntry,
    missing_functions: tuple[str, ...],
) -> SqlTestExecutionResult:
    function_list: str = ", ".join(missing_functions)
    error_message: str = (
        f"test '{test_entry.name}' depends on project function(s) {function_list}, "
        "but they do not exist in the target database. Run `sqb build` first, "
        "or build the required function(s) before running `sqb test`."
    )
    model_name: str = test_entry.chain[0].model_name if test_entry.chain else test_entry.name
    return SqlTestExecutionResult(
        test_name=test_entry.name,
        outcome=SqlTestOutcome.ERROR,
        source_path=test_entry.source_path,
        block_index=test_entry.block_index,
        parent_name=test_entry.parent_name,
        case_name=test_entry.case_name,
        case_index=test_entry.case_index,
        case_fingerprint=test_entry.case_fingerprint,
        parameter_schema=test_entry.parameter_schema,
        parameter_values=test_entry.parameter_values,
        step_results=(
            StepResult(
                model_name=model_name,
                outcome=SqlTestOutcome.ERROR,
                error_code=SQL_TEST_EXECUTION_ERROR_CODE,
                error_message=error_message,
            ),
        ),
        error_code=SQL_TEST_EXECUTION_ERROR_CODE,
        error_message=error_message,
    )


def _run_local_test_pipeline(
    *,
    plan: PlanOutput,
    adapter: BaseAdapter,
    project_dir: Path,
    strict: bool,
    session_parameters: Mapping[str, object] | None,
    callbacks: TestPipelineCallbacks,
    run_id: str,
    max_concurrency: int,
    timeouts: RunItemTimeouts,
) -> tuple[SqlTestExecutionResult, ...]:
    """Execute every planned test on in-memory DuckDB; no warehouse connection is opened."""

    dialect: str | None = adapter.sql_analysis_dialect()
    if dialect != SOURCE_DIALECT:
        raise ExecutorInputError(
            f"`sqb test --local` converts Snowflake tests; this target's dialect is {dialect}",
            code=LOCAL_TEST_ENGINE_CODE,
            help="Run `sqb test` without --local; this target can run its tests directly.",
        )
    entries: tuple[SqlTestPlanEntry, ...] = plan.test_entries
    if callbacks.on_progress is not None:
        callbacks.on_progress("Finding SQLQuack...")
    engine: LocalTestEngine = resolve_local_test_engine(
        project_dir=project_dir, strict=strict, session_parameters=session_parameters
    )
    if callbacks.on_progress is not None:
        callbacks.on_progress(f"Running tests on local DuckDB: {engine.describe()}")
    worker_count: int = min(max(1, max_concurrency), len(entries))
    with ThreadPoolExecutor(max_workers=worker_count) as executor:
        futures: list[Future[SqlTestExecutionResult]] = [
            cast(
                Future[SqlTestExecutionResult],
                executor.submit(
                    copy_context().run,
                    _execute_local_entry,
                    entry=entry,
                    adapter=adapter,
                    engine=engine,
                    callbacks=callbacks,
                    run_id=run_id,
                    timeouts=timeouts,
                ),
            )
            for entry in entries
        ]
        results: list[SqlTestExecutionResult] = []
        for future in futures:
            result: SqlTestExecutionResult = future.result()
            results.append(result)
            if callbacks.on_test_complete is not None:
                callbacks.on_test_complete(result)
    return tuple(results)


def _execute_local_entry(
    *,
    entry: SqlTestPlanEntry,
    adapter: BaseAdapter,
    engine: LocalTestEngine,
    callbacks: TestPipelineCallbacks,
    run_id: str,
    timeouts: RunItemTimeouts,
) -> SqlTestExecutionResult:
    with ResourceAttemptLifecycle(
        resource_id=sql_test_resource_id(
            test_name=entry.name,
            source_path=entry.source_path,
            block_index=entry.block_index,
            case_name=entry.case_name,
        ),
        resource_kind="test",
        resource_name=entry.name,
        run_id=run_id,
    ) as lifecycle:
        if callbacks.on_test_start is not None:
            callbacks.on_test_start(entry)
        result: SqlTestExecutionResult = execute_local_sql_test(
            test_entry=entry, adapter=adapter, engine=engine, timeouts=timeouts
        )
        if result.outcome != SqlTestOutcome.PASS:
            lifecycle.failed(error_code=result.error_code)
        return result
