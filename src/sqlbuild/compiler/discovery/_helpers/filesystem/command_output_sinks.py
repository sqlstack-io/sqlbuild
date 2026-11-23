"""Discovery and provider binding for command-output sinks."""

from __future__ import annotations

import inspect
from collections.abc import Callable
from pathlib import Path
from types import ModuleType
from typing import get_type_hints

import sqlbuild._native as _native
from sqlbuild._native import (
    DiscoveredCommandOutputSink,
    DiscoveredCommandOutputSinkDeclaration,
    DiscoveredEventExporter,
    DiscoveredProvider,
    DiscoveredProviderUsage,
    EventExporterDiscoveryError,
    RecallWriter,
)
from sqlbuild.compiler.discovery._helpers.filesystem.project_modules import (
    load_event_sink_module,
    provider_usages,
)
from sqlbuild.compiler.discovery._helpers.filesystem.recall_sinks import split_recall_sinks
from sqlbuild.runtime.event_exporting.main.get_command_output_event_sink_definition import (
    get_command_output_event_sink_definition,
)
from sqlbuild.runtime.output_capture.constants import COMMAND_OUTPUT_SINK_RECORD_PARAMETER_NAME
from sqlbuild.runtime.output_capture.models import CommandOutputRecord, CommandOutputEventSinkDefinition


def discover_command_output_sink_functions(
    *, project_dir: Path, providers: tuple[DiscoveredProvider, ...] = ()
) -> tuple[DiscoveredCommandOutputSink, ...]:
    """Discover validated command-output sink declarations under sinks/, without Recall's."""

    cached: (
        tuple[
            tuple[DiscoveredProvider, ...],
            tuple[DiscoveredEventExporter, ...],
            tuple[DiscoveredCommandOutputSink, ...],
        ]
        | None
    ) = _native.cached_event_exporter_extensions(project_dir=project_dir)
    if cached is not None:
        return cached[2]
    declarations: tuple[DiscoveredCommandOutputSinkDeclaration, ...] = (
        discover_command_output_sink_declarations(project_dir=project_dir)
    )
    _, ordinary = split_recall_sinks(
        sinks=bind_command_output_sink_declarations(
            declarations=declarations,
            providers=providers,
            project_dir=project_dir,
        ),
        history="command-output",
    )
    return ordinary


def discover_command_output_sink_declarations(
    *, project_dir: Path
) -> tuple[DiscoveredCommandOutputSinkDeclaration, ...]:
    """Import event sink modules and collect command-output declarations."""

    sinks_root: Path = project_dir / _native.EVENT_SINKS_PATH
    if not sinks_root.is_dir():
        return ()
    discovered: list[DiscoveredCommandOutputSinkDeclaration] = []
    seen_names: dict[str, Path] = {}
    for file_path in _native.discover_python_module_files(
        project_dir=project_dir, root=_native.EVENT_SINKS_PATH, public_only=True
    ):
        module: ModuleType = load_event_sink_module(file_path=file_path, project_dir=project_dir)
        seen_function_ids: set[int] = set()
        for _, value in inspect.getmembers(module, inspect.isfunction):
            if value.__module__ != module.__name__:
                continue
            function_id: int = id(value)
            if function_id in seen_function_ids:
                continue
            seen_function_ids.add(function_id)
            definition: CommandOutputEventSinkDefinition | None = get_command_output_event_sink_definition(
                value
            )
            if definition is None:
                continue
            existing_path: Path | None = seen_names.get(definition.name)
            if existing_path is not None:
                raise EventExporterDiscoveryError(
                    f"Duplicate command-output sink name '{definition.name}' found in "
                    f"{existing_path.relative_to(project_dir)} and "
                    f"{file_path.relative_to(project_dir)}"
                )
            _validate_command_output_sink_signature(
                function=value,
                sink_name=definition.name,
                file_path=file_path,
                project_dir=project_dir,
            )
            seen_names[definition.name] = file_path
            discovered.append(
                DiscoveredCommandOutputSinkDeclaration(
                    file_path=file_path,
                    relative_path=file_path.relative_to(project_dir),
                    name=definition.name,
                    function=value,
                    streams=definition.streams,
                )
            )
    return tuple(discovered)


def bind_command_output_sink_declarations(
    *,
    declarations: tuple[DiscoveredCommandOutputSinkDeclaration, ...],
    providers: tuple[DiscoveredProvider, ...],
    project_dir: Path,
) -> tuple[DiscoveredCommandOutputSink, ...]:
    """Validate command-output sink provider parameters."""

    provider_by_name: dict[str, DiscoveredProvider] = {
        provider.name: provider for provider in providers
    }
    bound: list[DiscoveredCommandOutputSink] = []
    for declaration in declarations:
        usages: tuple[DiscoveredProviderUsage, ...] = _bind_sink_provider_usages(
            function=declaration.function,
            sink_name=declaration.name,
            sink_label="Command-output sink",
            file_path=declaration.file_path,
            project_dir=project_dir,
            provider_by_name=provider_by_name,
        )
        bound.append(
            DiscoveredCommandOutputSink(
                file_path=declaration.file_path,
                relative_path=declaration.relative_path,
                name=declaration.name,
                function=declaration.function,
                streams=declaration.streams,
                provider_usages=usages,
            )
        )
    return tuple(bound)


