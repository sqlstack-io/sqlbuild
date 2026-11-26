from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class LoadPlanTestCase:
    """A selection's plan: the load it runs, what it loads, and a line of SQLLoad's plan of it."""

    description: str
    selector: str
    expected_load: tuple[str, bool, tuple[str, ...]]
    expected_line_fragment: str


@dataclass(frozen=True)
class LoadBuildTestCase:
    """Files landed before each build, and the rows the model reads after the last."""

    description: str
    landed: tuple[tuple[tuple[str, str], ...], ...]
    expected_rows: tuple[tuple[int, str], ...]


@dataclass(frozen=True)
class MissingTableTestCase:
    """A model reading a source named after a table the load doesn't load."""

    description: str
    command: str
    expected_fragment: str
