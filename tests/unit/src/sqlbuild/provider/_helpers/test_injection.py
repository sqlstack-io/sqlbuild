from __future__ import annotations

import sys
from pathlib import Path
from types import ModuleType

import pytest

from sqlbuild.provider._helpers.injection import call_with_provider_injection
from sqlbuild.provider.classes.container import ProviderContainer
from sqlbuild.provider.classes.session import ProviderSession
from sqlbuild.provider.exceptions import ProviderInjectionError
from sqlbuild.providers import Provider
from tests.unit.src.sqlbuild.provider._helpers._test_types import (
    ProviderInjectionErrorTestCase,
    ProviderInjectionLabelTestCase,
    ProviderInjectionTestCase,
)
from tests.unit.src.sqlbuild.provider._helpers.helpers import (
    SlackProvider,
    context_and_provider,
    context_only,
    keyword_only_provider,
    load_module,
    mismatched_provider,
    missing_provider,
    positional_only_context,
    provider_only,
    reserved_context_conflict,
    unannotated_provider,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderInjectionTestCase(
            description="context-only function still receives context",
            function=context_only,
            expected_result="runtime-context",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_context_only_function_when_calling_with_provider_injection_then_context_is_preserved(
    test_case: ProviderInjectionTestCase,
) -> None:
    result: object = call_with_provider_injection(
        function=test_case.function,
        context="runtime-context",
        providers=ProviderSession({}).providers,
    )

    assert result == test_case.expected_result


@pytest.mark.parametrize(
    "test_case",
    (
        ProviderInjectionTestCase(
            description="provider is injected by parameter name with context",
            function=context_and_provider,
            expected_result=("runtime-context", "slack"),
        ),
        ProviderInjectionTestCase(
            description="provider-only function receives provider by name",
            function=provider_only,
            expected_result="slack",
        ),
        ProviderInjectionTestCase(
            description="keyword-only provider is injected by name",
            function=keyword_only_provider,
            expected_result=("runtime-context", "slack"),
        ),
        ProviderInjectionTestCase(
            description="positional-only context is passed by position",
            function=positional_only_context,
            expected_result=("runtime-context", "slack"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_provider_parameter_when_calling_with_provider_injection_then_provider_is_injected(
    test_case: ProviderInjectionTestCase,
) -> None:
    providers: ProviderContainer = ProviderSession({"slack_provider": SlackProvider()}).providers

    result: object = call_with_provider_injection(
        function=test_case.function,
        context="runtime-context",
        providers=providers,
    )

    assert result == test_case.expected_result


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderInjectionLabelTestCase(
            description="unannotated provider parameter uses name-based injection",
            function=unannotated_provider,
            expected_label="slack",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unannotated_provider_parameter_when_name_matches_then_provider_is_injected(
    test_case: ProviderInjectionLabelTestCase,
) -> None:
    providers: ProviderContainer = ProviderSession({"slack_provider": SlackProvider()}).providers

    result: object = call_with_provider_injection(
        function=test_case.function,
        context="runtime-context",
        providers=providers,
    )

    assert isinstance(result, SlackProvider)
    assert result.label == test_case.expected_label


@pytest.mark.parametrize(
    "test_case",
    (
        ProviderInjectionErrorTestCase(
            description="missing provider parameter errors clearly",
            function=missing_provider,
            expected_error_fragment=(
                "Provider parameter 'alerts' requires provider 'alerts', but it was not found"
            ),
        ),
        ProviderInjectionErrorTestCase(
            description="annotation mismatch errors clearly",
            function=mismatched_provider,
            expected_error_fragment=(
                "Provider parameter 'slack_provider' expected ClockProvider, "
                "but provider 'slack_provider' is SlackProvider"
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_provider_parameter_when_calling_then_errors_clearly(
    test_case: ProviderInjectionErrorTestCase,
) -> None:
    providers: ProviderContainer = ProviderSession({"slack_provider": SlackProvider()}).providers

    with pytest.raises(ProviderInjectionError, match=test_case.expected_error_fragment):
        call_with_provider_injection(
            function=test_case.function,
            context="runtime-context",
            providers=providers,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderInjectionErrorTestCase(
            description="provider cannot shadow reserved context parameter",
            function=reserved_context_conflict,
            expected_error_fragment=(
                "Provider name 'ctx' conflicts with reserved context parameter 'ctx'"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_name_conflicts_with_context_parameter_when_calling_then_errors_clearly(
    test_case: ProviderInjectionErrorTestCase,
) -> None:
    providers: ProviderContainer = ProviderSession({"ctx": SlackProvider()}).providers

    with pytest.raises(ProviderInjectionError, match=test_case.expected_error_fragment):
        call_with_provider_injection(
            function=test_case.function,
            context="runtime-context",
            providers=providers,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderInjectionErrorTestCase(
            description="same provider file imported through alias gives actionable error",
            function=lambda marker_provider: None,
            expected_error_fragment=(
                "Provider parameter 'marker_provider' is annotated with MarkerProvider imported as "
                "'alias_marker', but provider 'marker_provider' was discovered as "
                "providers.marker.MarkerProvider. Import project providers using the project-root "
                "providers package path"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_class_imported_through_alias_when_calling_then_guides_user(
    test_case: ProviderInjectionErrorTestCase,
    tmp_path: Path,
) -> None:
    provider_file: Path = tmp_path / "providers" / "marker.py"
    provider_file.parent.mkdir(parents=True)
    provider_file.write_text(
        "from sqlbuild.providers import Provider\n\nclass MarkerProvider(Provider):\n    '''Test provider MarkerProvider.'''\n    pass\n",
        encoding="utf-8",
    )
    old_path: list[str] = list(sys.path)
    sys.path.insert(0, str(tmp_path))
    try:
        canonical_module: ModuleType = load_module(
            module_name="providers.marker",
            file_path=provider_file,
        )
        alias_module: ModuleType = load_module(
            module_name="alias_marker",
            file_path=provider_file,
        )
        canonical_provider_class: type[Provider] = canonical_module.MarkerProvider
        alias_provider_class: type[Provider] = alias_module.MarkerProvider

        def marker_consumer(marker_provider: object) -> None:
            del marker_provider

        marker_consumer.__annotations__["marker_provider"] = alias_provider_class

        providers: ProviderContainer = ProviderSession(
            {"marker_provider": canonical_provider_class()}
        ).providers

        with pytest.raises(ProviderInjectionError, match=test_case.expected_error_fragment):
            call_with_provider_injection(
                function=marker_consumer,
                context="runtime-context",
                providers=providers,
            )
    finally:
        sys.path = old_path
        sys.modules.pop("providers.marker", None)
        sys.modules.pop("alias_marker", None)
