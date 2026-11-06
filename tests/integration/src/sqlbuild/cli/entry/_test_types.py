from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class HandoffTestCase:
    description: str
    args: tuple[str, ...]
    expected_exit_code: int


@dataclass(frozen=True)
class TextReuseTestCase:
    description: str
    args: tuple[str, ...]
    expected_notice: bytes


@dataclass(frozen=True)
class JsonReuseTestCase:
    description: str
    args: tuple[str, ...]
    expected_hits: int


@dataclass(frozen=True)
class EditedProjectTestCase:
    description: str
    args: tuple[str, ...]
    old_text: str
    new_text: str
    expected_misses: int
    expected_fragment: str


@dataclass(frozen=True)
class SelectorFileTestCase:
    description: str
    selectors: str
    expected_records: int


@dataclass(frozen=True)
class PythonStartsTestCase:
    description: str
    args: tuple[str, ...]
    expected_python_starts: tuple[int, int, int]


@dataclass(frozen=True)
class ReuseFileMutationTestCase:
    description: str
    prepare: Callable[[Path], Path]
    expected_misses: int
    expected_fragment: str


@dataclass(frozen=True)
class ParserErrorTestCase:
    """One rejected command line and the coded error line `sqb` ends with."""

    description: str
    args: tuple[str, ...]
    expected_usage: bytes
    expected_error: bytes


@dataclass(frozen=True)
class SelectorCompileTestCase:
    """One compile command line and the model its JSON output names."""

    description: str
    args: tuple[str, ...]
    expected_model: bytes


@dataclass(frozen=True)
class LaunchedTeardownTestCase:
    """One compile command line and the reuse hits its repeated launched run reports."""

    description: str
    args: tuple[str, ...]
    expected_hits: int


@dataclass(frozen=True)
class PythonFreeTestCase:
    """One command line `sqb` serves without Python, in a directory `prepare` sets up."""

    description: str
    args: tuple[str, ...]
    prepare: Callable[[Path], None]
    expected_exit_code: int
    expected_captures: int


@dataclass(frozen=True)
class PythonStartTestCase:
    """One command line `sqb` hands to Python because of the project `prepare` writes."""

    description: str
    args: tuple[str, ...]
    prepare: Callable[[Path], None]
    expected_python_starts: int


@dataclass(frozen=True)
class InterruptTestCase:
    """One Python-free command line interrupted while it waits on the project `prepare` writes."""

    description: str
    args: tuple[str, ...]
    prepare: Callable[[Path], None]
    expected_exit_code: int


@dataclass(frozen=True)
class BrokenPipeTestCase:
    """One Python-free command line whose stdout has no reader."""

    description: str
    args: tuple[str, ...]
    prepare: Callable[[Path], None]
    expected_exit_code: int
    expected_stderr_end: bytes


@dataclass(frozen=True)
class MachineOutputResetTestCase:
    """A successful machine-output command followed in-process by an early failure."""

    description: str
    first_args: tuple[str, ...]
    failing_args: tuple[str, ...]
    failing_extra_toml: str
    expected_error_code: str


@dataclass(frozen=True)
class NativeCostTestCase:
    """A persisted history request and its independently specified ordering."""

    description: str
    args: tuple[str, ...]
    expected_run_ids: tuple[str, ...]
