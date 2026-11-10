"""Private failure evidence must not change comparison acceptance or successful runs."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

import pytest

from scripts.delta_compile_series.models import DeltaCompileSeries
from tests.unit.scripts.delta_compile_series.main._test_types import ComparisonEvidenceTestCase
from tests.unit.scripts.delta_compile_series.main.helpers import run_comparison


@pytest.mark.parametrize(
    "test_case",
    [ComparisonEvidenceTestCase(description="private paired evidence")],
    ids=lambda case: case.description,
)
def test_given_mismatch_when_capturing_then_keeps_private_paired_evidence(
    tmp_path: Path, test_case: ComparisonEvidenceTestCase
) -> None:
    destination: Path = tmp_path / "private"
    result: DeltaCompileSeries = run_comparison(
        root=tmp_path / "run", destination=destination, test_case=test_case
    )

    assert len(result.unexpected) == test_case.expected_mismatches
    assert (
        "stderr differs from a full compile: '' vs 'SQL test planning  START'"
        in result.unexpected[0]
    )
    assert result.unexpected[0].endswith(
        f"; SQL test cache: delta none logged; full {test_case.full_cache_log}"
    )
    directory: Path = destination / "failure-00"
    manifest: dict[str, Any] = json.loads((directory / "manifest.json").read_text())
    before: dict[str, Any] = json.loads((directory / "cache-before.json").read_text())
    after: dict[str, Any] = json.loads((directory / "cache-after.json").read_text())
    key = "target/cache/compiler-native-v1/sql-test-artifacts.json"
    assert manifest["seed"] == 16
    assert manifest["case"] == "orders comparison"
    assert manifest["step"].startswith("step 0: column_description")
    assert manifest["runs"]["full"]["command"] == ["sqb", "compile", "--json"]
    assert manifest["runs"]["full"]["environment"] == [
        ["SQLBUILD_DELTA_COMPILE_DISABLE", "1"],
        ["PASSWORD", "[REDACTED]"],
    ]
    assert "synthetic-environment-secret" not in (directory / "manifest.json").read_text()
    assert manifest["runs"]["delta"]["returncode"] == manifest["runs"]["full"]["returncode"] == 0
    assert (
        manifest["runs"]["delta"]["semantic_json_sha256"]
        == manifest["runs"]["full"]["semantic_json_sha256"]
    )
    assert before["full"]["caches"][key]["parseable"] is True
    assert after["full"]["caches"][key]["parseable"] is False
    physical: dict[str, Any] = before["delta"]["caches"][key]["physical_artifacts"]["orders"]
    assert physical["recorded_digest_matches"] is True
    assert physical["recorded_mtime_matches"] is True
    assert physical["recorded_identity"] == "original-identity"
    assert (directory / "delta.stdout").read_text() == '{"models": ["orders"]}\n'
    assert "synthetic-secret" not in (directory / "full.stderr").read_text()
    assert manifest["files"]["full.stderr"]["redacted"] is True
    artifacts: dict[str, Any] = json.loads((directory / "delta.artifacts.json").read_text())
    assert (
        artifacts["files"]["target/compiled/tests/orders/check.sql"]["sha256"]
        == hashlib.sha256(b"SELECT 1 AS order_id\n").hexdigest()
    )
    assert destination.stat().st_mode & 0o777 == 0o700
    assert directory.stat().st_mode & 0o777 == 0o700
    assert all(path.stat().st_mode & 0o777 == 0o600 for path in directory.iterdir())


@pytest.mark.parametrize(
    "test_case",
    [
        ComparisonEvidenceTestCase(
            description="no capture on success", stderr="", expected_mismatches=0
        )
    ],
    ids=lambda case: case.description,
)
def test_given_success_when_capturing_then_changes_neither_result_nor_files(
    tmp_path: Path, test_case: ComparisonEvidenceTestCase
) -> None:
    destination: Path = tmp_path / "private"
    captured: DeltaCompileSeries = run_comparison(
        root=tmp_path / "captured", destination=destination, test_case=test_case
    )
    normal: DeltaCompileSeries = run_comparison(
        root=tmp_path / "normal", destination=None, test_case=test_case
    )

    assert captured == normal
    assert len(captured.unexpected) == test_case.expected_mismatches
    assert captured.unexpected == ()
    assert not destination.exists()


@pytest.mark.parametrize(
    "test_case",
    [
        ComparisonEvidenceTestCase(
            description="bounded failures",
            stdout="x" * (512 * 1024 + 19),
            steps=6,
            expected_mismatches=6,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_large_repeated_mismatches_when_capturing_then_bounds_size(
    tmp_path: Path, test_case: ComparisonEvidenceTestCase
) -> None:
    destination: Path = tmp_path / "private"
    result: DeltaCompileSeries = run_comparison(
        root=tmp_path / "run", destination=destination, test_case=test_case
    )

    assert len(result.unexpected) == test_case.expected_mismatches
    assert len(list(destination.iterdir())) == 4
    assert (
        sum(path.stat().st_size for path in filter(Path.is_file, destination.rglob("*")))
        <= 16 * 1024 * 1024
    )
    manifest: dict[str, Any] = json.loads((destination / "failure-00/manifest.json").read_text())
    assert manifest["files"]["delta.stdout"]["truncated"] is True
    assert (
        manifest["files"]["delta.stdout"]["original_sha256"]
        == hashlib.sha256(test_case.stdout.encode()).hexdigest()
    )
    assert (destination / "failure-00/delta.stdout").stat().st_size == 512 * 1024


@pytest.mark.parametrize(
    "test_case",
    [ComparisonEvidenceTestCase(description="capture cannot mask mismatch")],
    ids=lambda case: case.description,
)
def test_given_capture_io_failure_when_comparing_then_preserves_original_failure(
    tmp_path: Path,
    test_case: ComparisonEvidenceTestCase,
) -> None:
    destination: Path = tmp_path / "not-a-directory"
    destination.write_text("occupied")
    result: DeltaCompileSeries = run_comparison(
        root=tmp_path / "run", destination=destination, test_case=test_case
    )
    normal: DeltaCompileSeries = run_comparison(
        root=tmp_path / "normal", destination=None, test_case=test_case
    )

    assert result == normal
    assert len(result.unexpected) == test_case.expected_mismatches
    assert destination.read_text() == "occupied"


@pytest.mark.parametrize(
    "test_case",
    [
        ComparisonEvidenceTestCase(
            description="malformed cache path", cache_relative_path="bad\x00.sql"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_cache_path_when_capturing_then_preserves_comparison(
    tmp_path: Path, test_case: ComparisonEvidenceTestCase
) -> None:
    destination: Path = tmp_path / "private"
    result: DeltaCompileSeries = run_comparison(
        root=tmp_path / "captured", destination=destination, test_case=test_case
    )
    normal: DeltaCompileSeries = run_comparison(
        root=tmp_path / "normal", destination=None, test_case=test_case
    )

    assert result == normal
    assert len(result.unexpected) == test_case.expected_mismatches
    before: dict[str, Any] = json.loads((destination / "failure-00/cache-before.json").read_text())
    assert "capture_error" in before["delta"]
