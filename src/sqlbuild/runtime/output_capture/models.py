"""Immutable output capture records and accounting."""

from __future__ import annotations

import dataclasses
from collections.abc import Callable, Mapping
from dataclasses import dataclass, field
from datetime import datetime

from sqlbuild._native import (
    COMMAND_OUTPUT_RECORD_TYPE,
    CURRENT_COMMAND_OUTPUT_SCHEMA_VERSION,
    freeze_command_output_record,
)
from sqlbuild.runtime.output_capture.constants import SQLBUILD_COMMAND_OUTPUT_PRODUCER_VERSION
from sqlbuild.runtime.output_capture.types import CommandOutputStream, OutputRecordPriority


@dataclass(frozen=True)
class CommandOutputRecord:
    """One ANSI-free bounded text chunk; schema version 2 adds its exact bytes in ``data``."""

    invocation_id: str
    sequence: int
    occurred_at: datetime
    stream: CommandOutputStream
    message: str
    external_context: Mapping[str, object]
    run_id: str | None = None
    chunk_index: int = 0
    chunk_count: int = 1
    priority: OutputRecordPriority = OutputRecordPriority.BULK
    record_type: str = COMMAND_OUTPUT_RECORD_TYPE
    dropped_records: int = 0
    schema_version: int = CURRENT_COMMAND_OUTPUT_SCHEMA_VERSION
    producer: str = "sqlbuild"
    producer_version: str = field(default=SQLBUILD_COMMAND_OUTPUT_PRODUCER_VERSION)
    data: bytes = b""

    def __post_init__(self) -> None:
        """Validate and freeze the known command-output wire contract."""

        object.__setattr__(self, "external_context", freeze_command_output_record(self))

    @property
    def record_id(self) -> str:
        """Return a deterministic destination key for this invocation sequence."""

        return f"{self.invocation_id}:command-output:{self.sequence}"

    def replace(
        self, *, data: bytes | None = None, message: str | None = None
    ) -> CommandOutputRecord:
        """Return a copy with new ``data`` or ``message`` that keeps the record ID and sequence."""

        return dataclasses.replace(
            self,
            data=self.data if data is None else data,
            message=self.message if message is None else message,
        )


@dataclass(frozen=True)
class CommandOutputCaptureSummary:
    """Best-effort bounded-delivery accounting."""

    accepted: int
    delivered: int
    failed: int
    dropped: int
    queue_depth: int
    queue_capacity: int
    flush_complete: bool


@dataclass(frozen=True)
class OutputCaptureContext:
    """Opaque integration-owned context for one command."""

    external_context: Mapping[str, object]


@dataclass(frozen=True)
class CommandOutputEventSinkDefinition:
    """Immutable metadata attached to a command-output sink function."""

    name: str
    streams: frozenset[CommandOutputStream]


@dataclass(frozen=True)
class BoundCommandOutputSink:
    """One command-output sink bound to command-scoped provider instances."""

    name: str
    function: Callable[..., object]
    provider_arguments: Mapping[str, object]
    streams: frozenset[CommandOutputStream]
