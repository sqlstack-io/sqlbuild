"""Public Python-node authoring type declarations."""

from enum import StrEnum
from typing import NotRequired, TypedDict


class LoaderColumnSpec(TypedDict):
    """Column declaration accepted by the public loader decorator."""

    name: str
    type: NotRequired[str]
    nullable: NotRequired[bool]
    description: NotRequired[str]
    meta: NotRequired[dict[str, object]]


class PythonNodeColumnSpec(TypedDict):
    """Column declaration accepted by dataset-like Python node decorators."""

    name: str
    type: NotRequired[str]
    nullable: NotRequired[bool]
    description: NotRequired[str]
    meta: NotRequired[dict[str, object]]


class ColumnLineageRefSpec(TypedDict):
    """Graph-based upstream column reference accepted by Python node decorators."""

    node: str
    column: str


class PythonCheckSeverity(StrEnum):
    """Severity for Python check results."""

    ERROR = "error"
    WARN = "warn"


class NodeSetKind(StrEnum):
    """What a set dependency names: a job's selection or every node carrying a tag."""

    JOB = "job"
    TAG = "tag"


class SqlResourceRefKind(StrEnum):
    """Typed reference target for SQL graph resources."""

    MODEL = "model"
    SOURCE = "source"
    SEED = "seed"
