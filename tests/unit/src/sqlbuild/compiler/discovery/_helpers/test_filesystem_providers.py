from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import pytest

from sqlbuild._native import (
    DiscoveredProvider,
    ProviderDiscoveryError,
    ResourceIdentityError,
    discover_provider_classes,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers._test_types import (
    DiscoverProviderCacheIsolationTestCase,
    DiscoverProviderClassesErrorTestCase,
    DiscoverProviderClassesTestCase,
    DiscoverProviderEnvSettingsTestCase,
    DiscoverProviderSecretErrorTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    (
        DiscoverProviderClassesTestCase(
            description="discovers provider classes",
            repo_files={
                "providers/notifications.py": """
from typing import ClassVar

from sqlbuild.providers import Provider


class SlackProvider(Provider):
    channel: str = "#alerts"


class AlertsProvider(Provider):
    provider_name: ClassVar[str] = "alerts"
""".strip()
                + "\n",
                "providers/nested/clock.py": """
from sqlbuild.providers import Provider


class Clock(Provider):
    pass
""".strip()
                + "\n",
            },
            expected_provider_names=("clock", "alerts", "slack_provider"),
            expected_provider_paths=(
                "providers/nested/clock.py",
                "providers/notifications.py",
                "providers/notifications.py",
            ),
            expected_provider_class_names=("Clock", "AlertsProvider", "SlackProvider"),
        ),
        DiscoverProviderClassesTestCase(
            description="ignores private files init files imported classes and abstract classes",
            repo_files={
                "providers/__init__.py": """
from sqlbuild.providers import Provider


class PackageProvider(Provider):
    pass
""".strip()
                + "\n",
                "providers/_private.py": """
from sqlbuild.providers import Provider


class PrivateProvider(Provider):
    pass
""".strip()
                + "\n",
                "providers/nested/_private.py": """
from sqlbuild.providers import Provider


class NestedPrivateProvider(Provider):
    pass
""".strip()
                + "\n",
                "providers/shared.py": """
from abc import ABC, abstractmethod

from sqlbuild.providers import Provider


class ImportedProvider(Provider):
    pass


class AbstractProvider(Provider, ABC):
    @abstractmethod
    def send(self) -> None:
        raise NotImplementedError
""".strip()
                + "\n",
                "providers/public.py": """
from providers.shared import AbstractProvider, ImportedProvider
from sqlbuild.providers import Provider


class PublicProvider(Provider):
    pass
""".strip()
                + "\n",
            },
            expected_provider_names=("public_provider", "imported_provider"),
            expected_provider_paths=("providers/public.py", "providers/shared.py"),
            expected_provider_class_names=("PublicProvider", "ImportedProvider"),
        ),
        DiscoverProviderClassesTestCase(
            description="skips provider base class imported into provider module",
            repo_files={
                "providers/base_only.py": """
from sqlbuild.providers import Provider
""".strip()
                + "\n",
            },
            expected_provider_names=(),
            expected_provider_paths=(),
            expected_provider_class_names=(),
        ),
        DiscoverProviderClassesTestCase(
            description="does not call setup during discovery",
            repo_files={
                "providers/no_setup.py": """
from pathlib import Path

from sqlbuild.providers import Provider


class NoSetupProvider(Provider):
    def setup(self, ctx):
        Path(__file__).with_name("setup.marker").write_text("setup", encoding="utf-8")
""".strip()
                + "\n",
            },
            expected_provider_names=("no_setup_provider",),
            expected_provider_paths=("providers/no_setup.py",),
            expected_provider_class_names=("NoSetupProvider",),
            expected_marker_file_exists=False,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_provider_files_when_discovering_then_returns_provider_classes(
    test_case: DiscoverProviderClassesTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)

    providers: tuple[DiscoveredProvider, ...] = discover_provider_classes(project_dir=tmp_path)

    assert tuple(provider.name for provider in providers) == test_case.expected_provider_names
    assert tuple(str(provider.relative_path) for provider in providers) == (
        test_case.expected_provider_paths
    )
    assert tuple(provider.provider_class.__name__ for provider in providers) == (
        test_case.expected_provider_class_names
    )
    assert tuple(provider.provider_class.__module__ for provider in providers) == tuple(
        path.removesuffix(".py").replace("/", ".") for path in test_case.expected_provider_paths
    )
    assert tuple(provider.settings.__class__ for provider in providers) == tuple(
        provider.provider_class for provider in providers
    )
    assert (
        tmp_path / "providers" / "setup.marker"
    ).is_file() is test_case.expected_marker_file_exists


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoverProviderCacheIsolationTestCase(
            description="isolates provider package cache between project directories",
            first_repo_files={
                "providers/shared.py": """
from sqlbuild.providers import Provider


class FirstProvider(Provider):
    pass
""".strip()
                + "\n",
            },
            second_repo_files={
                "providers/shared.py": """
from sqlbuild.providers import Provider


class ImportedProvider(Provider):
    pass
""".strip()
                + "\n",
                "providers/public.py": """
from providers.shared import ImportedProvider
from sqlbuild.providers import Provider


class PublicProvider(Provider):
    pass
""".strip()
                + "\n",
            },
            expected_first_provider_names=("first_provider",),
            expected_second_provider_names=("public_provider", "imported_provider"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_previous_project_provider_package_when_discovering_then_uses_current_project_package(
    test_case: DiscoverProviderCacheIsolationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    first_project_dir: Path = tmp_path / "first"
    second_project_dir: Path = tmp_path / "second"
    write_repo_files(first_project_dir, test_case.first_repo_files)
    write_repo_files(second_project_dir, test_case.second_repo_files)

    first_providers: tuple[DiscoveredProvider, ...] = discover_provider_classes(
        project_dir=first_project_dir
    )
    second_providers: tuple[DiscoveredProvider, ...] = discover_provider_classes(
        project_dir=second_project_dir
    )

    assert tuple(provider.name for provider in first_providers) == (
        test_case.expected_first_provider_names
    )
    assert tuple(provider.name for provider in second_providers) == (
        test_case.expected_second_provider_names
    )


@pytest.mark.parametrize(
    "test_case",
    (
        DiscoverProviderClassesErrorTestCase(
            description="duplicate provider names fail",
            repo_files={
                "providers/first.py": """
from typing import ClassVar

from sqlbuild.providers import Provider


class FirstProvider(Provider):
    provider_name: ClassVar[str] = "alerts"
""".strip()
                + "\n",
                "providers/second.py": """
from typing import ClassVar

from sqlbuild.providers import Provider


class SecondProvider(Provider):
    provider_name: ClassVar[str] = "alerts"
""".strip()
                + "\n",
            },
            expected_error_fragment="Duplicate provider name 'alerts'",
            expected_error_type=ProviderDiscoveryError,
        ),
        DiscoverProviderClassesErrorTestCase(
            description="duplicate provider names in same file fail",
            repo_files={
                "providers/duplicates.py": """
from typing import ClassVar

from sqlbuild.providers import Provider


class FirstProvider(Provider):
    provider_name: ClassVar[str] = "alerts"


class SecondProvider(Provider):
    provider_name: ClassVar[str] = "alerts"
""".strip()
                + "\n",
            },
            expected_error_fragment="Duplicate provider name 'alerts' found in providers/duplicates.py",
            expected_error_type=ProviderDiscoveryError,
        ),
        DiscoverProviderClassesErrorTestCase(
            description="provider import failure identifies file",
            repo_files={
                "providers/broken.py": """
from missing_package import missing_provider
""".strip()
                + "\n",
            },
            expected_error_fragment="Failed to import provider file providers/broken.py",
            expected_error_type=ProviderDiscoveryError,
        ),
        DiscoverProviderClassesErrorTestCase(
            description="provider settings failure identifies provider",
            repo_files={
                "providers/slack.py": """
from pydantic import Field
from sqlbuild.providers import Provider


class SlackProvider(Provider):
    retries: int = Field("not a number", validate_default=True)
""".strip()
                + "\n",
            },
            expected_error_fragment=(
                "Provider 'slack_provider' in providers/slack.py has invalid settings"
            ),
            expected_error_type=ProviderDiscoveryError,
        ),
        DiscoverProviderClassesErrorTestCase(
            description="invalid provider name identifies provider",
            repo_files={
                "providers/slack.py": """
from typing import ClassVar

from sqlbuild.providers import Provider


class SlackProvider(Provider):
    provider_name: ClassVar[str] = "bad-name"
""".strip()
                + "\n",
            },
            expected_error_fragment="Invalid provider identity 'bad-name'",
            expected_error_type=ResourceIdentityError,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_provider_files_when_discovering_then_raises(
    test_case: DiscoverProviderClassesErrorTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)

    with pytest.raises(test_case.expected_error_type, match=test_case.expected_error_fragment):
        discover_provider_classes(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoverProviderEnvSettingsTestCase(
            description="env-backed settings resolve during discovery",
            repo_files={
                "providers/slack.py": """
from pydantic import Field

from sqlbuild.providers import Provider


class SlackProvider(Provider):
    webhook_url: str = Field(validation_alias="SLACK_WEBHOOK_URL")
""".strip()
                + "\n",
            },
            env_name="SLACK_WEBHOOK_URL",
            env_value="https://hooks.example/secret",
            expected_provider_name="slack_provider",
            expected_field_name="webhook_url",
            expected_field_value="https://hooks.example/secret",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_env_backed_provider_settings_when_discovering_then_instance_has_env_value(
    test_case: DiscoverProviderEnvSettingsTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)
    monkeypatch.setenv(test_case.env_name, test_case.env_value)

    providers: tuple[DiscoveredProvider, ...] = discover_provider_classes(project_dir=tmp_path)

    assert len(providers) == 1
    assert providers[0].name == test_case.expected_provider_name
    assert providers[0].settings.model_dump()[test_case.expected_field_name] == (
        test_case.expected_field_value
    )


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoverProviderSecretErrorTestCase(
            description="invalid plain env setting error redacts raw input value",
            repo_files={
                "providers/slack.py": """
from pydantic import Field

from sqlbuild.providers import Provider


class SlackProvider(Provider):
    token: int = Field(validation_alias="SLACK_TOKEN")
""".strip()
                + "\n",
            },
            env_name="SLACK_TOKEN",
            env_value="super-secret-token-value",
            expected_error_fragment=(
                "Provider 'slack_provider' in providers/slack.py has invalid settings"
            ),
            unexpected_error_fragment="super-secret-token-value",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_plain_env_setting_when_discovering_then_error_redacts_raw_value(
    test_case: DiscoverProviderSecretErrorTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)
    monkeypatch.setenv(test_case.env_name, test_case.env_value)

    with pytest.raises(ProviderDiscoveryError) as exc_info:
        discover_provider_classes(project_dir=tmp_path)

    error_message: str = str(exc_info.value)
    assert test_case.expected_error_fragment in error_message
    assert test_case.unexpected_error_fragment not in error_message
