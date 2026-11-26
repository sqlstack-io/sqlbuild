"""A location's interpreter says what each of its event sinks delivered, for the location to report."""

import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    RuntimeSinkTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    runtime_sink_totals,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files

FAILING_SINK: str = (
    "from sqlbuild.event_sinks import LifecycleEvent, lifecycle_event_sink\n\n\n"
    "@lifecycle_event_sink\n"
    "def broker(event: LifecycleEvent) -> None:\n"
    "    raise RuntimeError('broker unavailable')\n\n\n"
    "@lifecycle_event_sink\n"
    "def archive(event: LifecycleEvent) -> None:\n"
    "    del event\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        RuntimeSinkTestCase(
            description="a sink that fails every event and one that delivers them",
            sink_source=FAILING_SINK,
            expected_sinks=(("archive", False), ("broker", True)),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_location_interpreter_with_event_sinks_when_asked_then_it_says_what_each_delivered(
    test_case: RuntimeSinkTestCase, tmp_path: Path
) -> None:
    write_files(
        project_dir=tmp_path,
        files={
            "sqlbuild_project.toml": (
                'name = "orders"\nadapter = "duckdb"\n'
                '[connections.local]\ndatabase = "warehouse.duckdb"\n'
                '[targets.development]\nschema = "main"\n'
            ),
            "sql/models/orders.sql": (
                "MODEL (description 'Orders.', materialized table);\nSELECT 1 AS id\n"
            ),
            "python/event_sinks/events.py": test_case.sink_source,
        },
    )
    _ = subprocess.run(["git", "init", "-q", str(tmp_path)], check=True, capture_output=True)
    totals: dict[str, Any] = runtime_sink_totals(
        tmp_path,
        lambda totals: all(
            sink["delivered"] + sink["failed"] == sink["accepted"] > 0 for sink in totals["sinks"]
        ),
        30,
    )
    assert sorted(
        (sink["name"], sink["failed"] == sink["accepted"] > 0) for sink in totals["sinks"]
    ) == list(test_case.expected_sinks), (test_case.description, totals)
