"""Guard that a location previews a `--changed` build of the 10K-model DuckShop project quickly.

The location plans from the server's synced copy of a warehouse that built the project: about 13K
relations, a fingerprint and `--changed` history per built model, written by
`benchmarks/duckshop-10k/synced_state.py` with the hashes the planner computes for the project, so
only the edited models and what reads them run. A quadratic lookup over those 10K models or 13K
relations, in planning or in the copy it plans from, shows here first.

Each case also plans the project live, as `sqb plan` does, against an offline Snowflake warehouse
holding the same synced catalog, a few sources carrying an undeclared column. It bounds the models
analysed again against inspected source columns, the warehouse statements sent, cursor batching
and the live plan's time as a multiple of the location's publish, which compiles the same
project on the same machine.

It runs in the 10,000-model cell of the dense compile job, after the compile guards.
"""

import json
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.plan._test_types import PreviewPerformanceTestCase
from tests.e2e.src.sqlbuild.cli.commands.main.plan.helpers import (
    DUCKSHOP_10K_GENERATOR,
    DUCKSHOP_10K_SETUP_SECONDS,
    DUCKSHOP_10K_TARGET,
    PREVIEW_DONE,
    LivePlan,
    live_plan,
    mirror_synced_state,
    published_keys,
    served_duckshop_location,
    timed_plan,
    wait_for_log_count,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.person.helpers import (
    person_login,
    person_request,
    provision_person,
)

#: Cursor reads one batched statement answers.
_CURSOR_BATCH_SIZE: int = 50


@pytest.mark.performance
@pytest.mark.preview_performance
@pytest.mark.parametrize(
    "test_case",
    (
        PreviewPerformanceTestCase(
            description="every model built, one in a hundred edited since",
            built_share=1.0,
            expected_unbuilt=(0, 0),
            expected_definition_changes=(40, 200),
            expected_max_warm_seconds=10.0,
            expected_max_warm_to_publish=0.45,
            expected_max_check_seconds=0.5,
            expected_max_live_to_publish=1.8,
            expected_source_rebinding=(25, 150),
            expected_max_warehouse_statements=1_150,
        ),
        PreviewPerformanceTestCase(
            description="one model in twenty never built",
            built_share=0.95,
            expected_unbuilt=(300, 700),
            expected_definition_changes=(40, 200),
            expected_max_warm_seconds=10.0,
            expected_max_warm_to_publish=0.45,
            expected_max_check_seconds=0.5,
            expected_max_live_to_publish=1.8,
            expected_source_rebinding=(25, 150),
            expected_max_warehouse_statements=1_150,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_a_synced_10k_model_warehouse_when_previewing_a_changed_build_then_the_warm_preview_is_fast(
    test_case: PreviewPerformanceTestCase,
    duckshop_10k: tuple[Path, Path, Path],
    tmp_path: Path,
) -> None:
    project, capture, published = duckshop_10k
    batches_path: Path = tmp_path / "synced_state.json"
    written: subprocess.CompletedProcess[str] = subprocess.run(
        (
            sys.executable,
            str(DUCKSHOP_10K_GENERATOR / "synced_state.py"),
            str(published),
            str(capture),
            str(batches_path),
            "--built-share",
            str(test_case.built_share),
        ),
        capture_output=True,
        text=True,
        check=False,
        timeout=DUCKSHOP_10K_SETUP_SECONDS,
    )
    assert written.returncode == 0, written.stdout + written.stderr
    batches: dict[str, list[dict[str, Any]]] = json.loads(batches_path.read_text())
    with served_duckshop_location(tmp_path / "served", project) as served:
        mirror_synced_state(served, batches)
        selection: list[str] = published_keys(served.control)
        cold, _ = timed_plan(served, selection, {"changed": True})
        warm, document = timed_plan(served, selection, {"changed": True})
        _ = wait_for_log_count(served, PREVIEW_DONE, 2, 60.0)
        admin: dict[str, Any] = provision_person(
            served.control.parents[1], "create-user", "plan-admin"
        )
        session: tuple[str, str] = person_login(served.api, admin["name"])
        scope: dict[str, Any] = {
            "location_id": served.api.location_id,
            "target": DUCKSHOP_10K_TARGET,
        }
        checks: list[float] = []
        for _ in range(5):
            started: float = time.monotonic()
            check: dict[str, Any] = person_request(
                served.api,
                "/api/control/runs/adhoc/check",
                body=scope | {"command": ["build", "--changed"], "priority": 0},
                session=session,
            )
            checks.append(time.monotonic() - started)
    live: LivePlan = live_plan(project, batches["catalog"])
    reasons: list[str] = [run["reason"] for run in document["changed"]["runs"]]
    unbuilt: int = reasons.count("relation missing")
    definition_changes: int = sum(
        reason.startswith(("definition changed", "planner change")) for reason in reasons
    )
    check_seconds: float = sorted(checks)[len(checks) // 2]
    warm_to_publish: float = warm / served.published_seconds
    print(
        f"duckshop 10k preview ({test_case.description}): cold={cold:.2f}s warm={warm:.2f}s "
        f"publish={served.published_seconds:.2f}s warm_to_publish={warm_to_publish:.2f} "
        f"check_median={check_seconds * 1000:.0f}ms runs={len(reasons)} unbuilt={unbuilt} "
        f"definition_changes={definition_changes} models={len(document['models'])} "
        f"live={live.seconds:.2f}s live_to_publish={live.seconds / served.published_seconds:.2f} "
        f"live_entries={live.model_entries} rebinding={live.source_rebinding_analyses} "
        f"cursor_relations={live.cursor_relations} cursor_statements={live.cursor_statements} "
        f"warehouse_statements={live.warehouse_statements} "
        f"warm_steps={json.dumps(document.get('timings', {}).get('steps'))}",
        flush=True,
    )
    assert document.get("error") is None and len(document["models"]) >= 10_000
    assert check["error"] is None
    assert test_case.expected_unbuilt[0] <= unbuilt <= test_case.expected_unbuilt[1], (
        "the synced state builds what the case says"
    )
    assert (
        test_case.expected_definition_changes[0]
        <= definition_changes
        <= test_case.expected_definition_changes[1]
    ), "only the edited models changed definition: the synced hashes match the planner's"
    assert warm < test_case.expected_max_warm_seconds, (
        "a warm preview of 10K models must stay under ten seconds"
    )
    assert warm_to_publish < test_case.expected_max_warm_to_publish, (
        "planning grew faster than compiling: look for a lookup over every model or relation"
    )
    assert check_seconds < test_case.expected_max_check_seconds
    assert live.model_entries >= 10_000
    assert (
        test_case.expected_source_rebinding[0]
        <= live.source_rebinding_analyses
        <= test_case.expected_source_rebinding[1]
    ), "only models whose result a source's new or changed columns can change are analysed again"
    assert live.cursor_statements == -(-live.cursor_relations // _CURSOR_BATCH_SIZE), (
        "cursor bounds are read in full batches"
    )
    assert live.warehouse_statements <= test_case.expected_max_warehouse_statements
    assert live.seconds / served.published_seconds < test_case.expected_max_live_to_publish, (
        "a live plan grew faster than compiling and publishing the same project"
    )
