"""Project runtime extension discovery entrypoint."""

from dataclasses import replace
from pathlib import Path

import sqlbuild._native as _native
from sqlbuild._native import (
    DiscoveredCommandOutputSink,
    DiscoveredCommandOutputSinkDeclaration,
    DiscoveredEventExporter,
    DiscoveredEventExporterDeclaration,
    DiscoveredProvider,
    DiscoveredRuntimeExtensions,
    EventExportSeverity,
    LifecycleEventSinkFilterConfig,
    LifecycleEventSinksConfig,
    ProjectConfigError,
    bind_event_exporter_declarations,
    discover_event_exporter_declarations,
    discover_provider_classes,
)
from sqlbuild.compiler.discovery._helpers.filesystem.command_output_sinks import (
    bind_command_output_sink_declarations,
    discover_command_output_sink_declarations,
)
from sqlbuild.compiler.discovery._helpers.filesystem.recall_sinks import split_recall_sinks


def discover_runtime_extensions(*, project_dir: Path) -> DiscoveredRuntimeExtensions:
    """Discover providers, typed sinks, Recall replacement sinks and lifecycle delivery settings."""

    declarations: tuple[DiscoveredEventExporterDeclaration, ...] = (
        discover_event_exporter_declarations(project_dir=project_dir)
    )
    output_declarations: tuple[DiscoveredCommandOutputSinkDeclaration, ...] = (
        discover_command_output_sink_declarations(project_dir=project_dir)
    )
    if (
        not declarations
        and not output_declarations
        and not (project_dir / "sqlbuild_project.toml").exists()
    ):
        return DiscoveredRuntimeExtensions()
    config: LifecycleEventSinksConfig = _native.load_project_config_file(
        project_dir
    ).event_sinks.lifecycle
    declaration_names: frozenset[str] = frozenset(item.name for item in declarations)
    unknown_names: frozenset[str] = frozenset(config.named) - declaration_names
    if unknown_names:
        raise ProjectConfigError(
            "event_sinks.lifecycle.named contains unknown event sink(s): "
            + ", ".join(sorted(unknown_names))
        )
    if not declarations and not output_declarations:
        return DiscoveredRuntimeExtensions()
    providers: tuple[DiscoveredProvider, ...] = discover_provider_classes(
        project_dir=project_dir, secret_policy="defer"
    )
    exporters: tuple[DiscoveredEventExporter, ...] = bind_event_exporter_declarations(
        declarations=declarations,
        project_dir=project_dir,
        providers=providers,
    )
    output_sinks: tuple[DiscoveredCommandOutputSink, ...] = bind_command_output_sink_declarations(
        declarations=output_declarations,
        project_dir=project_dir,
        providers=providers,
    )
    recall_lifecycle_sink, lifecycle_sinks = split_recall_sinks(
        sinks=tuple(
            _apply_runtime_filter(exporter=exporter, config=config) for exporter in exporters
        ),
        history="lifecycle",
    )
    recall_output_sink, ordinary_output_sinks = split_recall_sinks(
        sinks=output_sinks, history="command-output"
    )
    return DiscoveredRuntimeExtensions(
        providers=providers,
        event_exporters=lifecycle_sinks,
        command_output_sinks=ordinary_output_sinks,
        lifecycle_shutdown_timeout_seconds=config.shutdown_timeout_seconds,
        recall_lifecycle_sink=recall_lifecycle_sink,
        recall_output_sink=recall_output_sink,
    )


def _apply_runtime_filter(
    *, exporter: DiscoveredEventExporter, config: LifecycleEventSinksConfig
) -> DiscoveredEventExporter:
    kinds: frozenset[str] = exporter.event_kinds
    severity: EventExportSeverity = exporter.min_severity
    filters: tuple[LifecycleEventSinkFilterConfig, ...] = (
        config.defaults,
        config.named.get(exporter.name, LifecycleEventSinkFilterConfig()),
    )
    for runtime_filter in filters:
        if runtime_filter.event_kinds is not None:
            kinds = kinds & runtime_filter.event_kinds
        severity = _native.stricter_event_export_severity(
            first=severity, second=runtime_filter.min_severity
        )
    return replace(exporter, event_kinds=kinds, min_severity=severity)
