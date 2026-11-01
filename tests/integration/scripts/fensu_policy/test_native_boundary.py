"""Exercise native boundary policy through Fensu's real Rust analyzer and rule host."""

import shutil
import subprocess
import sys
from pathlib import Path

import pytest

from tests.integration.scripts.fensu_policy._test_types import NativeBoundaryTestCase


@pytest.mark.parametrize(
    "test_case",
    [
        NativeBoundaryTestCase(
            description="pyclass module metadata",
            metadata='#[pyclass(frozen, module = "sqlbuild._native")]\npub struct Snapshot;\n',
            expected_fault_count=2,
        ),
        NativeBoundaryTestCase(
            description="namespaced pyclass raw module metadata",
            metadata='#[pyo3::pyclass(frozen, module = r#"sqlbuild._native"#)]\npub struct Snapshot;\n',
            expected_fault_count=2,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_pyclass_metadata_when_checking_rust_then_only_real_python_crossings_fault(
    tmp_path: Path,
    test_case: NativeBoundaryTestCase,
) -> None:
    repository: Path = Path(__file__).resolve().parents[4]
    shutil.copytree(
        repository / "scripts/fensu_policy",
        tmp_path / "scripts/fensu_policy",
        ignore=shutil.ignore_patterns("__pycache__"),
    )
    (tmp_path / "scripts/__init__.py").write_text("")
    (tmp_path / "Cargo.toml").write_text(
        '[workspace]\nmembers = ["crates/sqlbuild-python"]\nresolver = "2"\n'
    )
    crate: Path = tmp_path / "crates/sqlbuild-python"
    (crate / "src").mkdir(parents=True)
    (crate / "Cargo.toml").write_text(
        '[package]\nname = "sqlbuild-python"\nversion = "0.1.0"\nedition = "2021"\n'
    )
    (tmp_path / "fensu.toml").write_text(
        '[targets.rust]\nanalyzer = "rust"\nroots = ["crates"]\n'
        'rule_modules = ["scripts.fensu_policy.rules.native_boundary"]\n'
        'select = ["XSB072"]\n'
    )
    source: Path = crate / "src/lib.rs"
    source.write_text(test_case.metadata)
    command: list[str] = [
        str(Path(sys.executable).with_name("fensu")),
        "check",
        "--target",
        "rust",
        "--no-cache",
        "--no-color",
    ]
    clean: subprocess.CompletedProcess[str] = subprocess.run(
        command, cwd=tmp_path, capture_output=True, text=True, timeout=15, check=False
    )
    assert clean.returncode == 0, clean.stdout + clean.stderr
    assert "Found 0 faults" in clean.stdout
    source.write_text(
        test_case.metadata + 'fn read(py: Python<\'_>) { py.import("sqlbuild.compiler"); }\n'
    )
    crossing: subprocess.CompletedProcess[str] = subprocess.run(
        command, cwd=tmp_path, capture_output=True, text=True, timeout=15, check=False
    )
    assert crossing.returncode == 1, crossing.stdout + crossing.stderr
    assert crossing.stdout.count("XSB072") == test_case.expected_fault_count, crossing.stdout
    assert crossing.stdout.count(" --> crates/sqlbuild-python/src/lib.rs:3:0") == 2
    assert " --> crates/sqlbuild-python/src/lib.rs:1:0" not in crossing.stdout
    assert source.read_text().startswith(test_case.metadata)
