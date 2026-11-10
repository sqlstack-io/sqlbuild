from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class FunctionReturnContractIdentityTestCase:
    description: str
    original_type: str
    changed_type: str
    expected_changed: bool


@dataclass(frozen=True)
class FunctionUpstreamIdentityTestCase:
    description: str
    original_query: str
    changed_query: str
    expected_changed: bool


@dataclass(frozen=True)
class VersionIdentityConfigTestCase:
    description: str
    config_key: str
    expected_in_identity: bool


@dataclass(frozen=True)
class CursorRoleIdentityTestCase:
    description: str
    original_config: dict[str, object]
    changed_config: dict[str, object]
    expected_equal: bool


@dataclass(frozen=True)
class HookIdentityTestCase:
    description: str
    original_hooks: dict[str, object]
    changed_hooks: dict[str, object]
    expected_changed: bool


@dataclass(frozen=True)
class HookIdentityPayloadTestCase:
    description: str
    hooks: dict[str, object]
    expected_fragments: tuple[str, ...]
    forbidden_fragments: tuple[str, ...]


@dataclass(frozen=True)
class PersistedIdentityTestCase:
    description: str
    build: Callable[[Path], object]
    expected_identity: object


@dataclass(frozen=True)
class DeepChainIdentityTestCase:
    description: str
    depth: int
    expected_hash_count: int


@dataclass(frozen=True)
class RecordedFingerprintGraphTestCase:
    description: str
    orders_sql: str
    summary_sql: str
    recorded_definitions: dict[str, str]
    expected_kept: frozenset[str]


@dataclass(frozen=True)
class RecordedFunctionHashTestCase:
    description: str
    body_sql: str
    recorded_body_sql: str
    expected_recorded_local_hash: bool
