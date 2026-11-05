"""Authenticated, bounded CI retention; only complete ciphertext is uploadable."""

from __future__ import annotations

import io
import os
import subprocess
import zipfile
from pathlib import Path

from scripts.delta_compile_series.constants import (
    MAX_ENCRYPTED_BYTES,
    MAX_ENCRYPTED_FAILURES,
    MAX_EVIDENCE_FILES,
    MAX_STEP_BYTES,
)


def encrypt_comparison_failure(directory: Path) -> None:
    """Retain at most twelve encrypted failures when CI explicitly enables retention."""

    configured: str | None = os.environ.get("SQLBUILD_COMPARISON_ENCRYPTED_DIR")
    if not configured:
        return
    destination: Path = Path(configured)
    destination.mkdir(parents=True, exist_ok=True, mode=0o700)
    destination.chmod(0o700)
    for index in range(MAX_ENCRYPTED_FAILURES):
        slot: Path = destination / f"failure-{index:02d}"
        try:
            slot.mkdir(mode=0o700)
        except FileExistsError:
            continue
        break
    else:
        return
    pending: Path = slot / "encrypted.partial"
    try:
        files: list[Path] = list(directory.iterdir())
        if len(files) > MAX_EVIDENCE_FILES or any(
            path.is_symlink() or not path.is_file() for path in files
        ):
            raise OSError("Invalid comparison evidence files")
        archive: io.BytesIO = io.BytesIO()
        remaining: int = MAX_STEP_BYTES
        with zipfile.ZipFile(archive, mode="w", compression=zipfile.ZIP_DEFLATED) as bundle:
            for path in sorted(files):
                with path.open("rb") as stream:
                    data: bytes = stream.read(remaining + 1)
                remaining -= len(data)
                if remaining < 0:
                    raise OSError("Comparison evidence exceeds retention bound")
                bundle.writestr(path.name, data)
        certificate: str = os.environ.get("SQLBUILD_COMPARISON_RECIPIENT", "")
        with pending.open("xb") as output:
            pending.chmod(0o600)
            result: subprocess.CompletedProcess[bytes] = subprocess.run(
                [
                    "openssl",
                    "cms",
                    "-encrypt",
                    "-binary",
                    "-aes-256-gcm",
                    "-outform",
                    "DER",
                    "-recip",
                    certificate,
                    "-keyopt",
                    "rsa_padding_mode:oaep",
                    "-keyopt",
                    "rsa_oaep_md:sha256",
                ],
                input=archive.getvalue(),
                stdout=output,
                stderr=subprocess.PIPE,
                check=False,
                timeout=30,
            )
        if result.returncode != 0 or not 0 < pending.stat().st_size <= MAX_ENCRYPTED_BYTES:
            raise OSError("Comparison evidence encryption failed")
        pending.replace(slot / "failure.cms")
    except subprocess.TimeoutExpired as error:
        raise OSError("Comparison evidence encryption timed out") from error
    finally:
        pending.unlink(missing_ok=True)
