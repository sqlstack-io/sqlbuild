from datetime import UTC, datetime

from sqlbuild.event_sinks import CommandOutputRecord, CommandOutputStream


def command_output_record() -> CommandOutputRecord:
    return CommandOutputRecord(
        invocation_id="invocation-1",
        run_id="run-1",
        sequence=7,
        occurred_at=datetime(2026, 9, 4, 12, 30, tzinfo=UTC),
        stream=CommandOutputStream.STDOUT,
        message="built model\n",
        external_context={"orchestrator": "dagster", "tags": ("uat", "scheduled")},
        chunk_index=1,
        chunk_count=2,
    )
