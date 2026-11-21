"""Test case types for the public custom-rule SQL document."""

from dataclasses import dataclass


@dataclass(frozen=True)
class DeepTreeTestCase:
    """A deeply nested query whose syntax tree a rule reads and then frees."""

    description: str
    dialect: str
    terms: int
    expected_node_count: int


@dataclass(frozen=True)
class FilePositionTimeLimitTestCase:
    """A wide model query whose every column a rule reports at its model-file position."""

    description: str
    dialect: str
    columns: int
    line_offset: int
    expected_max_seconds: float


@dataclass(frozen=True)
class UnplacedNodeTestCase:
    """A node-like value a rule reports, placed in a model file only with integer positions."""

    description: str
    line: object
    column: object
    expected_position: tuple[int, int] | None


@dataclass(frozen=True)
class ClauseTreeTestCase:
    """A query whose clause nodes a rule walks, with each one's children as `kind line:column`."""

    description: str
    dialect: str
    sql: str
    expected_clauses: dict[str, tuple[str, ...]]


@dataclass(frozen=True)
class CommonStructureKindTestCase:
    """A query whose common structures a rule reads, each labelled with the syntax tree's kind."""

    description: str
    dialect: str
    sql: str
    expected_kinds: tuple[str, ...]
