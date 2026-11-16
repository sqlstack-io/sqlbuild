"""Hook execution for model materialization lifecycle."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from sqlbuild._native import (
    CompiledRelationLocation,
    CostContext,
    DiscoveredHookFunction,
    HardCodedRelationOwnerKind,
    PythonHookEntry,
    PythonNodeIdentity,
    SourceEntry,
    SqlHookEntry,
    render_source_relation,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.relations.main.resolve_relation_location_qualified_name import (
    resolve_relation_location_qualified_name,
)
from sqlbuild.compiler.fingerprints.constants import NODE_TYPE_HOOK
from sqlbuild.compiler.python_nodes.main.identity import build_python_node_identity
from sqlbuild.compiler.python_nodes.types import SkipMode
from sqlbuild.cost.models import CostResourceContext
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.python_nodes.classes.runtime_relation_guard import RuntimeRelationGuard
from sqlbuild.executor.python_nodes.main._fingerprinting import (
    try_write_python_node_identity_fingerprint,
)
from sqlbuild.executor.python_nodes.types import PythonIdentityRecorder
from sqlbuild.executor.run.models import (
    HookContext,
    HookExecutionResult,
    HookInvocation,
    HookRelation,
    HookRelationLookup,
    HookRunContext,
    HookSkipResult,
)
from sqlbuild.executor.run.types import HookPhase
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.provider.main.runtime import (
    _empty_provider_container,
    invoke_with_providers,
)
from sqlbuild.python_nodes.models import SqlResourceRef
from sqlbuild.python_nodes.types import SqlResourceRefKind
from sqlbuild.runtime.observability.classes.operation_lifecycle import OperationLifecycle


def execute_hooks(
    *,
    connection: Any,
    adapter: BaseAdapter,
    hooks: object,
    phase: HookPhase,
    hook_functions: tuple[DiscoveredHookFunction, ...] = (),
    hook_run: HookRunContext | None = None,
    hook_results: list[HookExecutionResult] | None = None,
) -> bool:
    """Execute pre/post lifecycle hook entries."""

    resolved_hook_run: HookRunContext = HookRunContext() if hook_run is None else hook_run
    if hooks is None:
        return False
    if isinstance(hooks, str):
        _execute_sql_hook(
            connection=connection,
            adapter=adapter,
            statement=hooks,
            hook_index=0,
            phase=phase,
            hook_results=hook_results,
            label=_sql_hook_preview(hooks),
            hook_name=None,
        )
        return False
    if isinstance(hooks, SqlHookEntry):
        _execute_sql_hook(
            connection=connection,
            adapter=adapter,
            statement=hooks.statement,
            hook_index=0,
            phase=phase,
            hook_results=hook_results,
            label=hooks.name or _sql_hook_preview(hooks.statement),
            hook_name=hooks.name,
        )
        return False
    if isinstance(hooks, PythonHookEntry):
        return invoke_python_hook(
            connection=connection,
            adapter=adapter,
            hook_entry=hooks,
            hook_functions=hook_functions,
            hook_index=0,
            phase=phase,
            hook_run=resolved_hook_run,
            hook_results=hook_results,
        )
    if isinstance(hooks, list | tuple):
        skipped: bool = False
        hook_index: int
        hook: object
        for hook_index, hook in enumerate(hooks):
            if isinstance(hook, str):
                _execute_sql_hook(
                    connection=connection,
                    adapter=adapter,
                    statement=hook,
                    hook_index=hook_index,
                    phase=phase,
                    hook_results=hook_results,
                    label=_sql_hook_preview(hook),
                    hook_name=None,
                )
            elif isinstance(hook, SqlHookEntry):
                _execute_sql_hook(
                    connection=connection,
                    adapter=adapter,
                    statement=hook.statement,
                    hook_index=hook_index,
                    phase=phase,
                    hook_results=hook_results,
                    label=hook.name or _sql_hook_preview(hook.statement),
                    hook_name=hook.name,
                )
            elif isinstance(hook, PythonHookEntry):
                skipped = invoke_python_hook(
                    connection=connection,
                    adapter=adapter,
                    hook_entry=hook,
                    hook_functions=hook_functions,
                    hook_index=hook_index,
                    phase=phase,
                    hook_run=resolved_hook_run,
                    hook_results=hook_results,
                )
                if skipped:
                    return True
            else:
                raise ExecutorInputError(
                    f"{phase.value}[{hook_index}] must be a SQL or Python hook entry, "
                    f"got {type(hook).__name__}"
                )
        return skipped
    raise ExecutorInputError(
        f"{phase.value} must be a SQL/Python hook entry or list of hook entries, "
        f"got {type(hooks).__name__}"
    )


def invoke_python_hook(
    *,
    connection: Any,
    adapter: BaseAdapter,
    hook_entry: PythonHookEntry,
    hook_functions: tuple[DiscoveredHookFunction, ...],
    hook_index: int,
    phase: HookPhase,
    hook_run: HookRunContext,
    hook_results: list[HookExecutionResult] | None = None,
) -> bool:
    with CostContext.resource_scope(
        resource_type="hook",
        resource_name=hook_entry.name,
        phase=phase.value,
    ):
        return _invoke_python_hook(
            connection=connection,
            adapter=adapter,
            hook_entry=hook_entry,
            hook_functions=hook_functions,
            hook_index=hook_index,
            phase=phase,
            hook_run=hook_run,
            hook_results=hook_results,
        )


def _invoke_python_hook(
    *,
    connection: Any,
    adapter: BaseAdapter,
    hook_entry: PythonHookEntry,
    hook_functions: tuple[DiscoveredHookFunction, ...],
    hook_index: int,
    phase: HookPhase,
    hook_run: HookRunContext,
    hook_results: list[HookExecutionResult] | None = None,
) -> bool:
    model_name: str | None = hook_run.model_name
    destination: CompiledRelationLocation | None = hook_run.destination
    hook_label: str = f'{phase.value}[{hook_index}] python("{hook_entry.name}")'
    hook_function: DiscoveredHookFunction | None = _find_hook_function(
        name=hook_entry.name,
        hook_functions=hook_functions,
    )
    if hook_function is None:
        error_message: str = f"{hook_label} was not found in the runtime hook registry"
        _record_hook_result(
            hook_results=hook_results,
            phase=phase,
            hook_index=hook_index,
            hook_type="python",
            label=hook_entry.name,
            status=ExecutionStatus.FAILED,
            error_message=error_message,
        )
        raise ExecutorInputError(error_message)
    if model_name is None or destination is None:
        error_message = f"{hook_label} is missing runtime model context"
        _record_hook_result(
            hook_results=hook_results,
            phase=phase,
            hook_index=hook_index,
            hook_type="python",
            label=hook_entry.name,
            status=ExecutionStatus.FAILED,
            error_message=error_message,
        )
        raise ExecutorInputError(error_message)

    try:
        context: HookContext = build_hook_context(
            connection=connection,
            adapter=adapter,
            hook_function=hook_function,
            invocation=HookInvocation(entry=hook_entry, index=hook_index, phase=phase),
            model_name=model_name,
            destination=destination,
            hook_run=hook_run,
        )
    except ExecutorInputError as exc:
        _record_hook_result(
            hook_results=hook_results,
            phase=phase,
            hook_index=hook_index,
            hook_type="python",
            label=hook_entry.name,
            status=ExecutionStatus.FAILED,
            error_message=f"{hook_label} failed: {exc}",
        )
        raise ExecutorInputError(f"{hook_label} failed: {exc}") from exc
    with OperationLifecycle(
        operation_kind="python_node",
        operation_name="python_hook",
        hook_phase=phase.value,
        hook_index=hook_index,
        hook_type="python",
        hook_name=hook_entry.name,
    ) as lifecycle:
        try:
            returned: object = invoke_with_providers(
                function=hook_function.function,
                context=context,
                providers=hook_run.providers,
                supplied_kwargs=dict(hook_entry.kwargs),
            )
        except Exception as exc:
            lifecycle.failed(error=exc)
            error_message: str = f"{hook_label} failed: {exc}"
            _record_hook_result(
                hook_results=hook_results,
                phase=phase,
                hook_index=hook_index,
                hook_type="python",
                label=hook_entry.name,
                status=ExecutionStatus.FAILED,
                error_message=error_message,
            )
            raise ExecutorInputError(error_message) from exc
        if returned is not None and not isinstance(returned, HookSkipResult):
            error_message = f"{hook_label} returned unsupported value; return None or ctx.skip(...)"
            _record_hook_result(
                hook_results=hook_results,
                phase=phase,
                hook_index=hook_index,
                hook_type="python",
                label=hook_entry.name,
                status=ExecutionStatus.FAILED,
                error_message=error_message,
            )
            raise ExecutorInputError(error_message)
    if isinstance(returned, HookSkipResult):
        _record_hook_result(
            hook_results=hook_results,
            phase=phase,
            hook_index=hook_index,
            hook_type="python",
            label=hook_entry.name,
            status=ExecutionStatus.SKIPPED,
            skip_mode=returned.mode,
            skip_reason=returned.reason,
        )
        _record_python_hook_identity(
            hook_function=hook_function,
            adapter=adapter,
            connection=connection,
            run_id=hook_run.run_id,
            destination=destination,
            model_name=model_name,
            python_identity_recorder=hook_run.python_identity_recorder,
        )
        return True
    _record_hook_result(
        hook_results=hook_results,
        phase=phase,
        hook_index=hook_index,
        hook_type="python",
        label=hook_entry.name,
        status=ExecutionStatus.SUCCESS,
    )
    _record_python_hook_identity(
        hook_function=hook_function,
        adapter=adapter,
        connection=connection,
        run_id=hook_run.run_id,
        destination=destination,
        model_name=model_name,
        python_identity_recorder=hook_run.python_identity_recorder,
    )
    return False


def _record_python_hook_identity(
    *,
    hook_function: DiscoveredHookFunction,
    adapter: BaseAdapter,
    connection: Any,
    run_id: str,
    destination: CompiledRelationLocation,
    model_name: str,
    python_identity_recorder: PythonIdentityRecorder | None,
) -> None:
    identity: PythonNodeIdentity = build_python_node_identity(
        node_type=NODE_TYPE_HOOK,
        node_name=hook_function.name,
        function=hook_function.function,
        project_dir=hook_function.file_path.parents[len(hook_function.relative_path.parts) - 1],
        decorator_config={"description": hook_function.description},
    )
    if python_identity_recorder is not None:
        python_identity_recorder(identity=identity, _target_name=model_name)
    else:
        try_write_python_node_identity_fingerprint(
            identity=identity,
            adapter=adapter,
            connection=connection,
            run_id=run_id,
            database=destination.database,
            schema=destination.schema,
            target_name=model_name,
        )


def _execute_sql_hook(
    *,
    connection: Any,
    adapter: BaseAdapter,
    statement: str,
    hook_index: int,
    phase: HookPhase,
    hook_results: list[HookExecutionResult] | None,
    label: str,
    hook_name: str | None,
) -> None:
    try:
        current: CostResourceContext | None = CostContext.current()
        parent_name: str = "hook" if current is None else current.resource_name
        with CostContext.resource_scope(
            resource_type="hook",
            resource_name=f"{parent_name}.{phase.value}[{hook_index}]",
            phase=phase.value,
        ):
            with OperationLifecycle(
                operation_kind="quality",
                operation_name="sql_hook",
                hook_phase=phase.value,
                hook_index=hook_index,
                hook_type="sql",
                hook_name=hook_name,
            ):
                adapter.execute(connection=connection, sql=statement)
    except Exception as exc:
        _record_hook_result(
            hook_results=hook_results,
            phase=phase,
            hook_index=hook_index,
            hook_type="sql",
            label=label,
            status=ExecutionStatus.FAILED,
            error_message=str(exc),
        )
        raise
    _record_hook_result(
        hook_results=hook_results,
        phase=phase,
        hook_index=hook_index,
        hook_type="sql",
        label=label,
        status=ExecutionStatus.SUCCESS,
    )


def _record_hook_result(
    *,
    hook_results: list[HookExecutionResult] | None,
    phase: HookPhase,
    hook_index: int,
    hook_type: str,
    label: str,
    status: ExecutionStatus,
    skip_mode: SkipMode | None = None,
    skip_reason: str | None = None,
    error_message: str | None = None,
) -> None:
    if hook_results is None:
        return
    result_accumulator: list[HookExecutionResult] = hook_results
    result_accumulator.append(
        HookExecutionResult(
            phase=phase,
            index=hook_index,
            hook_type=hook_type,
            label=label,
            status=status,
            skip_mode=skip_mode,
            skip_reason=skip_reason,
            error_message=error_message,
        )
    )


def _sql_hook_preview(statement: str) -> str:
    normalized: str = " ".join(statement.split())
    hook_sql_preview_character_limit: int = 80
    if len(normalized) <= hook_sql_preview_character_limit:
        return normalized
    return normalized[:77] + "..."


def build_hook_context(  # noqa: PLR0913
    *,
    connection: Any,
    adapter: BaseAdapter,
    hook_function: DiscoveredHookFunction,
    invocation: HookInvocation,
    model_name: str,
    destination: CompiledRelationLocation,
    hook_run: HookRunContext,
) -> HookContext:
    relation: HookRelation = HookRelation(
        name=destination.name,
        schema=destination.schema,
        database=destination.database,
        qualified=resolve_relation_location_qualified_name(adapter=adapter, location=destination),
    )
    return HookContext(
        model_name=model_name,
        phase=invocation.phase,
        hook_name=invocation.entry.name,
        hook_index=invocation.index,
        run_id=hook_run.run_id,
        target=hook_run.target,
        target_config=hook_run.target_config,
        vars=hook_run.effective_vars if hook_run.effective_vars is not None else {},
        destination=relation,
        adapter_name=adapter.adapter_name,
        adapter=adapter,
        connection=connection,
        statement_recorder=hook_run.statement_recorder
        if hook_run.statement_recorder is not None
        else StatementRecorder(),
        providers=hook_run.providers
        if hook_run.providers is not None
        else _empty_provider_container(),
        relations=_resolve_hook_relations(
            adapter=adapter, refs=hook_function.reads, lookup=hook_run.relation_lookup
        ),
        relation_guard=_hook_relation_guard(
            adapter=adapter,
            hook_name=hook_function.name,
            model_name=model_name,
            hook_run=hook_run,
        ),
    )


def _hook_relation_guard(
    *, adapter: BaseAdapter, hook_name: str, model_name: str, hook_run: HookRunContext
) -> RuntimeRelationGuard | None:
    if not hook_run.enforce_explicit_references or hook_run.warnings is None:
        return None
    lookup: HookRelationLookup = hook_run.relation_lookup
    all_refs: tuple[SqlResourceRef, ...] = (
        *(
            SqlResourceRef(kind=SqlResourceRefKind.MODEL, name=name)
            for name in lookup.model_locations
        ),
        *(
            SqlResourceRef(kind=SqlResourceRefKind.SEED, name=name)
            for name in lookup.seed_locations
        ),
        *(SqlResourceRef(kind=SqlResourceRefKind.SOURCE, name=name) for name in lookup.source_map),
    )
    return RuntimeRelationGuard(
        owner_label=f"hook '{hook_name}' on model '{model_name}'",
        owner_kind=HardCodedRelationOwnerKind.HOOK,
        project_relations=_resolve_hook_relations(adapter=adapter, refs=all_refs, lookup=lookup),
        dialect=adapter.sql_analysis_dialect(),
        default_database=adapter.default_database(),
        default_schema=adapter.default_schema(),
        warnings=hook_run.warnings,
        own_refs=frozenset({SqlResourceRef(kind=SqlResourceRefKind.MODEL, name=model_name)}),
    )


def _resolve_hook_relations(
    *, adapter: BaseAdapter, refs: tuple[SqlResourceRef, ...], lookup: HookRelationLookup
) -> dict[SqlResourceRef, str]:
    relations: dict[SqlResourceRef, str] = {}
    for ref in refs:
        if ref.kind is SqlResourceRefKind.SOURCE:
            entry: SourceEntry | None = lookup.source_map.get(ref.name)
            if entry is None:
                raise ExecutorInputError(f"No runtime relation found for source '{ref.name}'")
            relations[ref] = render_source_relation(entry=entry, adapter=adapter)
            continue
        locations: Mapping[str, CompiledRelationLocation] = (
            lookup.model_locations
            if ref.kind is SqlResourceRefKind.MODEL
            else lookup.seed_locations
        )
        location: CompiledRelationLocation | None = locations.get(ref.name)
        if location is None:
            raise ExecutorInputError(f"No runtime relation found for {ref.kind.value} '{ref.name}'")
        relations[ref] = resolve_relation_location_qualified_name(
            adapter=adapter, location=location
        )
    return relations


def _find_hook_function(
    *, name: str, hook_functions: tuple[DiscoveredHookFunction, ...]
) -> DiscoveredHookFunction | None:
    hook_function: DiscoveredHookFunction
    for hook_function in hook_functions:
        if hook_function.name == name:
            return hook_function
    return None


def render_hooks(*, hooks: object, phase: HookPhase) -> tuple[str, ...]:
    if hooks is None:
        return ()
    if isinstance(hooks, str):
        return (hooks,)
    if isinstance(hooks, SqlHookEntry):
        return (hooks.statement,)
    if isinstance(hooks, PythonHookEntry):
        return ()
    if isinstance(hooks, list | tuple):
        statements: list[str] = []
        hook_index: int
        hook: object
        for hook_index, hook in enumerate(hooks):
            if isinstance(hook, str):
                statements.append(hook)
            elif isinstance(hook, SqlHookEntry):
                statements.append(hook.statement)
            elif isinstance(hook, PythonHookEntry):
                continue
            else:
                raise ExecutorInputError(
                    f"{phase.value}[{hook_index}] must be a SQL or Python hook entry, "
                    f"got {type(hook).__name__}"
                )
        return tuple(statements)
    raise ExecutorInputError(
        f"{phase.value} must be a SQL/Python hook entry or list of hook entries, "
        f"got {type(hooks).__name__}"
    )
