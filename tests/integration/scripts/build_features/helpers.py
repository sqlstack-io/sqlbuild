"""Write small Cargo workspaces shaped like the sqb and extension crates."""

import subprocess
from pathlib import Path

from tests.integration.scripts.build_features._test_types import BuildFeaturesTestCase

FEATURES: str = "[features]\nextra = []\nmore = []\n"


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _path_dependency(name: str, features: tuple[str, ...]) -> str:
    listed: str = ", ".join(f'"{feature}"' for feature in features)
    return f'{name} = {{ path = "../{name}", features = [{listed}] }}\n'


def write_workspace(root: Path, test_case: BuildFeaturesTestCase) -> None:
    """Write sqb and extension crates sharing a library and a proc-macro, then lock them."""
    _write(
        root / "Cargo.toml",
        '[workspace]\nresolver = "2"\nmembers = ["crates/*"]\n',
    )
    for name in ("shared", "helper"):
        _write(
            root / "crates" / name / "Cargo.toml",
            f'[package]\nname = "{name}"\nversion = "0.1.0"\nedition = "2021"\n\n{FEATURES}',
        )
        _write(root / "crates" / name / "src" / "lib.rs", "")
    _write(
        root / "crates" / "derive" / "Cargo.toml",
        '[package]\nname = "derive"\nversion = "0.1.0"\nedition = "2021"\n\n'
        '[lib]\nproc-macro = true\n\n[dependencies]\nhelper = { path = "../helper" }\n',
    )
    _write(root / "crates" / "derive" / "src" / "lib.rs", "")
    _write(
        root / "crates" / "sqlbuild-cli" / "Cargo.toml",
        '[package]\nname = "sqlbuild-cli"\nversion = "0.1.0"\nedition = "2021"\n\n'
        "[features]\nui = []\n\n"
        '[dependencies]\nshared = { path = "../shared" }\nderive = { path = "../derive" }\n',
    )
    _write(root / "crates" / "sqlbuild-cli" / "src" / "lib.rs", "")
    _write(
        root / "crates" / "sqlbuild-python" / "Cargo.toml",
        '[package]\nname = "sqlbuild-python"\nversion = "0.1.0"\nedition = "2021"\n\n'
        "[features]\nextension-module = []\n\n"
        '[dependencies]\nsqlbuild-cli = { path = "../sqlbuild-cli" }\n'
        f"{_path_dependency('shared', test_case.extension_shared_features)}\n"
        "[build-dependencies]\n"
        f"{_path_dependency('helper', test_case.extension_host_features)}",
    )
    _write(root / "crates" / "sqlbuild-python" / "src" / "lib.rs", "")
    _write(root / "crates" / "sqlbuild-python" / "build.rs", "fn main() {}\n")
    subprocess.run(("cargo", "generate-lockfile", "--offline"), cwd=root, check=True)
