import importlib
import json
import os
import re
import shutil
import sys
from collections.abc import Callable
from pathlib import Path
from typing import Any, NamedTuple, cast

import pytest

import sqlbuild._native as native_module
from sqlbuild._native import COMPILE_REUSE_DISABLE_ENV_VAR as REUSE_DISABLE_ENV_VAR
from sqlbuild._native import (
    STAGE_CAPTURE_DIR_ENV_VAR,
    PreparedCompileArtifacts,
    compiler_cache_directory,
)
from sqlbuild._native import run_cli as main
from sqlbuild.cli.compile_reuse._helpers import provider_settings as provider_settings_module

STAGING_PREFIX: str = ".sqlbuild-staging-"
NATIVE_REUSE_DIRECTORY_NAME: str = "project-reuse-v3"
NATIVE_REUSE_SUFFIX: str = ".store"


class _StagingEveryProject(PreparedCompileArtifacts):
    """Stage artifacts for a project of any size."""

    min_prepared_models: int = 1


OUTPUT_MODEL_HEADER: str = (
    "MODEL (description 'Orders {name}.', materialized table, contract enforced, "
    "columns (order_id (type INTEGER)));\n"
)
OUTPUT_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQLCLEAN021"]\n'
    ),
    **{
        f"models/orders_{name}.sql": OUTPUT_MODEL_HEADER.format(name=name)
        + "SELECT CAST(1 AS INTEGER) AS order_id\n"
        for name in ("east", "north", "west")
    },
}
EDITED_MODEL: str = "models/orders_north.sql"
DELETED_MODEL: str = "models/orders_west.sql"
TOUCH_BACK_NS: int = 3_600_000_000_000
BLOCKED_ARTIFACT: str = "target/compiled/models/orders_north.sql"
HELPER_MODULE_DIRECTORY: str = "site_packages"
HELPER_MODULE: str = "orders_reuse_helper"
TRACKED_VARIABLE: str = "SQLBUILD_ORDERS_REGION"

SURROGATE_SELECTOR: str = "orders\udcff"
SURROGATE_HOOK_FILES: dict[str, str] = {
    "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
    "models/orders.sql": "MODEL (description 'Orders.');\nSELECT 1 AS order_id\n",
    "hooks/python/notify.py": (
        "from sqlbuild.hooks import hook\n\n\n@hook\ndef notify_complete(ctx):\n"
        '    """Notify "q" \\\\ café \\udcff\\U0001F600\\ud83d\\ude00\\ud83d end."""\n'
        "    return None\n"
    ),
}


def nested_payload_hook_files(*, depth: int) -> dict[str, str]:
    """A model whose Python post-hook payload is `depth` nested lists around 1."""

    return {
        "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
        "models/orders.sql": (
            "MODEL (description 'Orders.', materialized table, post_hooks [python(\"notify\", "
            f"payload: {'[' * depth}1{']' * depth})]);\nSELECT 1 AS order_id\n"
        ),
        "hooks/python/notify.py": (
            "from sqlbuild.hooks import hook\n\n\n@hook\ndef notify(ctx, payload):\n"
            '    """Notify."""\n    return None\n'
        ),
    }


def nested_lists(*, depth: int) -> object:
    """`depth` nested single-item lists around 1."""

    value: object = 1
    for _ in range(depth):
        value = [value]
    return value


type CompileOutcome = tuple[int, dict[str, Any], dict[str, bytes]]


def write_files(*, project_dir: Path, files: dict[str, str]) -> None:
    """Write project files at their project-relative paths."""

    for relative, contents in files.items():
        path: Path = project_dir / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")


def prepare_output_compiles(*, monkeypatch: pytest.MonkeyPatch, reuse_disabled: str) -> None:
    """Stage small projects natively and settle file stamps."""

    monkeypatch.setenv(REUSE_DISABLE_ENV_VAR, reuse_disabled)
    monkeypatch.setattr(native_module, "PreparedCompileArtifacts", _StagingEveryProject)
    monkeypatch.setattr(native_module, "COMPILE_REUSE_RACY_WINDOW_NS", 0)


def main_compile_json(*, project_dir: Path) -> int:
    """Compile through the CLI with `--json --no-cache`; the report stays in captured stdout."""

    return main(["--project-dir", str(project_dir), "compile", "--json", "--no-cache"])


def compile_json_text(
    *, project_dir: Path, args: tuple[str, ...], capsys: pytest.CaptureFixture[str]
) -> str:
    """Compile through the CLI and return the JSON report text."""

    _ = main(["--project-dir", str(project_dir), "compile", "--json", *args])
    return capsys.readouterr().out


