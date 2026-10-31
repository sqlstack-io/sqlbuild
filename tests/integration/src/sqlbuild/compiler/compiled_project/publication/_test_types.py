"""Publication cases and isolated server credentials."""

from dataclasses import dataclass
from typing import NamedTuple


class SnapshotApi(NamedTuple):
    url: str
    location_id: str
    bearer: str


@dataclass(frozen=True)
class SnapshotPublicationTestCase:
    description: str
    target: str
    expected_generation: int
    expected_nodes: frozenset[str]
