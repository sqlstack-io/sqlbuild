"""Provider-aware Python callable invocation helpers."""

from __future__ import annotations

import inspect
from collections.abc import Callable
from pathlib import Path
from typing import TYPE_CHECKING, get_type_hints

from sqlbuild.provider.classes.container import ProviderContainer
from sqlbuild.provider.exceptions import ProviderInjectionError, ProviderLookupError

if TYPE_CHECKING:
    from sqlbuild.provider.classes.provider import Provider

_CONTEXT_PARAMETER_NAMES: frozenset[str] = frozenset({"ctx", "context", "_ctx", "hook_context"})


def call_with_provider_injection(
    *,
    function: Callable[..., object],
    context: object,
    providers: ProviderContainer | None = None,
    supplied_kwargs: dict[str, object] | None = None,
) -> object:
    """Call a Python node function with context and name-based provider injection."""

    signature: inspect.Signature = inspect.signature(function)
    try:
        type_hints: dict[str, object] = get_type_hints(function)
    except TypeError:
        type_hints = {}
    kwargs: dict[str, object] = dict(supplied_kwargs or {})
    context_bound: bool = False
    for parameter in signature.parameters.values():
        if parameter.kind in {
            inspect.Parameter.VAR_POSITIONAL,
            inspect.Parameter.VAR_KEYWORD,
        }:
            continue
        annotation: object = type_hints.get(parameter.name, parameter.annotation)
        if parameter.name in _CONTEXT_PARAMETER_NAMES:
            if parameter.name in kwargs:
                raise ProviderInjectionError(
                    f"Hook argument '{parameter.name}' conflicts with reserved context "
                    f"parameter '{parameter.name}'. Rename the hook argument; context "
                    "parameters are injected by SQLBuild."
                )
            if providers is not None and parameter.name in providers:
                raise ProviderInjectionError(
                    f"Provider name '{parameter.name}' conflicts with reserved context parameter "
                    f"'{parameter.name}'"
                )
            kwargs[parameter.name] = context
            context_bound = True
            continue
        if providers is not None and parameter.name in providers and parameter.name in kwargs:
            raise ProviderInjectionError(
                f"Hook argument '{parameter.name}' conflicts with provider injection for "
                f"parameter '{parameter.name}'. Rename the hook argument or remove it to let "
                "SQLBuild inject the provider."
            )
        provider: Provider | None = _provider_for_parameter(
            parameter=parameter,
            annotation=annotation,
            providers=providers,
        )
        if provider is not None:
            kwargs[parameter.name] = provider
            continue
        if parameter.name in kwargs:
            continue
        if not context_bound and parameter.default is inspect.Parameter.empty:
            kwargs[parameter.name] = context
            context_bound = True
    positional: list[object] = []
    for parameter in signature.parameters.values():
        if parameter.kind != inspect.Parameter.POSITIONAL_ONLY or parameter.name not in kwargs:
            break
        positional.append(kwargs.pop(parameter.name))
    return function(*positional, **kwargs)


def _provider_for_parameter(
    *,
    parameter: inspect.Parameter,
    annotation: object,
    providers: ProviderContainer | None,
) -> Provider | None:
    from sqlbuild.provider.classes.provider import Provider

    provider_annotation: type[Provider] | None = (
        annotation if isinstance(annotation, type) and issubclass(annotation, Provider) else None
    )
    expects_provider: bool = provider_annotation is not None
    if providers is None:
        if expects_provider:
            raise ProviderInjectionError(
                f"Provider parameter '{parameter.name}' requires provider '{parameter.name}', "
                "but no provider container is available"
            )
        return None
    if parameter.name not in providers:
        if expects_provider:
            raise ProviderInjectionError(
                f"Provider parameter '{parameter.name}' requires provider '{parameter.name}', "
                "but it was not found"
            )
        return None
    try:
        provider: Provider = providers[parameter.name]
    except ProviderLookupError as error:
        raise ProviderInjectionError(str(error)) from error
    if provider_annotation is not None and not isinstance(provider, provider_annotation):
        alias_message: str | None = _provider_alias_import_message(
            parameter_name=parameter.name,
            provider=provider,
            provider_annotation=provider_annotation,
        )
        if alias_message is not None:
            raise ProviderInjectionError(alias_message)
        raise ProviderInjectionError(
            f"Provider parameter '{parameter.name}' expected {provider_annotation.__name__}, "
            f"but provider '{parameter.name}' is {provider.__class__.__name__}"
        )
    return provider


def _provider_alias_import_message(
    *,
    parameter_name: str,
    provider: Provider,
    provider_annotation: type[Provider],
) -> str | None:
    provider_class: type[Provider] = provider.__class__
    if provider_class.__name__ != provider_annotation.__name__:
        return None
    provider_file: str | None = inspect.getsourcefile(provider_class)
    annotation_file: str | None = inspect.getsourcefile(provider_annotation)
    if provider_file is None or annotation_file is None:
        return None
    if Path(provider_file).resolve() != Path(annotation_file).resolve():
        return None
    return (
        f"Provider parameter '{parameter_name}' is annotated with "
        f"{provider_annotation.__name__} imported as "
        f"'{provider_annotation.__module__}', but provider '{parameter_name}' was discovered as "
        f"{provider_class.__module__}.{provider_class.__name__}. Import project providers using "
        "the project-root providers package path, for example: "
        "from providers.my_provider import MyProvider"
    )