def compile_outputs(
    *, project_dir: Path, args: tuple[str, ...], capsys: pytest.CaptureFixture[str]
) -> CompileOutcome:
    """Compile through the CLI; return the exit code, timing-free JSON and compiled files."""

    exit_code: int = main(["--project-dir", str(project_dir), "compile", "--json", *args])
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)
    _ = payload.pop("compile_timings", None)
    return exit_code, payload, compile_files(project_dir=project_dir)


def reuse_compile(
    *, project_dir: Path, capsys: pytest.CaptureFixture[str]
) -> tuple[bool, CompileOutcome]:
    """Compile through the CLI with reuse on; return whether it replayed, and its outcome."""

    exit_code: int = main(["--project-dir", str(project_dir), "compile", "--json"])
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)
    timings: dict[str, int] = cast(dict[str, int], payload.pop("compile_timings"))
    return timings["project_reuse_hits"] == 1, (
        exit_code,
        payload,
        compile_files(project_dir=project_dir),
    )


class ReuseComparison(NamedTuple):
    """One reuse-enabled compile, whether it replayed, and an uncached compile of the project."""

    reused: bool
    outcome: tuple[int, dict[str, Any], dict[str, bytes]]
    reference: tuple[int, dict[str, Any], dict[str, bytes]]


def reuse_against_uncached(
    *, project_dir: Path, capsys: pytest.CaptureFixture[str]
) -> ReuseComparison:
    """Compile with reuse on, then compile a fresh copy with --no-cache, both comparable."""

    reused, outcome = reuse_compile(project_dir=project_dir, capsys=capsys)
    reference: CompileOutcome = uncached_reference(
        project_dir=project_dir, reference_dir=project_dir.parent / "uncached", capsys=capsys
    )
    return ReuseComparison(
        reused=reused, outcome=comparable(outcome), reference=comparable(reference)
    )


def stored_compile_files(*, project_dir: Path) -> list[Path]:
    """Every stored compile slot."""

    return sorted(
        (compiler_cache_directory(project_dir) / NATIVE_REUSE_DIRECTORY_NAME).glob(
            f"*{NATIVE_REUSE_SUFFIX}"
        )
    )


