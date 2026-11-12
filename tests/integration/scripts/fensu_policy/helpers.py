"""Build a minimal repository and run the real Fensu CLI on its Rust target."""

import shutil
import subprocess
import sys
from pathlib import Path


def write_observation_project(*, root: Path, allowed: tuple[str, ...], planner: str) -> None:
    """A crate whose observations module may name observation state, plus the given planner."""

    repository: Path = Path(__file__).resolve().parents[4]
    shutil.copytree(
        repository / "scripts/fensu_policy",
        root / "scripts/fensu_policy",
        ignore=shutil.ignore_patterns("__pycache__"),
    )
    (root / "scripts/__init__.py").write_text("")
    (root / "Cargo.toml").write_text(
        '[workspace]\nmembers = ["crates/sqlbuild-python"]\nresolver = "2"\n'
    )
    crate: Path = root / "crates/sqlbuild-python"
    (crate / "src/observations").mkdir(parents=True)
    (crate / "Cargo.toml").write_text(
        '[package]\nname = "sqlbuild-python"\nversion = "0.1.0"\nedition = "2021"\n'
    )
    (crate / "src/lib.rs").write_text("mod observations;\nmod planner;\n")
    (crate / "src/observations/mod.rs").write_text(
        "pub fn table() -> &'static str { OBSERVATIONS_TABLE_NAME }\n"
    )
    (crate / "src/planner.rs").write_text(planner)
    entries: str = ", ".join(f'"{path}=Writes observations."' for path in allowed)
    (root / "fensu.toml").write_text(
        '[targets.rust]\nanalyzer = "rust"\nroots = ["crates"]\n'
        'rule_modules = ["scripts.fensu_policy.rules.observation_boundary"]\n'
        'select = ["XSB078"]\n\n'
        f"[targets.rust.rule_options.XSB078]\nallowed_paths = [{entries}]\n"
    )


def check_rust(*, root: Path) -> subprocess.CompletedProcess[str]:
    """Run `fensu check` on the Rust target without a cache."""

    return subprocess.run(
        [
            str(Path(sys.executable).with_name("fensu")),
            "check",
            "--target",
            "rust",
            "--no-cache",
            "--no-color",
        ],
        cwd=root,
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
