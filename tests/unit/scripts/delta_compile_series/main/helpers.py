"""Synthetic compile callbacks exercising the comparison harness's public interface."""

from __future__ import annotations

import hashlib
import json
import subprocess
from pathlib import Path

from scripts.delta_compile_series.main.run_delta_compile_series import run_delta_compile_series
from scripts.delta_compile_series.main.write_delta_compile_project import (
    write_delta_compile_project,
)
from scripts.delta_compile_series.models import DeltaCompileOutcome, DeltaCompileSeries
from tests.unit.scripts.delta_compile_series.main._test_types import ComparisonEvidenceTestCase


def run_comparison(
    *,
    root: Path,
    destination: Path | None,
    test_case: ComparisonEvidenceTestCase,
) -> DeltaCompileSeries:
    """Compare real copied cache/artifacts, with controlled output and cache changes."""

    project: Path = root / "orders"
    write_delta_compile_project(project_dir=project, model_count=8)
    artifact: Path = project / "target/compiled/tests/orders/check.sql"
    artifact.parent.mkdir(parents=True)
    artifact.write_text("SELECT 1 AS order_id\n")
    cache: Path = project / "target/cache/compiler-native-v1/sql-test-artifacts.json"
    cache.parent.mkdir(parents=True)
    cache.write_text(
        json.dumps(
            {
                "version": 5,
                "records": {
                    "orders": {
                        "relative_path": test_case.cache_relative_path,
                        "identity": "original-identity",
                        "size": artifact.stat().st_size,
                        "mtime_ns": artifact.stat().st_mtime_ns,
                        "content_sha256": hashlib.sha256(artifact.read_bytes()).hexdigest(),
                    }
                },
            }
        )
    )

    def compile_project() -> DeltaCompileOutcome:
        return DeltaCompileOutcome(0, test_case.stdout, "", ("sqb", "compile", "--json"))

    def compile_full(copy: Path) -> DeltaCompileOutcome:
        _ = (copy / cache.relative_to(project)).write_text("{")
        return DeltaCompileOutcome(
            0,
            test_case.stdout,
            test_case.stderr,
            ("sqb", "compile", "--json"),
            (("SQLBUILD_DELTA_COMPILE_DISABLE", "1"), ("PASSWORD", "synthetic-environment-secret")),
        )

    return run_delta_compile_series(
        project_dir=project,
        seed=16,
        steps=test_case.steps,
        compile_project=compile_project,
        compile_in_full=compile_full,
        diagnostics_dir=destination,
        case="orders comparison",
    )


def diagnostic_recipient(root: Path) -> tuple[Path, Path]:
    """Generate disposable test keys without printing or persisting them outside tmp_path."""

    root.mkdir(mode=0o700)
    key: Path = root / "key.pem"
    certificate: Path = root / "recipient.pem"
    _ = subprocess.run(
        [
            "openssl",
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-noenc",
            "-keyout",
            str(key),
            "-out",
            str(certificate),
            "-days",
            "1",
            "-subj",
            "/CN=orders-diagnostics-test",
        ],
        check=True,
        capture_output=True,
    )
    key.chmod(0o600)
    return certificate, key


def decrypt_evidence(path: Path, key: Path) -> subprocess.CompletedProcess[bytes]:
    """Keep all unauthenticated output in memory; callers must check success before use."""

    return subprocess.run(
        [
            "openssl",
            "cms",
            "-decrypt",
            "-binary",
            "-inform",
            "DER",
            "-in",
            str(path),
            "-inkey",
            str(key),
        ],
        check=False,
        capture_output=True,
    )