def corrupt_stored_compiles(project_dir: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Overwrite every stored compile file with bytes no reader accepts."""

    del monkeypatch
    for path in stored_compile_files(project_dir=project_dir):
        _ = path.write_bytes(b"garbage")


def truncate_stored_compiles(project_dir: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Cut every stored compile file in half."""

    del monkeypatch
    for path in stored_compile_files(project_dir=project_dir):
        contents: bytes = path.read_bytes()
        _ = path.write_bytes(contents[: len(contents) // 2])


def change_runtime(project_dir: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Report a different interpreter from now on."""

    del project_dir
    monkeypatch.setattr(sys, "version", "3.0.0 (main, Jan  1 2026, 00:00:00) [simulated]")


def change_native_build(project_dir: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Report another build of the native extension from now on, as after reinstalling it."""

    del project_dir
    monkeypatch.setattr(native_module, "BUILD_IDENTITY", f"{native_module.BUILD_IDENTITY}-rebuilt")


def edit_loaded_module(project_dir: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Rewrite the imported helper module outside the project."""

    del monkeypatch
    module: Path = project_dir.parent / HELPER_MODULE_DIRECTORY / f"{HELPER_MODULE}.py"
    _ = module.write_text(module.read_text(encoding="utf-8") + "# edited\n", encoding="utf-8")


def edit_then_fail_store(project_dir: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Edit a model, then make every later store fail while enumerating provider settings."""

    def raise_error(**_kwargs: object) -> None:
        raise RuntimeError("settings went away")

    _edit(project_dir=project_dir)
    monkeypatch.setattr(provider_settings_module, "provider_settings_inputs", raise_error)


def set_tracked_variable(project_dir: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Set a SQLBuild environment variable no template reads."""

    del project_dir
    monkeypatch.setenv(TRACKED_VARIABLE, "north")


def set_untracked_variable(project_dir: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Point stage captures somewhere; capture settings are not compile inputs."""

    monkeypatch.setenv(STAGE_CAPTURE_DIR_ENV_VAR, str(project_dir.parent / "captures"))


def block_stored_compiles(project_dir: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Replace every stored compile file with a non-empty directory."""

    del monkeypatch
    for path in stored_compile_files(project_dir=project_dir):
        path.unlink()
        path.mkdir()
        _ = (path / "keep.txt").write_text("kept\n", encoding="utf-8")


def load_helper_module(*, tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Import a helper module from outside the project, as an installed package would be."""

    directory: Path = tmp_path / HELPER_MODULE_DIRECTORY
    directory.mkdir()
    _ = (directory / f"{HELPER_MODULE}.py").write_text("ORDERS = 1\n", encoding="utf-8")
    monkeypatch.syspath_prepend(str(directory))
    monkeypatch.delitem(sys.modules, HELPER_MODULE, raising=False)
    _ = importlib.import_module(HELPER_MODULE)


def compile_files(*, project_dir: Path) -> dict[str, bytes]:
    """Every file under target/compiled by its relative path."""

    compiled: Path = project_dir / "target" / "compiled"
    return {
        path.relative_to(compiled).as_posix(): path.read_bytes()
        for path in sorted(filter(Path.is_file, compiled.rglob("*")))
    }


def main_compile(*, project_dir: Path) -> int:
    """Run one uncached JSON compile through the CLI entry point."""

    return main(["--project-dir", str(project_dir), "compile", "--json", "--no-cache"])


def uncached_reference(
    *, project_dir: Path, reference_dir: Path, capsys: pytest.CaptureFixture[str]
) -> CompileOutcome:
    """Compile a fresh copy of the project with --no-cache."""

    shutil.rmtree(reference_dir, ignore_errors=True)
    _ = shutil.copytree(project_dir, reference_dir, ignore=shutil.ignore_patterns("target"))
    return compile_outputs(project_dir=reference_dir, args=("--no-cache",), capsys=capsys)


def comparable(outcome: CompileOutcome) -> tuple[int, dict[str, Any], dict[str, bytes]]:
    """The outcome with project paths made relative, so two copies compare equal."""

    exit_code, payload, files = outcome
    artifacts: dict[str, Any] = cast(dict[str, Any], payload.get("artifacts", {}))
    return exit_code, {**payload, "artifacts": sorted(artifacts)}, files


def staging_directories(target_dir: Path) -> list[str]:
    """Staging directories left beside the compiled directory."""

    return sorted(path.name for path in target_dir.glob(f"{STAGING_PREFIX}*"))


def block_artifact(*, project_dir: Path) -> Path:
    """Replace one published artifact with a non-empty directory and edit its model."""

    blocked: Path = project_dir / BLOCKED_ARTIFACT
    blocked.unlink()
    blocked.mkdir()
    _ = (blocked / "keep.txt").write_text("kept\n", encoding="utf-8")
    model: Path = project_dir / EDITED_MODEL
    _ = model.write_text(
        model.read_text(encoding="utf-8").replace("CAST(1 AS", "CAST(2 AS"), encoding="utf-8"
    )
    return blocked


def neutral_error(*, error: BaseException, project_dir: Path) -> str:
    """The error text with the project directory and staging name replaced."""

    text: str = str(error).replace(str(project_dir), "<project>")
    return re.sub(rf"{re.escape(STAGING_PREFIX)}[0-9a-f]+", f"{STAGING_PREFIX}<id>", text)


def edit_sequence_step(*, project_dir: Path, step: str) -> None:
    """Apply one step of the cached edit sequence."""

    _SEQUENCE_ACTIONS[step](project_dir=project_dir)


def _unchanged(*, project_dir: Path) -> None:
    del project_dir


def _touch_back(*, project_dir: Path) -> None:
    model: Path = project_dir / EDITED_MODEL
    touched_ns: int = model.stat().st_mtime_ns - TOUCH_BACK_NS
    os.utime(model, ns=(touched_ns, touched_ns))


def _edit(*, project_dir: Path) -> None:
    model: Path = project_dir / EDITED_MODEL
    _ = model.write_text(
        model.read_text(encoding="utf-8").replace("CAST(1 AS", "CAST(30 AS"), encoding="utf-8"
    )


def _delete(*, project_dir: Path) -> None:
    (project_dir / DELETED_MODEL).unlink()


_SEQUENCE_ACTIONS: dict[str, Callable[..., None]] = {
    "cold": _unchanged,
    "warm": _unchanged,
    "touch": _touch_back,
    "retouch": _touch_back,
    "edit": _edit,
    "delete": _delete,
}


def artifacts_except(*, files: dict[str, bytes], name: str) -> dict[str, bytes]:
    """Every artifact except one."""

    return {key: files[key] for key in sorted(files.keys() - {name})}
