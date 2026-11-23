from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import pytest

from sqlbuild._native import (
    DiscoveredHookFunction,
    DiscoveredProvider,
    DiscoveredProviderUsage,
    PythonNodeDiscoveryError,
    discover_hook_functions,
    discover_provider_classes,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers._test_types import (
    DiscoverHookFunctionsErrorTestCase,
    DiscoverHookFunctionsTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    (
        DiscoverHookFunctionsTestCase(
            description="discovers decorated hook functions",
            repo_files={
                "sql/hooks/python/notifications.py": """
from sqlbuild.hooks import hook


@hook
def notify(ctx):
    \"\"\"Default notification hook.\"\"\"
    return None


@hook(name="notify_success", description="Explicit success hook")
def notify_success(ctx):
    return None


@hook()
def plain_call(ctx):
    return None


@hook(description="Description-only hook")
def description_only(ctx):
    return None


def undecorated(ctx):
    return None
""".strip()
                + "\n",
                "sql/hooks/python/nested/catalog.py": """
from sqlbuild.hooks import hook


@hook(name="catalog-publish")
def publish(ctx):
    return None
""".strip()
                + "\n",
            },
            expected_hook_names=(
                "catalog-publish",
                "description_only",
                "notify",
                "notify_success",
                "plain_call",
            ),
            expected_hook_paths=(
                "sql/hooks/python/nested/catalog.py",
                "sql/hooks/python/notifications.py",
                "sql/hooks/python/notifications.py",
                "sql/hooks/python/notifications.py",
                "sql/hooks/python/notifications.py",
            ),
            expected_hook_descriptions=(
                None,
                "Description-only hook",
                "Default notification hook.",
                "Explicit success hook",
                None,
            ),
            expected_function_names=(
                "publish",
                "description_only",
                "notify",
                "notify_success",
                "plain_call",
            ),
        ),
        DiscoverHookFunctionsTestCase(
            description="ignores private files and init files",
            repo_files={
                "sql/hooks/python/__init__.py": """
from sqlbuild.hooks import hook


@hook
def package_hook(ctx):
    return None
""".strip()
                + "\n",
                "sql/hooks/python/_private.py": """
from sqlbuild.hooks import hook


@hook
def private_hook(ctx):
    return None
""".strip()
                + "\n",
                "sql/hooks/python/public.py": """
from sqlbuild.hooks import hook


@hook
def public_hook(ctx):
    return None
""".strip()
                + "\n",
            },
            expected_hook_names=("public_hook",),
            expected_hook_paths=("sql/hooks/python/public.py",),
            expected_hook_descriptions=(None,),
            expected_function_names=("public_hook",),
        ),
        DiscoverHookFunctionsTestCase(
            description="does not execute hook functions during discovery",
            repo_files={
                "sql/hooks/python/no_execute.py": """
from pathlib import Path
from sqlbuild.hooks import hook


@hook
def no_execute(ctx):
    Path(__file__).with_name("executed.marker").write_text("executed", encoding="utf-8")
""".strip()
                + "\n",
            },
            expected_hook_names=("no_execute",),
            expected_hook_paths=("sql/hooks/python/no_execute.py",),
            expected_hook_descriptions=(None,),
            expected_function_names=("no_execute",),
            expected_marker_file_exists=False,
        ),
        DiscoverHookFunctionsTestCase(
            description="ignores imported decorated hooks",
            repo_files={
                "sql/hooks/python/shared.py": """
from sqlbuild.hooks import hook


@hook(name="imported")
def imported_hook(ctx):
    return None
""".strip()
                + "\n",
                "sql/hooks/python/uses_import.py": """
from sqlbuild.hooks import hook
from hooks.python.shared import imported_hook


@hook
def local_hook(ctx):
    return imported_hook
""".strip()
                + "\n",
            },
            expected_hook_names=("imported", "local_hook"),
            expected_hook_paths=("sql/hooks/python/shared.py", "sql/hooks/python/uses_import.py"),
            expected_hook_descriptions=(None, None),
            expected_function_names=("imported_hook", "local_hook"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_hook_files_when_discovering_then_returns_decorated_hooks(
    test_case: DiscoverHookFunctionsTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)

    hooks: tuple[DiscoveredHookFunction, ...] = discover_hook_functions(project_dir=tmp_path)

    assert tuple(hook.name for hook in hooks) == test_case.expected_hook_names
    assert tuple(str(hook.relative_path) for hook in hooks) == test_case.expected_hook_paths
    assert tuple(hook.description for hook in hooks) == test_case.expected_hook_descriptions
    function_name_attr: str = "__name__"
    assert tuple(getattr(hook.function, function_name_attr) for hook in hooks) == (
        test_case.expected_function_names
    )
    assert all(callable(hook.function) for hook in hooks)
    assert (
        tmp_path / "sql" / "hooks" / "python" / "nodes" / "executed.marker"
    ).is_file() is test_case.expected_marker_file_exists


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoverHookFunctionsTestCase(
            description="records hook provider usage metadata",
            repo_files={
                "python/providers/marker.py": """
from sqlbuild.providers import Provider


class MarkerProvider(Provider):
    pass
""".strip()
                + "\n",
                "sql/hooks/python/marker.py": """
from python.providers.marker import MarkerProvider
from sqlbuild.hooks import hook


@hook
def mark(ctx, marker_provider: MarkerProvider):
    return None
""".strip()
                + "\n",
            },
            expected_hook_names=("mark",),
            expected_hook_paths=("sql/hooks/python/marker.py",),
            expected_hook_descriptions=(None,),
            expected_function_names=("mark",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_hook_provider_parameter_when_discovering_then_records_provider_usage(
    test_case: DiscoverHookFunctionsTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)
    providers: tuple[DiscoveredProvider, ...] = discover_provider_classes(project_dir=tmp_path)

    hooks: tuple[DiscoveredHookFunction, ...] = discover_hook_functions(
        project_dir=tmp_path,
        providers=providers,
    )

    assert tuple(hook.name for hook in hooks) == test_case.expected_hook_names
    assert len(hooks[0].provider_usages) == 1
    usage: DiscoveredProviderUsage = hooks[0].provider_usages[0]
    assert usage.provider_name == "marker_provider"
    assert usage.parameter_name == "marker_provider"
    assert usage.annotation_class_name == "MarkerProvider"
    assert usage.annotation_module == "python.providers.marker"


@pytest.mark.parametrize(
    "test_case",
    (
        DiscoverHookFunctionsErrorTestCase(
            description="duplicate hook names fail",
            repo_files={
                "sql/hooks/python/first.py": """
from sqlbuild.hooks import hook


@hook(name="notify")
def notify_a(ctx):
    return None
""".strip()
                + "\n",
                "sql/hooks/python/second.py": """
from sqlbuild.hooks import hook


@hook(name="notify")
def notify_b(ctx):
    return None
""".strip()
                + "\n",
            },
            expected_error_fragment="Duplicate hook name 'notify'",
        ),
        DiscoverHookFunctionsErrorTestCase(
            description="hook import failure identifies file",
            repo_files={
                "sql/hooks/python/broken.py": """
from missing_package import missing_hook
""".strip()
                + "\n",
            },
            expected_error_fragment="Failed to import Python node file sql/hooks/python/broken.py",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_hook_files_when_discovering_then_raises(
    test_case: DiscoverHookFunctionsErrorTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)

    with pytest.raises(PythonNodeDiscoveryError, match=test_case.expected_error_fragment):
        discover_hook_functions(project_dir=tmp_path)