def _validate_command_output_sink_signature(
    *,
    function: Callable[..., object],
    sink_name: str,
    file_path: Path,
    project_dir: Path,
) -> None:
    relative_path: Path = file_path.relative_to(project_dir)
    if inspect.iscoroutinefunction(function):
        raise EventExporterDiscoveryError(
            f"Command-output sink '{sink_name}' in {relative_path} must be synchronous"
        )
    parameters: tuple[inspect.Parameter, ...] = tuple(
        inspect.signature(function).parameters.values()
    )
    if not parameters or parameters[0].name != COMMAND_OUTPUT_SINK_RECORD_PARAMETER_NAME:
        raise EventExporterDiscoveryError(
            f"Command-output sink '{sink_name}' in {relative_path} must declare record first"
        )
    _validate_sink_parameter_shape(
        parameters=parameters,
        sink_name=sink_name,
        sink_label="Command-output sink",
        relative_path=relative_path,
    )
    try:
        type_hints: dict[str, object] = get_type_hints(function)
    except (NameError, TypeError) as error:
        raise EventExporterDiscoveryError(
            f"Command-output sink '{sink_name}' in {relative_path} has invalid annotations"
        ) from error
    record_annotation: object = type_hints.get("record", parameters[0].annotation)
    if record_annotation not in {inspect.Parameter.empty, CommandOutputRecord}:
        raise EventExporterDiscoveryError(
            f"Command-output sink '{sink_name}' in {relative_path} record must be "
            "CommandOutputRecord"
        )
    _validate_sink_return_annotation(
        function=function,
        type_hints=type_hints,
        sink_name=sink_name,
        sink_label="Command-output sink",
        relative_path=relative_path,
    )


def _validate_sink_parameter_shape(
    *,
    parameters: tuple[inspect.Parameter, ...],
    sink_name: str,
    sink_label: str,
    relative_path: Path,
) -> None:
    if any(
        parameter.kind
        in {
            inspect.Parameter.POSITIONAL_ONLY,
            inspect.Parameter.VAR_POSITIONAL,
            inspect.Parameter.VAR_KEYWORD,
        }
        for parameter in parameters
    ):
        raise EventExporterDiscoveryError(
            f"{sink_label} '{sink_name}' in {relative_path} must use named parameters"
        )
    if any(parameter.default is not inspect.Parameter.empty for parameter in parameters):
        raise EventExporterDiscoveryError(
            f"{sink_label} '{sink_name}' in {relative_path} parameters must not have defaults"
        )


def _validate_sink_return_annotation(
    *,
    function: Callable[..., object],
    type_hints: dict[str, object],
    sink_name: str,
    sink_label: str,
    relative_path: Path,
) -> None:
    return_annotation: object = type_hints.get(
        "return", inspect.signature(function).return_annotation
    )
    if return_annotation not in {inspect.Signature.empty, None, type(None)}:
        raise EventExporterDiscoveryError(
            f"{sink_label} '{sink_name}' in {relative_path} return annotation must be None"
        )


def _bind_sink_provider_usages(
    *,
    function: Callable[..., object],
    sink_name: str,
    sink_label: str,
    file_path: Path,
    project_dir: Path,
    provider_by_name: dict[str, DiscoveredProvider],
) -> tuple[DiscoveredProviderUsage, ...]:
    relative_path: Path = file_path.relative_to(project_dir)
    parameters: tuple[inspect.Parameter, ...] = tuple(
        inspect.signature(function).parameters.values()
    )
    type_hints: dict[str, object] = get_type_hints(function)
    for parameter in parameters[1:]:
        if type_hints.get(parameter.name, parameter.annotation) is RecallWriter:
            continue
        provider: DiscoveredProvider | None = provider_by_name.get(parameter.name)
        if provider is None:
            raise EventExporterDiscoveryError(
                f"{sink_label} '{sink_name}' in {relative_path} requires unknown provider "
                f"'{parameter.name}'"
            )
        annotation: object = type_hints.get(parameter.name, parameter.annotation)
        if annotation is not inspect.Parameter.empty and annotation is not provider.provider_class:
            provider_class_name: str = provider.provider_class.__name__
            raise EventExporterDiscoveryError(
                f"{sink_label} '{sink_name}' in {relative_path} provider parameter "
                f"'{parameter.name}' must be unannotated or exactly {provider_class_name}"
            )
    return provider_usages(function=function, provider_by_name=provider_by_name)
