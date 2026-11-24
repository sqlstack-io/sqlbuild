from __future__ import annotations

import json

import pytest

from sqlbuild.event_sinks import (
    CommandOutputRecord,
    CommandOutputValidationError,
    command_output_from_json,
    command_output_to_json,
)
from tests.unit.src.sqlbuild.runtime.output_capture.main._test_types import (
    CommandOutputJsonTestCase,
)
from tests.unit.src.sqlbuild.runtime.output_capture.main.helpers import command_output_record


@pytest.mark.parametrize(
    "test_case",
    (CommandOutputJsonTestCase("deterministic versioned envelope", 1),),
    ids=lambda case: case.description,
)
def test_given_command_output_record_when_serializing_then_wire_envelope_is_deterministic(
    test_case: CommandOutputJsonTestCase,
) -> None:
    record: CommandOutputRecord = command_output_record()

    first: str = command_output_to_json(record)
    second: str = command_output_to_json(record)
    payload: dict[str, object] = json.loads(first)

    assert first == second
    assert payload["schema_version"] == test_case.expected_value
    assert payload["record_type"] == "command_output"
    assert payload["record_id"] == "invocation-1:command-output:7"
    assert payload["occurred_at"] == "2026-09-04T12:30:00.000000Z"
    assert payload["external_context"] == {
        "orchestrator": "dagster",
        "tags": ["uat", "scheduled"],
    }


@pytest.mark.parametrize(
    "test_case",
    (CommandOutputJsonTestCase("exact round trip", True),),
    ids=lambda case: case.description,
)
def test_given_canonical_json_when_deserializing_then_round_trips_exactly(
    test_case: CommandOutputJsonTestCase,
) -> None:
    record: CommandOutputRecord = command_output_record()

    decoded: CommandOutputRecord = command_output_from_json(command_output_to_json(record))

    assert (decoded == record) is test_case.expected_value


@pytest.mark.parametrize(
    "test_case",
    (CommandOutputJsonTestCase("conflicting record identity", "record_id"),),
    ids=lambda case: case.description,
)
def test_given_conflicting_record_id_when_deserializing_then_rejects_envelope(
    test_case: CommandOutputJsonTestCase,
) -> None:
    payload: dict[str, object] = json.loads(command_output_to_json(command_output_record()))
    payload["record_id"] = "different"

    assert isinstance(test_case.expected_value, str)
    with pytest.raises(CommandOutputValidationError, match=test_case.expected_value):
        command_output_from_json(json.dumps(payload))


@pytest.mark.parametrize(
    "test_case",
    (CommandOutputJsonTestCase("unknown known-schema field", "fields"),),
    ids=lambda case: case.description,
)
def test_given_unknown_field_when_deserializing_then_rejects_known_schema(
    test_case: CommandOutputJsonTestCase,
) -> None:
    payload: dict[str, object] = json.loads(command_output_to_json(command_output_record()))
    payload["sql"] = "select sensitive_value"

    assert isinstance(test_case.expected_value, str)
    with pytest.raises(CommandOutputValidationError, match=test_case.expected_value):
        command_output_from_json(json.dumps(payload))


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
