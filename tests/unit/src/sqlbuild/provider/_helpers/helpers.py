"""Helpers for provider helper tests."""

from __future__ import annotations

import importlib.util
import sys
from importlib.machinery import ModuleSpec
from pathlib import Path
from types import ModuleType
from typing import ClassVar

from sqlbuild.providers import Provider


class SlackProvider(Provider):
    provider_name: ClassVar[str] = "slack_provider"
    label: str = "slack"


class ClockProvider(Provider):
    provider_name: ClassVar[str] = "clock_provider"
    label: str = "clock"


def context_only(ctx: object) -> object:
    return ctx


def context_and_provider(ctx: object, slack_provider: SlackProvider) -> tuple[object, str]:
    return ctx, slack_provider.label


def positional_only_context(ctx: object, /, *, slack_provider: SlackProvider) -> tuple[object, str]:
    return ctx, slack_provider.label


def keyword_only_provider(ctx: object, *, slack_provider: SlackProvider) -> tuple[object, str]:
    return ctx, slack_provider.label


def provider_only(slack_provider: SlackProvider) -> str:
    return slack_provider.label


def unannotated_provider(slack_provider: object) -> object:
    return slack_provider


def missing_provider(alerts: SlackProvider) -> None:
    return None


def mismatched_provider(slack_provider: ClockProvider) -> None:
    return None


def reserved_context_conflict(ctx: object) -> object:
    return ctx


def load_module(*, module_name: str, file_path: Path) -> ModuleType:
    spec: ModuleSpec | None = importlib.util.spec_from_file_location(module_name, file_path)
    assert spec is not None
    assert spec.loader is not None
    module: ModuleType = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    spec.loader.exec_module(module)
    return module
