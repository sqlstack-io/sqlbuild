"""Invocation-local context; only the fenced scheduler commits a tick's result."""

from dataclasses import dataclass


@dataclass(kw_only=True)
class SensorContext[EventT]:
    """The call's event, or `None` when polling, where assigning `cursor` proposes progress."""

    event: EventT
    target: str
    evaluation_id: str
    cursor: str | None = None
