"""Test types for provider runtime session tests."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from sqlbuild.provider.classes.session import ProviderSession


@dataclass(frozen=True)
class ProviderContainerLookupTestCase:
    description: str
    lookup_name: str
    expected_provider_name: str
    expected_setup_events: tuple[str, ...]


@dataclass(frozen=True)
class ProviderContainerMissingTestCase:
    description: str
    lookup_name: str
    expected_error_fragment: str


@dataclass(frozen=True)
class ProviderSessionLifecycleTestCase:
    description: str
    access_names: tuple[str, ...]
    expected_events: tuple[str, ...]
    expected_setup_calls: int = 0
    expected_teardown_calls: int = 0


@dataclass(frozen=True)
class ProviderSessionErrorTestCase:
    description: str
    provider_name: str
    expected_error_fragment: str


@dataclass(frozen=True)
class ProviderSessionLeftOpenTestCase:
    description: str
    closes: tuple[Callable[[ProviderSession], None], ...]
    within_seconds: float
    expected_events: tuple[str, ...]
