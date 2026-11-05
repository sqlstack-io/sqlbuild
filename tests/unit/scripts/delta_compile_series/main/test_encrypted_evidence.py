"""Real OpenSSL retention through the unchanged comparison interface."""

from __future__ import annotations

import io
import subprocess
import zipfile
from concurrent.futures import Future, ThreadPoolExecutor
from pathlib import Path

import pytest

from scripts.delta_compile_series.models import DeltaCompileSeries
from tests.unit.scripts.delta_compile_series.main._test_types import ComparisonEvidenceTestCase
from tests.unit.scripts.delta_compile_series.main.helpers import (
    decrypt_evidence,
    diagnostic_recipient,
    run_comparison,
)


@pytest.mark.parametrize(
    "test_case",
    [ComparisonEvidenceTestCase(description="authenticated failure retention")],
    ids=lambda case: case.description,
)
def test_given_mismatch_when_encrypting_then_decrypts_exact_evidence_and_rejects_damage(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, test_case: ComparisonEvidenceTestCase
) -> None:
    certificate, key = diagnostic_recipient(tmp_path / "recipient")
    _, wrong_key = diagnostic_recipient(tmp_path / "other-recipient")
    encrypted: Path = tmp_path / "encrypted"
    private: Path = tmp_path / "private"
    monkeypatch.setenv("SQLBUILD_COMPARISON_ENCRYPTED_DIR", str(encrypted))
    monkeypatch.setenv("SQLBUILD_COMPARISON_RECIPIENT", str(certificate))
    result: DeltaCompileSeries = run_comparison(
        root=tmp_path / "run", destination=private, test_case=test_case
    )
    assert len(result.unexpected) == test_case.expected_mismatches
    ciphertext: Path = encrypted / "failure-00/failure.cms"
    decrypted: subprocess.CompletedProcess[bytes] = decrypt_evidence(ciphertext, key)
    assert decrypted.returncode == 0
    with zipfile.ZipFile(io.BytesIO(decrypted.stdout)) as archive:
        assert set(archive.namelist()) == {path.name for path in (private / "failure-00").iterdir()}
        assert all(
            archive.read(name) == (private / "failure-00" / name).read_bytes()
            for name in archive.namelist()
        )
    assert decrypt_evidence(ciphertext, wrong_key).returncode != 0
    original: bytes = ciphertext.read_bytes()
    damaged: Path = tmp_path / "damaged.cms"
    damaged.write_bytes(original[:-1] + bytes([original[-1] ^ 1]))
    assert decrypt_evidence(damaged, key).returncode != 0
    damaged.write_bytes(original[: len(original) // 2])
    assert decrypt_evidence(damaged, key).returncode != 0
    assert b"SQL test planning" not in original
    assert ciphertext.stat().st_mode & 0o777 == 0o600
    assert not list(encrypted.rglob("*.partial"))


@pytest.mark.parametrize(
    "test_case",
    [
        ComparisonEvidenceTestCase(
            description="successful comparison", stderr="", expected_mismatches=0
        )
    ],
    ids=lambda case: case.description,
)
def test_given_success_when_retention_enabled_then_creates_no_private_or_encrypted_files(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, test_case: ComparisonEvidenceTestCase
) -> None:
    encrypted: Path = tmp_path / "encrypted"
    private: Path = tmp_path / "private"
    monkeypatch.setenv("SQLBUILD_COMPARISON_ENCRYPTED_DIR", str(encrypted))
    result: DeltaCompileSeries = run_comparison(
        root=tmp_path / "run", destination=private, test_case=test_case
    )
    assert result.unexpected == ()
    assert len(result.unexpected) == test_case.expected_mismatches
    assert not encrypted.exists()
    assert not private.exists()


@pytest.mark.parametrize(
    "test_case",
    [ComparisonEvidenceTestCase(description="failed encryption")],
    ids=lambda case: case.description,
)
def test_given_invalid_recipient_when_retaining_then_preserves_failure_without_uploadable_output(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, test_case: ComparisonEvidenceTestCase
) -> None:
    encrypted: Path = tmp_path / "encrypted"
    monkeypatch.setenv("SQLBUILD_COMPARISON_ENCRYPTED_DIR", str(encrypted))
    monkeypatch.setenv("SQLBUILD_COMPARISON_RECIPIENT", str(tmp_path / "missing.pem"))
    result: DeltaCompileSeries = run_comparison(
        root=tmp_path / "captured", destination=tmp_path / "private", test_case=test_case
    )
    normal: DeltaCompileSeries = run_comparison(
        root=tmp_path / "normal", destination=None, test_case=test_case
    )
    assert result == normal
    assert len(result.unexpected) == test_case.expected_mismatches
    assert not list(encrypted.rglob("*.cms"))
    assert not list(encrypted.rglob("*.partial"))


@pytest.mark.parametrize(
    "test_case",
    [
        ComparisonEvidenceTestCase(
            description="bounded encrypted retention",
            steps=4,
            expected_mismatches=4,
            stdout="x" * (512 * 1024 + 19),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_repeated_failures_when_retaining_then_bounds_ciphertext_count_and_bytes(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, test_case: ComparisonEvidenceTestCase
) -> None:
    certificate, _ = diagnostic_recipient(tmp_path / "recipient")
    encrypted: Path = tmp_path / "encrypted"
    monkeypatch.setenv("SQLBUILD_COMPARISON_ENCRYPTED_DIR", str(encrypted))
    monkeypatch.setenv("SQLBUILD_COMPARISON_RECIPIENT", str(certificate))
    with ThreadPoolExecutor(max_workers=4) as executor:
        futures: list[Future[DeltaCompileSeries]] = [
            executor.submit(
                run_comparison,
                root=tmp_path / f"run-{index}",
                destination=tmp_path / f"private-{index}",
                test_case=test_case,
            )
            for index in range(4)
        ]
        assert all(
            len(future.result().unexpected) == test_case.expected_mismatches for future in futures
        )
    files: list[Path] = list(encrypted.rglob("*.cms"))
    assert len(files) == 12
    assert all(0 < path.stat().st_size <= 4 * 1024 * 1024 + 64 * 1024 for path in files)
    assert sum(path.stat().st_size for path in files) <= 12 * (4 * 1024 * 1024 + 64 * 1024)
    assert not list(encrypted.rglob("*.partial"))
