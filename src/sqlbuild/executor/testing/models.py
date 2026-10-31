"""SQL unit test execution result models."""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from sqlbuild._native import SqlTestParameterDeclaration, SqlValue
from sqlbuild.executor.testing.types import SqlTestOutcome
from sqlbuild.runtime.execution_limits.models import ItemTimeoutReport


@dataclass(frozen=True)
class SqlTestDifferenceSample:
    """One redacted, size-bounded row from an expected-output difference."""

    values: tuple[tuple[str, str], ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class SqlTestColumnDifference:
    """One safely aligned, redacted actual and expected column value."""

    name: str
    actual: str
    expected: str


@dataclass(frozen=True)
class StepResult:
    """Outcome of one chain step in a SQL unit test."""

    model_name: str
    outcome: SqlTestOutcome
    actual_row_count: int = 0
    expected_row_count: int = 0
    mismatched_row_count: int = 0
    unexpected_row_count: int | None = None
    missing_row_count: int | None = None
    unexpected_samples: tuple[SqlTestDifferenceSample, ...] = field(default_factory=tuple)
    missing_samples: tuple[SqlTestDifferenceSample, ...] = field(default_factory=tuple)
    column_differences: tuple[SqlTestColumnDifference, ...] = field(default_factory=tuple)
    error_code: str | None = None
    error_help: str | None = None
    error_message: str | None = None


@dataclass(frozen=True)
class SqlTestExecutionResult:
    """Outcome of one SQL unit test execution."""

    test_name: str
    outcome: SqlTestOutcome
    source_path: Path | None = None
    block_index: int | None = None
    parent_name: str | None = None
    case_name: str | None = None
    case_index: int | None = None
    case_fingerprint: str | None = None
    parameter_schema: tuple[SqlTestParameterDeclaration, ...] = field(default_factory=tuple)
    parameter_values: tuple[tuple[str, SqlValue], ...] = field(default_factory=tuple)
    step_results: tuple[StepResult, ...] = field(default_factory=tuple)
    error_code: str | None = None
    error_help: str | None = None
    error_message: str | None = None
    real_model_count: int | None = None
    timeout: ItemTimeoutReport | None = None
    assumptions: tuple[str, ...] = field(default_factory=tuple)
    """Named assumptions a local conversion relied on; empty on the warehouse."""
    local_sql: str | None = None
    """The DuckDB statement a local test ran; `None` on the warehouse."""


@dataclass(frozen=True)
class LocalTestEngine:
    """The SQLQuack executable, policy and caller session a local test run converts with."""

    executable: Path
    version: str
    policy: Path | None
    strict: bool
    session_parameters: tuple[tuple[str, str], ...] = ()
    session_refusal: str | None = None
    """Why the caller's session cannot be told to SQLQuack, refusing every test."""

    def describe(self) -> str:
        """One line naming the converter, its policy and mode."""

        policy: str = (
            str(self.policy) if self.policy is not None else "built-in (refuses every divergence)"
        )
        mode: str = "strict" if self.strict else "assumptions allowed"
        return f"SQLQuack {self.version} at {self.executable}; policy {policy}; {mode}"


@dataclass(frozen=True)
class SqlQuackRefusal:
    """Why SQLQuack refused one construct of a statement."""

    construct: str
    category: str
    reason: str

    def describe(self) -> str:
        """`construct (category): reason`; positions are in the rendered statement, so omitted."""

        return f"{self.construct} ({self.category}): {self.reason}"


@dataclass(frozen=True)
class SqlQuackConversion:
    """SQLQuack's report for one statement: DuckDB SQL, or the refusals."""

    converted: bool
    sql: str
    refusals: tuple[SqlQuackRefusal, ...] = ()
    assumptions: tuple[str, ...] = ()
    notes: tuple[str, ...] = ()


@dataclass(frozen=True)
class LocalStatementRun:
    """The rows one converted statement returned on DuckDB, or its error."""

    rows: tuple[Any, ...]
    error: str | None
    timed_out: bool
    columns: tuple[str, ...] = ()

    @property
    def description(self) -> tuple[tuple[str], ...]:
        """The column names in a DB-API cursor's description shape."""

        return tuple((column,) for column in self.columns)

    def fetchall(self) -> list[Any]:
        """The rows, as a DB-API cursor returns them."""

        return list(self.rows)


@dataclass(frozen=True)
class LocalTestOptions:
    """`sqb test --local`: the project whose SQLQuack policy applies, and strict mode."""

    project_dir: Path
    strict: bool = False
