"""Test-case models for audit-factory authoring."""

from dataclasses import dataclass

from sqlbuild.compiler.auditing.types import ThresholdOperator
from sqlbuild.python_nodes.models import DiscoverDefinition, Item, RetryPolicy


@dataclass(frozen=True)
class AuditDecoratorTestCase:
    description: str
    expected_bare_name: str
    expected_called_name: str


@dataclass(frozen=True)
class AuditThresholdHelperTestCase:
    description: str
    lower: float
    upper: float
    expected_operator: ThresholdOperator


@dataclass(frozen=True)
class InvalidAuditCaseNameTestCase:
    description: str
    name: str
    expected_error_fragment: str


@dataclass(frozen=True)
class AuditCaseMetaTestCase:
    description: str
    meta: dict[str, object] | None
    expected_meta_json: str | None


@dataclass(frozen=True)
class InvalidAuditCaseMetaTestCase:
    description: str
    meta: object
    expected_error_fragment: str


@dataclass(frozen=True)
class RetryDelayTestCase:
    description: str
    policy: RetryPolicy
    retry_index: int
    expected_delay: float
    expected_type: type


@dataclass(frozen=True)
class JitteredRetryDelayTestCase:
    description: str
    policy: RetryPolicy
    retry_index: int
    seed: int
    expected_upper_bound: float


@dataclass(frozen=True)
class DiscoveredResultTestCase:
    description: str
    items: tuple[Item, ...]
    expected_items: list[dict[str, object]]


@dataclass(frozen=True)
class DiscoveredArgsErrorTestCase:
    description: str
    args: dict[str, object]
    expected_error_fragment: str


@dataclass(frozen=True)
class DiscoverAuthoringErrorTestCase:
    description: str
    expected_error_fragment: str


@dataclass(frozen=True)
class DiscoverDefinitionTestCase:
    description: str
    expected_definition: DiscoverDefinition
    expected_for_each: str
