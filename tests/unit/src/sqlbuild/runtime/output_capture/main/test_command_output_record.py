from __future__ import annotations

import dataclasses

import pytest

from sqlbuild.event_sinks import CommandOutputRecord, CommandOutputValidationError
from tests.unit.src.sqlbuild.runtime.output_capture.main._test_types import (
    InvalidReplacementRecordTestCase,
    ReplacementRecordTestCase,
)
from tests.unit.src.sqlbuild.runtime.output_capture.main.helpers import command_output_record


@pytest.mark.parametrize(
    "test_case",
    (
        ReplacementRecordTestCase(
            description="new bytes keep the message",
            data=b"built \xffmodel\n",
            message=None,
            expected_message="built model\n",
        ),
        ReplacementRecordTestCase(
            description="new bytes and message",
            data=b"[redacted]\n",
            message="[redacted]\n",
            expected_message="[redacted]\n",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_a_version_2_record_when_replaced_then_the_copy_keeps_its_id_and_sequence(
    test_case: ReplacementRecordTestCase,
) -> None:
    record: CommandOutputRecord = dataclasses.replace(
        command_output_record(), data=b"built model\n", schema_version=2
    )

    replaced: CommandOutputRecord = record.replace(data=test_case.data, message=test_case.message)

    assert (replaced.record_id, replaced.sequence, replaced.schema_version) == (
        record.record_id,
        record.sequence,
        2,
    )
    assert (replaced.data, replaced.message) == (test_case.data, test_case.expected_message)


@pytest.mark.parametrize(
    "test_case",
    (
        InvalidReplacementRecordTestCase(
            description="an unknown schema version",
            changes={"schema_version": 3},
            expected_error="schema_version must be 1 or 2",
        ),
        InvalidReplacementRecordTestCase(
            description="text where bytes belong",
            changes={"data": "built model\n"},
            expected_error="data must be bytes",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_an_invalid_record_shape_when_created_then_validation_names_the_field(
    test_case: InvalidReplacementRecordTestCase,
) -> None:
    with pytest.raises(CommandOutputValidationError, match=test_case.expected_error):
        dataclasses.replace(command_output_record(), **test_case.changes)  # type: ignore[arg-type]


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
