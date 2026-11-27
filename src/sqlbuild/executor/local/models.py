"""Where one test or scenario runs, and why it cannot run locally."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class MacroPlace:
    """The refused construct in a macro's own source file."""

    path: str
    line: int
    column: int
    snippet: str


@dataclass(frozen=True)
class LocalRefusal:
    """One SQLQuack refusal, or one of SQLBuild's own reasons, placed in the file that holds it."""

    category: str
    construct: str
    reason: str
    code: str | None = None
    message: str | None = None
    label: str | None = None
    notes: tuple[str, ...] = field(default_factory=tuple)
    help: str | None = None
    path: str | None = None
    line: int | None = None
    column: int | None = None
    width: int = 1
    snippet: str | None = None
    snippet_column: int | None = None
    expanded_from: str | None = None
    macro_name: str | None = None
    """The macro the refused construct came from, as `@py.<name>` or `@sql.<name>`."""
    macro_place: MacroPlace | None = None
    """Where the macro's own source holds the construct; `None` when it can't be found there."""

    @property
    def title(self) -> str:
        """`<category>: <construct>`, as SQLQuack's census labels a refusal."""

        return f"{self.category}: {self.construct}"


@dataclass(frozen=True)
class RunOnDecision:
    """Where one test or scenario runs: `local` or `warehouse`, and why not locally."""

    name: str
    run_on: str
    refusals: tuple[LocalRefusal, ...] = field(default_factory=tuple)
    relied_on: tuple[str, ...] = field(default_factory=tuple)

    @property
    def reason(self) -> str | None:
        """The first refusal's label; `None` for a test that runs locally."""

        return self.refusals[0].title if self.refusals else None


@dataclass(frozen=True)
class Segment:
    """Where one CTE body sits in the comparison SQL, and what it stands for."""

    start: int
    end: int
    origin: str


@dataclass(frozen=True)
class TestSources:
    """What a unit test's comparison SQL was built from: its test file, models and compiled file."""

    sql: str
    segments: Sequence[Segment]
    test_path: str | None
    test_text: str | None
    compiled_path: str
    mocked_models: frozenset[str]
    models: Mapping[str, Any]
    chain_sql: Mapping[str, str]
    dialect: str
    macros: Mapping[str, Any] = field(default_factory=dict)
    """The project's loaded macros by name, to place a refusal in a macro's own source."""


@dataclass(frozen=True)
class DecisionContext:
    """What deciding where each unit test runs reads: the project, its dialect and converted SQL."""

    project: Any
    dialect: str | None
    run_on: str
    set_difference_operator: str
    functions: Mapping[str, Any]
    schema: Mapping[str, Any]
    time_zone: str | None
    rendered: Mapping[str, tuple[str, list[tuple[int, int, str]]]]
    project_dir: Path
    compiled_paths: Mapping[str, str]


@dataclass(frozen=True)
class ConvertedSql:
    """What SQLBuild keeps of a SQLQuack conversion: its DuckDB SQL and macros, or its refusals."""

    converted: bool
    sql: str
    macros: tuple[str, ...]
    refusals: tuple[Mapping[str, Any], ...]
    rendered: str
    relied_on: tuple[str, ...] = ()


@dataclass(frozen=True)
class LocalInputs:
    """What a project adds to SQLQuack's conversion of its local test SQL: allowances, overrides."""

    allow: tuple[tuple[str, bool], ...] = ()
    overrides: str | None = None
    digest: str = ""


@dataclass(frozen=True)
class LocalSqlCheck:
    """One statement's SQLQuack conversion, when asked for, and the relation DuckDB lacks for it."""

    conversion: ConvertedSql | None
    missing: str | None
    bound: bool = True


@dataclass(frozen=True)
class LocalWorkCounts:
    """Work done to decide where tests run: conversions, DuckDB bindings, schema serialisations."""

    conversions: int
    bindings: int
    schema_keys: int
    cached: int = 0
