"""Public typed references for SQL graph resources."""

from sqlbuild.python_nodes.models import NodeSetRef as NodeSetRef
from sqlbuild.python_nodes.models import SqlResourceRef as SqlResourceRef
from sqlbuild.python_nodes.types import NodeSetKind as NodeSetKind
from sqlbuild.python_nodes.types import SqlResourceRefKind as SqlResourceRefKind

__all__ = (
    "NodeSetKind",
    "NodeSetRef",
    "SqlResourceRef",
    "SqlResourceRefKind",
    "job",
    "model",
    "seed",
    "source",
    "tag",
)


def model(name: str) -> SqlResourceRef:
    """Return a typed reference to a SQLBuild model."""

    return SqlResourceRef(kind=SqlResourceRefKind.MODEL, name=name)


def source(name: str) -> SqlResourceRef:
    """Return a typed reference to a SQLBuild source."""

    return SqlResourceRef(kind=SqlResourceRefKind.SOURCE, name=name)


def seed(name: str) -> SqlResourceRef:
    """Return a typed reference to a SQLBuild seed."""

    return SqlResourceRef(kind=SqlResourceRefKind.SEED, name=name)


def job(name: str, *, exclude: str | None = None) -> NodeSetRef:
    """Depend on a job's end nodes: its selection less `exclude` that nothing else in it reads."""

    return NodeSetRef(kind=NodeSetKind.JOB, name=name, exclude=exclude)


def tag(name: str, *, exclude: str | None = None) -> NodeSetRef:
    """Depend on the end nodes of every node carrying the tag, less `exclude`."""

    return NodeSetRef(kind=NodeSetKind.TAG, name=name, exclude=exclude)
