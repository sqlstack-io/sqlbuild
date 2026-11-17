"""End-to-end tests for the release performance comparison against the previous release."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

from scripts.release_performance.constants import BENCHMARK_COMMANDS
from tests.e2e.scripts.release_performance._test_types import ReleasePerformanceTestCase
from tests.e2e.scripts.release_performance.helpers import (
    BASELINE_MARKER,
    write_marked_baseline_source,
)

_REPO_ROOT: Path = Path(__file__).resolve().parents[4]


@pytest.mark.parametrize(
    "test_case",
    (
        ReleasePerformanceTestCase(
            description="the same build as candidate and baseline reports every command",
            inspection_models=300,
            build_models=100,
            dense_models=20,
            expected_outcomes=((0, True), (1, False)),
            expected_fragments=(
                "| `compile (no cache)` |",
                "| `compile (warm cache)` |",
                "| `compile (one-model edit)` |",
                "| `dense compile (warm cache)` |",
                "| `dense compile (one-model edit)` |",
                "| `dense compile (no cache)` |",
                "| `plan --json` |",
                "| `build (empty warehouse)` |",
                "| `lineage column trace` |",
                "Benchmark projects are generated per side",
                "### Release performance progress",
                " UTC: Generating baseline benchmark projects with ",
                " UTC: Warming caches for candidate ",
                " UTC: Timing `dense compile (no cache)`",
                "**Result: ",
            ),
            expected_marked_projects=(
                "baseline/build",
                "baseline/build-template",
                "baseline/dense",
                "baseline/inspection",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_two_installations_when_comparing_release_performance_then_reports_each_command(
    test_case: ReleasePerformanceTestCase, tmp_path: Path
) -> None:
    summary: Path = tmp_path / "summary.md"
    evidence: Path = tmp_path / "evidence.json"
    environment: dict[str, str] = dict(os.environ)
    environment["GITHUB_STEP_SUMMARY"] = str(summary)
    installation: str = str(Path(sys.executable).parent.parent)
    baseline_source: Path = write_marked_baseline_source(
        repo_root=_REPO_ROOT, destination=tmp_path / "baseline-source"
    )

    result: subprocess.CompletedProcess[str] = subprocess.run(
        [
            sys.executable,
            "-m",
            "scripts.compare_release_performance",
            "--candidate",
            installation,
            "--baseline",
            installation,
            "--runs",
            "1",
            "--inspection-models",
            str(test_case.inspection_models),
            "--build-models",
            str(test_case.build_models),
            "--dense-models",
            str(test_case.dense_models),
            "--baseline-source",
            str(baseline_source),
            "--work-dir",
            str(tmp_path / "work"),
            "--output",
            str(evidence),
        ],
        cwd=_REPO_ROOT,
        capture_output=True,
        text=True,
        env=environment,
        check=False,
    )

    work: Path = tmp_path / "work"
    assert (
        tuple(
            sorted(
                marker.parent.relative_to(work).as_posix()
                for marker in work.glob(f"*/*/{BASELINE_MARKER}")
            )
        )
        == test_case.expected_marked_projects
    )
    output: str = result.stdout + result.stderr
    markdown: str = summary.read_text(encoding="utf-8")
    for fragment in test_case.expected_fragments:
        assert fragment in markdown
    assert (result.returncode, "**Result: passed.**" in markdown) in (
        test_case.expected_outcomes
    ), output
    recorded: dict[str, object] = json.loads(evidence.read_text(encoding="utf-8"))
    assert [command["name"] for command in recorded["commands"]] == [
        command.name for command in BENCHMARK_COMMANDS
    ]
    for command in recorded["commands"]:
        assert command["max_rss_ratio"] is None


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
