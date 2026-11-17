from dataclasses import dataclass


@dataclass(frozen=True)
class NodeResultReadWriteIntegrationTestCase:
    description: str
    expected_payload: dict[str, object]
    expected_metadata: dict[str, object]


@dataclass(frozen=True)
class EarlierNodeResultLocationIntegrationTestCase:
    """A result left in an earlier location, results in the store's own, and which one is read."""

    description: str
    earlier_schema: str
    schema_preserved: bool
    new_run_ids: tuple[str, ...]
    expected_run_id: str | None
