from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class SqlMacroErrorTestCase:
    """One project whose SQL macros the CLI rejects, and the diagnostic it prints."""

    description: str
    files: dict[str, str]
    expected_code: str
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SqlMacroBuildTestCase:
    """One model calling a SQL macro, built on DuckDB, and the rows it holds."""

    description: str
    files: dict[str, str]
    query: str
    expected_rows: list[tuple[object, ...]]


@dataclass(frozen=True)
class SqlMacroCommandTestCase:
    """One project, one sqb command, and what its output and an optional file must hold."""

    description: str
    files: dict[str, str]
    command: tuple[str, ...]
    expected_returncode: int
    expected_fragments: tuple[str, ...]
    inspected_paths: tuple[str, ...] = ()


@dataclass(frozen=True)
class SqlMacroEditTestCase:
    """A built project whose macro file is edited, then rebuilt, and the rows it then holds."""

    description: str
    files: dict[str, str]
    edited_path: str
    replaced_text: tuple[str, str]
    query: str
    expected_rows: list[tuple[object, ...]]


@dataclass(frozen=True)
class SqlMacroFormatTestCase:
    """A project formatted, and how a formatted macro file then ends."""

    description: str
    files: dict[str, str]
    formatted_path: str
    expected_ending: str
    expected_start: str = ""


@dataclass(frozen=True)
class SqlMacroRecompileTestCase:
    """A compiled project whose macro file is edited, then compiled again, and the error then."""

    description: str
    files: dict[str, str]
    edited_path: str
    replaced_text: tuple[str, str]
    expected_code: str
    expected_fragment: str


@dataclass(frozen=True)
class SqlMacroCompileTestCase:
    """One project compiled, what its output and inspected files hold, and what they must not."""

    description: str
    files: dict[str, str]
    expected_returncode: int
    expected_fragments: tuple[str, ...]
    unexpected_fragments: tuple[str, ...] = ()
    inspected_paths: tuple[str, ...] = ()


@dataclass(frozen=True)
class SqlMacroLineageTestCase:
    """A column read through a SQL macro call, and the same column written inline."""

    description: str
    macro_files: dict[str, str]
    inline_files: dict[str, str]
    column: str
    expected_edge: str


@dataclass(frozen=True)
class SqlMacroWarmCompileTestCase:
    """A compiled project whose macro file is edited, then compiled warm, and what each prints."""

    description: str
    files: dict[str, str]
    edited_path: str
    replaced_text: tuple[str, str]
    expected_cold_fragment: str
    expected_warm_fragment: str


@dataclass(frozen=True)
class SqlMacroEquivalenceTestCase:
    """A project built with two models whose compiled SQL and rows must be the same."""

    description: str
    files: dict[str, str]
    compared_paths: tuple[str, str]
    expected_fragment: str
    compared_queries: tuple[str, str]
    expected_row_count: int


@dataclass(frozen=True)
class SqlMacroFormatPairTestCase:
    """A macro body reading slots and the same body written by hand, formatted together."""

    description: str
    slot_text: str
    hand_text: str
    expected_check_returncode: int


@dataclass(frozen=True)
class SqlMacroBodyRulesTestCase:
    """One project linting a SQL macro body, and the findings it must and must not print."""

    description: str
    files: dict[str, str]
    expected_returncode: int
    expected_fragments: tuple[str, ...]
    expected_absent_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SqlMacroWaveTestCase:
    """A chain of models, one per analysis wave, each calling a macro once with a wrong type."""

    description: str
    files: dict[str, str]
    wave_count: int
    expected_finding: str
