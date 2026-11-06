"""Invocation-local context; native fenced completion alone commits a proposed cursor."""

from dataclasses import dataclass


@dataclass(kw_only=True)
class SensorContext:
    """Cursor assignment proposes progress; failure or timeout retains the committed cursor."""

    cursor: str | None
    target: str
    evaluation_id: str
