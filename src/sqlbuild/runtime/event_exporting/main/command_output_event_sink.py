"""Declare command-output sinks."""

from __future__ import annotations

import re
from collections.abc import Callable, Iterable
from typing import Any, cast

from sqlbuild.python_nodes.main.attach_definition import attach_definition
from sqlbuild.runtime.event_exporting.constants import COMMAND_OUTPUT_SINK_ATTRIBUTE
from sqlbuild.runtime.event_exporting.exceptions import EventExporterInputError
from sqlbuild.runtime.output_capture.models import CommandOutputEventSinkDefinition
from sqlbuild.runtime.output_capture.types import CommandOutputStream


def command_output_event_sink(
    function: Callable[..., object] | None = None,
    *,
    name: str | None = None,
    streams: Iterable[CommandOutputStream | str] | None = None,
) -> Callable[..., object] | Callable[[Callable[..., object]], Callable[..., object]]:
    """Mark a synchronous function as an explicit command-output sink."""

    def decorate(inner: Callable[..., object]) -> Callable[..., object]:
        resolved_name: str = _sink_name(function=inner, explicit_name=name)
        resolved_streams: frozenset[CommandOutputStream] = _command_output_streams(streams)
        return attach_definition(
            function=inner,
            attribute_name=COMMAND_OUTPUT_SINK_ATTRIBUTE,
            definition=CommandOutputEventSinkDefinition(
                name=resolved_name,
                streams=resolved_streams,
            ),
        )

    return decorate(function) if function is not None else decorate


def _sink_name(*, function: Callable[..., object], explicit_name: str | None) -> str:
    inner_function: Any = cast(Any, function)
    resolved_name: str = explicit_name or inner_function.__name__
    if not re.fullmatch(r"[a-z_][a-z0-9_]*", resolved_name):
        raise EventExporterInputError("sink names must be lower snake_case Python identifiers")
    return resolved_name


def _command_output_streams(
    values: Iterable[CommandOutputStream | str] | None,
) -> frozenset[CommandOutputStream]:
    if values is None:
        return frozenset(CommandOutputStream)
    try:
        resolved: frozenset[CommandOutputStream] = frozenset(
            CommandOutputStream(value) for value in values
        )
    except (TypeError, ValueError) as error:
        raise EventExporterInputError(
            "command-output sink streams must contain only stdout or stderr"
        ) from error
    if not resolved:
        raise EventExporterInputError(
            "command-output sink streams must contain at least one stream"
        )
    return resolved
