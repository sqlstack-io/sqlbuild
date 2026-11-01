"""Locate Rust-to-Python crossings and Python-typed storage in the native bindings crate."""

from __future__ import annotations

import re
from collections.abc import Callable, Iterable

from fensu import Fault, RuleContext

from scripts.fensu_policy.constants import (
    NATIVE_BINDINGS_ROOT,
    NATIVE_BUDGET_SEPARATOR,
    NATIVE_CROSSING_PATTERNS,
    NATIVE_EDGE_PATHS,
    NATIVE_LINE_COMMENT,
    NATIVE_PENDING_BUDGETS,
    NATIVE_PYTHON_FIELD_PATTERN,
    NATIVE_RUST_SUFFIX,
    NATIVE_SKIPPED_LINE_PREFIXES,
    NATIVE_STRING_QUOTE,
    NATIVE_TEST_DIRECTORY,
    NATIVE_TEST_FILE_NAME,
    NATIVE_TEST_MODULE_MARKER,
)

_CROSSING: re.Pattern[str] = re.compile("|".join(NATIVE_CROSSING_PATTERNS))
_PYTHON_FIELD: re.Pattern[str] = re.compile(NATIVE_PYTHON_FIELD_PATTERN)
_STRUCT_OPEN: re.Pattern[str] = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?struct\s+\w+[^;]*\{\s*$")
_FIELD: re.Pattern[str] = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?\w+\s*:")
_PYCLASS_MODULE: re.Pattern[str] = re.compile(
    r"(^[ \t]*#\[\s*(?:pyo3::)?pyclass\s*\([^]]*?\bmodule\s*=\s*(?:r#*)?)"
    r'"(?:\\.|[^"\\\n])*"',
    re.MULTILINE,
)


def is_checked_binding(*, path: str) -> bool:
    """Whether `path` is non-test bindings source; tests live under `tests/` or in `tests.rs`."""

    if not path.startswith(NATIVE_BINDINGS_ROOT) or not path.endswith(NATIVE_RUST_SUFFIX):
        return False
    parts: list[str] = path.split("/")
    name: str = parts[-1]
    return NATIVE_TEST_DIRECTORY not in parts[:-1] and name != NATIVE_TEST_FILE_NAME


def is_edge_path(*, path: str, edge_paths: Iterable[str]) -> bool:
    """Whether `path` lies under one of the configured edge prefixes."""

    return any(path == edge or path.startswith(edge.rstrip("/") + "/") for edge in edge_paths)


def budgets_by_path(*, entries: Iterable[str]) -> dict[str, int]:
    """Parse `path=count` budget entries."""

    budgets: dict[str, int] = {}
    entry: str
    for entry in entries:
        path, _, count = entry.rpartition(NATIVE_BUDGET_SEPARATOR)
        budgets[path.strip()] = int(count)
    return budgets


def _code_lines(*, source: str) -> list[tuple[int, str]]:
    lines: list[tuple[int, str]] = []
    number: int
    line: str
    for number, line in enumerate(source.splitlines(), start=1):
        stripped: str = line.strip()
        if stripped.startswith(NATIVE_TEST_MODULE_MARKER):
            break
        if stripped.startswith(NATIVE_SKIPPED_LINE_PREFIXES):
            continue
        code: str = line if NATIVE_STRING_QUOTE in line else line.split(NATIVE_LINE_COMMENT, 1)[0]
        lines.append((number, code))
    return lines


def crossing_lines(*, source: str) -> list[int]:
    """Line numbers of Rust-to-Python crossings, excluding inert PyO3 module metadata."""

    source = _PYCLASS_MODULE.sub(r'\1""', source)
    found: list[int] = []
    number: int
    line: str
    for number, line in _code_lines(source=source):
        found.extend([number] * len(_CROSSING.findall(line)))
    return found


def python_field_lines(*, source: str) -> list[int]:
    """Line numbers of struct fields that hold Python objects."""

    found: list[int] = []
    in_struct: bool = False
    number: int
    line: str
    for number, line in _code_lines(source=source):
        if _STRUCT_OPEN.match(line):
            in_struct = True
            continue
        if in_struct and line.strip().startswith("}"):
            in_struct = False
            continue
        if in_struct and _FIELD.match(line) and _PYTHON_FIELD.search(line):
            found.append(number)
    return found


def budgeted_faults(
    *,
    ctx: RuleContext,
    lines: Callable[..., list[int]],
    what: str,
) -> list[Fault]:
    """Faults for a bindings file whose `lines` exceed, or fall below, its pending budget."""

    path: str = "/".join(ctx.repo_relative_parts())
    if not is_checked_binding(path=path) or is_edge_path(
        path=path, edge_paths=ctx.option(NATIVE_EDGE_PATHS)
    ):
        return []
    return budget_faults(ctx=ctx, path=path, found=lines(source=ctx.source), what=what)


def budget_faults(*, ctx: RuleContext, path: str, found: list[int], what: str) -> list[Fault]:
    """Faults when the `found` lines of `path` exceed, or fall below, its pending budget."""

    budget: int | None = budgets_by_path(entries=ctx.option(NATIVE_PENDING_BUDGETS)).get(path)
    if budget is None:
        return [ctx.fault_for(path=ctx.path, line=line, column=0) for line in found]
    if len(found) > budget:
        return [
            ctx.fault_for(
                path=ctx.path,
                line=found[0],
                column=0,
                message=f"{len(found)} {what}, above the pending budget of {budget}",
            )
        ]
    if len(found) < budget:
        return [
            ctx.fault_for(
                path=ctx.path,
                line=1,
                column=0,
                message=f"{len(found)} {what}, below the pending budget of {budget}; lower it",
                remediation="Lower this file's pending_budgets entry to the new count.",
            )
        ]
    return []
