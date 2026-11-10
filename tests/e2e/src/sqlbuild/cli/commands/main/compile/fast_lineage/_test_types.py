from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class NativeFastLineageCliTestCase:
    """Compiling and tracing fast lineage of one project through the CLI."""

    description: str
    files: dict[str, str]
    lineage_targets: tuple[str, ...]
    expected_lineage: dict[str, dict[str, object]]
    expected_minimum_traced_edges: int


@dataclass(frozen=True)
class WarmFastLineageCliTestCase:
    """Fast lineage traced after a cold compile filled the compile's stores."""

    description: str
    reuse_disabled: str
    expected_minimum_cache_hits: int


@dataclass(frozen=True)
class NativeRichLineageFailureCliTestCase:
    """A native rich lineage failure injected at the binding, compiled through the CLI."""

    description: str
    files: dict[str, str]
    command: tuple[str, ...]
    expected_returncode: int
    expected_error_line: str


@dataclass(frozen=True)
class NativeRichLineageCliTestCase:
    """Compiling and tracing rich lineage of one project through the CLI."""

    description: str
    files: dict[str, str]
    lineage_targets: tuple[str, ...]
    expected_lineage: dict[str, dict[str, object]]
    expected_minimum_traced_edges: int
