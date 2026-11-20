"""Session-scoped copies of project functions so unit tests call the current definitions."""

from __future__ import annotations

import logging
import re
from collections.abc import Iterable
from dataclasses import replace
from typing import Any

from sqlbuild._native import (
    ChainStep,
    CompiledObjectKey,
    FunctionPlanEntry,
    SqlCodeScanner,
    SqlTestAssertionStep,
    SqlTestPlanEntry,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.models import FunctionDefinition, TemporaryFunction
from sqlbuild.diagnostics.classes.diagnostic_record_redactor import DiagnosticRecordRedactor
from sqlbuild.diagnostics.main.log_debug_event import log_debug_event
from sqlbuild.executor.pipeline.exceptions import TemporaryTestFunctionError
from sqlbuild.executor.pipeline.models import TemporaryTestFunctions
from sqlbuild.runtime.execution_limits.exceptions import (
    ExecutionDurationLimitError,
    ItemTimeoutError,
)

_LOGGER: logging.Logger = logging.getLogger("sqlbuild.execution")
_IDENTIFIER_QUOTES: frozenset[str] = frozenset({'"', "`", "["})
_SCANNER_CONTEXT: str = "SQL test function call rewriting"


def create_temporary_test_functions(
    *,
    function_entries: tuple[FunctionPlanEntry, ...],
    required_keys: Iterable[CompiledObjectKey],
    adapter: BaseAdapter,
    connection: Any,
) -> TemporaryTestFunctions:
    """Create a temporary copy of each required function and the functions its body calls."""

    syntax: dict[str, object] = adapter.sql_lexical_syntax.native_mapping
    entries_by_key: dict[CompiledObjectKey, FunctionPlanEntry] = _entries_by_key(function_entries)
    keys_by_target: dict[str, CompiledObjectKey] = _keys_by_target(entries_by_key)
    replacements: dict[str, str] = {}
    created_keys: set[CompiledObjectKey] = set()
    statements: list[tuple[str, str]] = []
    failures: dict[CompiledObjectKey, str] = {}
    key: CompiledObjectKey
    for key in _dependency_order(
        required_keys=required_keys, entries_by_key=entries_by_key, syntax=syntax
    ):
        entry: FunctionPlanEntry = entries_by_key[key]
        uncopied: tuple[CompiledObjectKey, ...] = tuple(
            callee
            for callee in _called_function_keys(
                sql=entry.body_sql, keys_by_target=keys_by_target, syntax=syntax
            )
            if callee in failures
        )
        if uncopied:
            failures[key] = (
                f"function {uncopied[0].name}, which it calls, has no temporary copy: "
                f"{failures[uncopied[0]]}"
            )
            continue
        deployed_target: str = str(entry.destination.qualified_name)
        temporary: TemporaryFunction | None = adapter.render_create_temporary_function(
            name=entry.name,
            definition=FunctionDefinition(
                destination=deployed_target,
                arguments=entry.arguments,
                returns=entry.returns,
                body_sql=rewrite_function_calls(
                    sql=entry.body_sql, replacements=replacements, syntax=syntax
                ),
                return_columns=entry.return_columns,
                language=entry.language,
                runtime_version=entry.runtime_version,
                entry_point=entry.entry_point,
                packages=entry.packages,
                dataframe=entry.dataframe,
                source_file_path=entry.source_file_path,
            ),
        )
        if temporary is None:
            failures[key] = (
                f"the {adapter.adapter_name} adapter cannot create a temporary copy of "
                f"{entry.language} function {entry.name} for the test run"
            )
            continue
        failure: str | None = _execute_statements(
            adapter=adapter, connection=connection, statements=temporary.statements
        )
        if failure is not None:
            failures[key] = f"its temporary copy for the test run could not be created: {failure}"
            continue
        replacements[deployed_target] = temporary.target
        created_keys.add(key)
        statements.extend((entry.name, statement) for statement in temporary.statements)
    return TemporaryTestFunctions(
        statements=tuple(statements),
        created_keys=frozenset(created_keys),
        replacements=replacements,
        failures=failures,
    )


def install_temporary_test_functions(
    *,
    adapter: BaseAdapter,
    connection: Any,
    functions: TemporaryTestFunctions,
    connection_label: str,
) -> None:
    """Create the copies the first connection created on another one, or fail with T006."""

    function_name: str
    statement: str
    for function_name, statement in functions.statements:
        try:
            _ = adapter.execute(connection=connection, sql=statement)
        except (ItemTimeoutError, ExecutionDurationLimitError):
            raise
        except Exception as error:
            raise TemporaryTestFunctionError(
                function_name=function_name,
                connection=connection_label,
                reason=DiagnosticRecordRedactor.text(str(error)) or type(error).__name__,
            ) from error


def called_function_keys(
    *,
    function_entries: tuple[FunctionPlanEntry, ...],
    keys: Iterable[CompiledObjectKey],
    adapter: BaseAdapter,
) -> tuple[CompiledObjectKey, ...]:
    """The planned functions among `keys` and every planned function their bodies call."""

    return _dependency_order(
        required_keys=keys,
        entries_by_key=_entries_by_key(function_entries),
        syntax=adapter.sql_lexical_syntax.native_mapping,
    )


def rewrite_test_entry(
    *, entry: SqlTestPlanEntry, replacements: dict[str, str], syntax: dict[str, object]
) -> SqlTestPlanEntry:
    """Point a test's function calls at their temporary copies."""

    if not replacements:
        return entry
    return replace(
        entry,
        chain=tuple(
            _rewrite_chain_step(step=step, replacements=replacements, syntax=syntax)
            for step in entry.chain
        ),
        assertions=tuple(
            _rewrite_assertion_step(step=step, replacements=replacements, syntax=syntax)
            for step in entry.assertions
        ),
    )


def rewrite_function_calls(
    *, sql: str, replacements: dict[str, str], syntax: dict[str, object]
) -> str:
    """Replace each deployed function target called in SQL code with its temporary target."""

    if not replacements:
        return sql
    pieces: list[str] = []
    copied_to: int = 0
    match: re.Match[str]
    for match in _code_matches(pattern=_call_pattern(targets=replacements), sql=sql, syntax=syntax):
        pieces.append(sql[copied_to : match.start()])
        pieces.append(replacements[match.group(1)])
        copied_to = match.end()
    pieces.append(sql[copied_to:])
    return "".join(pieces)


def _execute_statements(
    *, adapter: BaseAdapter, connection: Any, statements: tuple[str, ...]
) -> str | None:
    try:
        statement: str
        for statement in statements:
            _ = adapter.execute(connection=connection, sql=statement)
    except (ItemTimeoutError, ExecutionDurationLimitError):
        raise
    except Exception as error:
        reason: str = DiagnosticRecordRedactor.text(str(error))
        log_debug_event(
            logger=_LOGGER,
            message="Temporary test function could not be created; using the built function",
            sqlbuild_error=reason,
        )
        return reason
    return None


def _entries_by_key(
    function_entries: tuple[FunctionPlanEntry, ...],
) -> dict[CompiledObjectKey, FunctionPlanEntry]:
    return {
        entry.key: entry
        for entry in function_entries
        if entry.destination.qualified_name is not None
    }


def _keys_by_target(
    entries_by_key: dict[CompiledObjectKey, FunctionPlanEntry],
) -> dict[str, CompiledObjectKey]:
    return {str(entry.destination.qualified_name): key for key, entry in entries_by_key.items()}


def _dependency_order(
    *,
    required_keys: Iterable[CompiledObjectKey],
    entries_by_key: dict[CompiledObjectKey, FunctionPlanEntry],
    syntax: dict[str, object],
) -> tuple[CompiledObjectKey, ...]:
    keys_by_target: dict[str, CompiledObjectKey] = _keys_by_target(entries_by_key)
    ordered: list[CompiledObjectKey] = []
    visiting: set[CompiledObjectKey] = set()
    pending: list[tuple[CompiledObjectKey, bool]] = [
        (key, False) for key in reversed(tuple(required_keys)) if key in entries_by_key
    ]
    while pending:
        key, expanded = pending.pop()
        if key in ordered:
            continue
        if expanded:
            ordered.append(key)
            continue
        if key in visiting:
            continue
        visiting.add(key)
        pending.append((key, True))
        pending.extend(
            (dependency, False)
            for dependency in _called_function_keys(
                sql=entries_by_key[key].body_sql, keys_by_target=keys_by_target, syntax=syntax
            )
            if dependency != key and dependency not in ordered
        )
    return tuple(ordered)


def _called_function_keys(
    *, sql: str, keys_by_target: dict[str, CompiledObjectKey], syntax: dict[str, object]
) -> tuple[CompiledObjectKey, ...]:
    if not keys_by_target:
        return ()
    return tuple(
        keys_by_target[match.group(1)]
        for match in _code_matches(
            pattern=_call_pattern(targets=keys_by_target), sql=sql, syntax=syntax
        )
    )


def _call_pattern(*, targets: Iterable[str]) -> re.Pattern[str]:
    alternatives: str = "|".join(
        re.escape(target) for target in sorted(targets, key=lambda value: len(value), reverse=True)
    )
    return re.compile(rf"(?<![\w$.])({alternatives})(?=\s*\()")


def _code_matches(
    *, pattern: re.Pattern[str], sql: str, syntax: dict[str, object]
) -> tuple[re.Match[str], ...]:
    matches: tuple[re.Match[str], ...] = tuple(pattern.finditer(sql))
    if not matches:
        return ()
    hidden: tuple[tuple[int, int], ...] = _comment_and_string_ranges(sql=sql, syntax=syntax)
    return tuple(match for match in matches if not _inside(offset=match.start(), ranges=hidden))


def _inside(*, offset: int, ranges: tuple[tuple[int, int], ...]) -> bool:
    start: int
    end: int
    for start, end in ranges:
        if start <= offset < end:
            return True
    return False


def _comment_and_string_ranges(
    *, sql: str, syntax: dict[str, object]
) -> tuple[tuple[int, int], ...]:
    scanner: SqlCodeScanner = SqlCodeScanner(sql, syntax, _SCANNER_CONTEXT)
    ranges: list[tuple[int, int]] = []
    index: int = 0
    while index < len(sql):
        end: int | None = scanner.non_code_end(index)
        if end is None:
            index += 1
            continue
        if sql[index] not in _IDENTIFIER_QUOTES:
            ranges.append((index, end))
        index = max(end, index + 1)
    return tuple(ranges)


def _rewrite_chain_step(
    *, step: ChainStep, replacements: dict[str, str], syntax: dict[str, object]
) -> ChainStep:
    return replace(
        step,
        resolved_sql=rewrite_function_calls(
            sql=step.resolved_sql, replacements=replacements, syntax=syntax
        ),
        expected_cte_sql=_rewrite_optional(
            sql=step.expected_cte_sql, replacements=replacements, syntax=syntax
        ),
        lifted_ctes=_rewrite_ctes(ctes=step.lifted_ctes, replacements=replacements, syntax=syntax),
        comparison_body_sql=_rewrite_optional(
            sql=step.comparison_body_sql, replacements=replacements, syntax=syntax
        ),
        expected_lifted_ctes=_rewrite_ctes(
            ctes=step.expected_lifted_ctes, replacements=replacements, syntax=syntax
        ),
    )


def _rewrite_assertion_step(
    *, step: SqlTestAssertionStep, replacements: dict[str, str], syntax: dict[str, object]
) -> SqlTestAssertionStep:
    return replace(
        step,
        resolved_sql=rewrite_function_calls(
            sql=step.resolved_sql, replacements=replacements, syntax=syntax
        ),
        lifted_ctes=_rewrite_ctes(ctes=step.lifted_ctes, replacements=replacements, syntax=syntax),
        comparison_body_sql=_rewrite_optional(
            sql=step.comparison_body_sql, replacements=replacements, syntax=syntax
        ),
    )


def _rewrite_optional(
    *, sql: str | None, replacements: dict[str, str], syntax: dict[str, object]
) -> str | None:
    if sql is None:
        return None
    return rewrite_function_calls(sql=sql, replacements=replacements, syntax=syntax)


def _rewrite_ctes(
    *, ctes: tuple[tuple[str, str], ...], replacements: dict[str, str], syntax: dict[str, object]
) -> tuple[tuple[str, str], ...]:
    return tuple(
        (name, rewrite_function_calls(sql=sql, replacements=replacements, syntax=syntax))
        for name, sql in ctes
    )
