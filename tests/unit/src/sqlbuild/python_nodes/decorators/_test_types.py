from dataclasses import dataclass

from sqlbuild.python_nodes.models import ColumnLineageRef, RetryPolicy
from sqlbuild.python_nodes.types import PythonCheckSeverity


@dataclass(frozen=True)
class TaskDecoratorMetadataTestCase:
    description: str
    expected_name: str
    expected_dep_count: int
    expected_tags: tuple[str, ...]
    expected_group: str | None
    expected_description: str | None
    expected_meta: dict[str, object] | None
    expected_retry: RetryPolicy | None = None


@dataclass(frozen=True)
class AssetDecoratorMetadataTestCase:
    description: str
    expected_name: str
    expected_dep_count: int
    expected_tags: tuple[str, ...]
    expected_group: str | None
    expected_description: str | None
    expected_meta: dict[str, object] | None
    expected_column_names: tuple[str, ...]
    expected_column_types: tuple[str | None, ...]
    expected_column_descriptions: tuple[str | None, ...]
    expected_column_lineage: dict[str, tuple[ColumnLineageRef, ...]]
    expected_retry: RetryPolicy | None = None


@dataclass(frozen=True)
class CheckDecoratorMetadataTestCase:
    description: str
    expected_name: str
    expected_dep_count: int
    expected_severity: PythonCheckSeverity
    expected_tags: tuple[str, ...]
    expected_group: str | None
    expected_description: str | None
    expected_meta: dict[str, object] | None


@dataclass(frozen=True)
class FactoryDecoratorMetadataTestCase:
    description: str
    expected_name: str


@dataclass(frozen=True)
class FactoryDecoratorErrorTestCase:
    description: str
    expected_error_fragment: str


@dataclass(frozen=True)
class HookReadsTestCase:
    description: str
    reads: object
    expected_reads: tuple[str, ...] = ()
    expected_error: str = ""


@dataclass(frozen=True)
class NodeSetDependencyTestCase:
    """A task's dependency on a job or tag, as stored and as it renders."""

    description: str
    dependency: object
    expected_dependency: object
    expected_text: str
