"""SQL function execution helpers."""

from __future__ import annotations

import hashlib
from datetime import UTC, datetime
from typing import Any

from sqlbuild._native import (
    Fingerprint,
    FunctionPlanEntry,
    compute_function_definition_hash,
    error_code,
    error_help,
    error_message,
    function_node_type,
    write_fingerprint,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.models import FunctionDefinition
from sqlbuild.compiler.compile.types import FunctionLanguage
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.build.models import FunctionExecutionResult
from sqlbuild.executor.functions.constants import (
    FUNCTION_EXECUTION_FAILED_CODE,
    FUNCTION_PYTHON_UNSUPPORTED_CODE,
    FUNCTION_TABLE_UNSUPPORTED_CODE,
    FUNCTION_TARGET_INVALID_CODE,
)
from sqlbuild.executor.scheduling.types import ExecutionStatus


def execute_function(
    *,
    function_entry: FunctionPlanEntry,
    adapter: BaseAdapter,
    connection: Any,
    statement_recorder: StatementRecorder,
    run_id: str,
    query_change_tracking: bool,
    schema_prepared: bool = False,
) -> FunctionExecutionResult:
    """Create or replace one SQL function."""

    function_kind: str = function_node_type(return_columns=function_entry.return_columns)
    if function_entry.destination.qualified_name is None:
        return FunctionExecutionResult(
            function_name=function_entry.name,
            status=ExecutionStatus.FAILED,
            function_kind=function_kind,
            error_code=FUNCTION_TARGET_INVALID_CODE,
            error_message="function target could not be qualified",
        )
    warnings: list[str] = []
    try:
        if (
            function_entry.language == FunctionLanguage.PYTHON
            and not adapter.supports_python_functions()
        ):
            raise ExecutorInputError(
                f"Adapter '{type(adapter).__name__}' does not support Python UDFs",
                code=FUNCTION_PYTHON_UNSUPPORTED_CODE,
            )
        if function_entry.return_columns and not adapter.supports_table_functions():
            raise ExecutorInputError(
                f"Adapter '{type(adapter).__name__}' does not support SQL table functions",
                code=FUNCTION_TABLE_UNSUPPORTED_CODE,
            )
        if not schema_prepared:
            adapter.ensure_schema(
                connection=connection,
                database=function_entry.destination.database,
                schema=function_entry.destination.schema,
                statement_recorder=statement_recorder,
            )
        adapter.create_function(
            connection=connection,
            definition=FunctionDefinition(
                destination=function_entry.destination.qualified_name,
                arguments=function_entry.arguments,
                returns=function_entry.returns,
                body_sql=function_entry.body_sql,
                return_columns=function_entry.return_columns,
                language=function_entry.language,
                runtime_version=function_entry.runtime_version,
                entry_point=function_entry.entry_point,
                packages=function_entry.packages,
                source_file_path=function_entry.source_file_path,
            ),
            statement_recorder=statement_recorder,
        )
        warnings = _try_write_function_fingerprint(
            entry=function_entry,
            adapter=adapter,
            connection=connection,
            run_id=run_id,
            query_change_tracking=query_change_tracking,
            warnings=warnings,
            statement_recorder=statement_recorder,
            schema_prepared=schema_prepared,
        )
        return FunctionExecutionResult(
            function_name=function_entry.name,
            status=ExecutionStatus.SUCCESS,
            function_kind=function_kind,
            warning_messages=tuple(warnings),
            lifecycle_events=statement_recorder.snapshot(),
        )
    except Exception as error:
        return FunctionExecutionResult(
            function_name=function_entry.name,
            status=ExecutionStatus.FAILED,
            function_kind=function_kind,
            error_code=error_code(error=error, fallback_code=FUNCTION_EXECUTION_FAILED_CODE),
            error_help=error_help(error),
            error_message=error_message(error),
            warning_messages=tuple(warnings),
            lifecycle_events=statement_recorder.snapshot(),
        )


def _try_write_function_fingerprint(
    *,
    entry: FunctionPlanEntry,
    adapter: BaseAdapter,
    connection: Any,
    run_id: str,
    query_change_tracking: bool,
    warnings: list[str],
    statement_recorder: StatementRecorder,
    schema_prepared: bool,
) -> list[str]:
    if not query_change_tracking:
        return warnings
    fingerprint_schema: str | None = entry.fingerprint_destination.schema
    target_is_unqualified: bool = (
        entry.destination.schema is None and entry.destination.database is None
    )
    if target_is_unqualified and not adapter.supports_unqualified_function_fingerprints():
        fingerprint_schema = None
    if fingerprint_schema is None:
        warnings.append(
            "fingerprint write skipped for "
            f"function '{entry.name}': fingerprint schema is missing while "
            "query_change_tracking is enabled"
        )
        return warnings
    try:
        if not schema_prepared:
            adapter.ensure_schema(
                connection=connection,
                database=entry.fingerprint_destination.database,
                schema=fingerprint_schema,
                statement_recorder=statement_recorder,
            )
        schema_fp: str = hashlib.sha256(b"").hexdigest()
        definition_hash: str = compute_function_definition_hash(
            fingerprint_sql=entry.fingerprint_query_sql,
            language=str(entry.language),
            dialect=adapter.sql_analysis_dialect(),
        )
        fingerprint: Fingerprint = Fingerprint(
            node_type=function_node_type(return_columns=entry.return_columns),
            node_name=entry.name,
            target_database=entry.destination.database,
            target_schema=entry.destination.schema,
            target_name=entry.destination.name,
            run_id=run_id,
            definition_hash=definition_hash,
            version_hash=definition_hash,
            schema_fingerprint=schema_fp,
            definition=entry.fingerprint_query_sql,
            metadata_json="{}",
            ts=datetime.now(tz=UTC),
        )
        _ = write_fingerprint(
            connection=connection,
            execute=adapter.execute,
            database=entry.fingerprint_destination.database,
            schema=fingerprint_schema,
            fingerprint=fingerprint,
            render_qualified_name=adapter.render_qualified_name,
            render_framework_type=adapter.render_framework_type,
            read_columns=adapter.get_columns,
            render_add_columns=adapter.render_add_columns,
            render_create_table_sql=adapter.render_create_fingerprint_table_sql,
            render_create_index_sqls=adapter.render_create_fingerprint_index_sqls,
        )
    except Exception as exc:
        warnings.append(
            f"fingerprint write failed for function '{entry.name}'; "
            f"future function-change detection may be incorrect: {exc}"
        )
    return warnings
