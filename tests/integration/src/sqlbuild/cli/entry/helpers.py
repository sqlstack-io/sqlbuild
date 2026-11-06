import json
import os
import re
import shutil
import signal
import subprocess
import sys
from datetime import UTC, datetime, timedelta
from decimal import Decimal
from pathlib import Path

import sqlbuild
from sqlbuild._native import COMPILE_REUSE_DISABLE_ENV_VAR as REUSE_DISABLE_ENV_VAR
from sqlbuild._native import CostRunRecord, ResourceCost, write_cost_run
from sqlbuild.cost.models import RunCostSummary
from sqlbuild.cost.types import CostStatus

type LauncherRun = tuple[int, bytes, bytes]

_ELAPSED_SECONDS: re.Pattern[bytes] = re.compile(rb"\(\d+\.\d s\)")
_PHASE_SECONDS: re.Pattern[bytes] = re.compile(rb"\(\d+\.\d+s\)")
_DIAGNOSTIC_IDENTITY: re.Pattern[bytes] = re.compile(rb'"(invocation_id|occurred_at)":"[^"]*"')
NATIVE_SQB: Path = Path(sys.executable).with_name("sqb")
PYTHON_CLI: Path = Path(sys.executable).with_name("sqlbuild-python")
PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders_shop"\n'
        'adapter = "duckdb"\n'
        'default_target = "dev"\n\n'
        "[connections.local]\n"
        'database = "warehouse.duckdb"\n\n'
        "[targets.dev]\n"
        'connection = "local"\n'
        'schema = "dev"\n'
    ),
    "models/orders.sql": (
        'MODEL (\n  description "One row per order",\n  materialized table,\n);\n\n'
        "SELECT 1 AS order_id, CAST('2026-04-01' AS DATE) AS ordered_on\n"
    ),
}


def write_project(project_dir: Path) -> Path:
    """Write a one-model DuckDB project."""

    for relative, contents in PROJECT_FILES.items():
        path: Path = project_dir / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")
    return project_dir


def run(program: Path, *, args: tuple[str, ...], cwd: Path) -> LauncherRun:
    """Run one CLI process with reuse enabled; return its exit code, stdout and timeless stderr."""

    completed: subprocess.CompletedProcess[bytes] = subprocess.run(
        [str(program), *args],
        cwd=cwd,
        capture_output=True,
        check=False,
        env={**os.environ, "NO_COLOR": "1", REUSE_DISABLE_ENV_VAR: "0"},
        timeout=600,
    )
    return (
        completed.returncode,
        completed.stdout,
        _PHASE_SECONDS.sub(b"(s)", completed.stderr),
    )


def launch_records(project_dir: Path) -> dict[Path, int]:
    """Every launch record in the project and its modification time."""

    directory: Path = project_dir / "target" / "cache" / "launcher-v1"
    return {path: path.stat().st_mtime_ns for path in directory.glob("*.json")}


def compute_log_captures(project_dir: Path) -> set[Path]:
    """Every completed compute log capture directory."""

    return {marker.parent for marker in (project_dir / "logs").glob("*/*/complete")}


def capture_metadata(capture: Path) -> dict[str, object]:
    """A capture's final metadata."""

    return json.loads((capture / "metadata.json").read_text(encoding="utf-8"))


def without_seconds(stream: bytes) -> bytes:
    """The stream with the reuse notice's elapsed seconds blanked."""

    return _ELAPSED_SECONDS.sub(b"(s)", stream)


def compile_timings(stdout: bytes) -> dict[str, int]:
    """The `compile_timings` of a JSON compile report."""

    return json.loads(stdout)["compile_timings"]


def without_compile_timings(stdout: bytes) -> dict[str, object]:
    """Compare full reports while allowing invocation timings to differ."""

    payload: dict[str, object] = json.loads(stdout)
    _ = payload.pop("compile_timings")
    return payload


def counting_install(directory: Path, marker: Path) -> Path:
    """Install `sqb` beside a `sqlbuild-python` that logs each start to `marker`; return `sqb`."""

    directory.mkdir(parents=True)
    program: Path = Path(shutil.copy2(NATIVE_SQB, directory / NATIVE_SQB.name))
    script: Path = directory / PYTHON_CLI.name
    _ = script.write_text(
        f'#!/bin/sh\necho started >> "{marker}"\nexec "{PYTHON_CLI}" "$@"\n', encoding="utf-8"
    )
    script.chmod(0o755)
    return program


def python_starts(marker: Path) -> int:
    """How many times the logging `sqlbuild-python` started."""

    return len(marker.read_text(encoding="utf-8").splitlines())


def write_linked_project(project_dir: Path) -> Path:
    """Put the model behind a file link to an external input."""

    project: Path = write_project(project_dir)
    model: Path = project / "models" / "orders.sql"
    target: Path = project.parent / "linked_orders.sql"
    _ = model.replace(target)
    model.symlink_to(target)
    return project


def rewrite_model_preserving_mtime(project_dir: Path) -> None:
    """Change equal-length SQL bytes, including through a link, without changing mtime."""

    model: Path = project_dir / "models" / "orders.sql"
    original: os.stat_result = model.stat()
    _ = model.write_text(model.read_text().replace("1 AS order_id", "2 AS order_id"))
    os.utime(model, ns=(original.st_atime_ns, original.st_mtime_ns))


def run_with_full_output(program: Path, *, args: tuple[str, ...], cwd: Path) -> LauncherRun:
    """Run with a writable descriptor that rejects every output byte."""

    with Path("/dev/full").open("wb") as output:
        completed: subprocess.CompletedProcess[bytes] = subprocess.run(
            [str(program), *args],
            cwd=cwd,
            stdout=output,
            stderr=subprocess.PIPE,
            env={**os.environ, "NO_COLOR": "1", REUSE_DISABLE_ENV_VAR: "0"},
            check=False,
            timeout=600,
        )
    return completed.returncode, b"", completed.stderr


def startup_install(directory: Path) -> tuple[Path, Path, Path]:
    """Install a launcher in an isolated interpreter with an editable startup search path."""

    subprocess.run(
        [sys.executable, "-m", "venv", "--without-pip", "--system-site-packages", str(directory)],
        check=True,
        capture_output=True,
    )
    scripts: Path = directory / "bin"
    program: Path = Path(shutil.copy2(NATIVE_SQB, scripts / "sqb"))
    handoff: Path = scripts / "sqlbuild-python"
    _ = handoff.write_text(
        f"#!{scripts / 'python'}\n"
        "from sqlbuild.cli.entry.main.launched import main\nraise SystemExit(main())\n",
        encoding="utf-8",
    )
    handoff.chmod(0o755)
    site_packages: Path = next((directory / "lib").glob("python*/site-packages"))
    _ = (site_packages / "test_dependencies.pth").write_text(
        str(Path(sqlbuild.__file__).resolve().parent.parent)
        + "\n"
        + "\n".join(str(Path(entry).resolve()) for entry in sys.path)
        + "\n",
        encoding="utf-8",
    )
    old: Path = directory.parent / "old_helpers"
    new: Path = directory.parent / "new_helpers"
    old.mkdir()
    new.mkdir()
    _ = (old / "catalog_helper.py").write_text("SQL = '1'\n", encoding="utf-8")
    _ = (new / "catalog_helper.py").write_text("SQL = '2'\n", encoding="utf-8")
    startup: Path = site_packages / "catalog.pth"
    _ = startup.write_text(str(old) + "\n", encoding="utf-8")
    return program, startup, new


def compiled_files(project_dir: Path) -> dict[str, bytes]:
    """Every compiled SQL artifact under the project's target, by relative path."""

    compiled: Path = project_dir / "target" / "compiled"
    return {
        path.relative_to(compiled).as_posix(): path.read_bytes()
        for path in sorted(compiled.rglob("*.sql"))
    }


def counting_environment(directory: Path, marker: Path) -> Path:
    """Install `sqb` in an environment whose `sqlbuild-python` logs each start; return `sqb`."""

    program: Path = counting_install(directory / "bin", marker)
    site_packages: Path = (
        directory / "lib" / f"python{sys.version_info.major}.{sys.version_info.minor}"
    ) / "site-packages"
    site_packages.mkdir(parents=True)
    _ = (site_packages / "sqlbuild_source.pth").write_text(
        str(Path(sqlbuild.__file__).resolve().parent.parent) + "\n", encoding="utf-8"
    )
    return program


def reset_directory(directory: Path) -> Path:
    """An empty `directory`, removing whatever an earlier run left."""

    shutil.rmtree(directory, ignore_errors=True)
    directory.mkdir(parents=True)
    return directory


def _capture_files(capture: Path) -> dict[str, object]:
    metadata: dict[str, object] = capture_metadata(capture)
    for volatile in ("invocation_id", "started_at", "completed_at", "capture_date"):
        _ = metadata.pop(volatile, None)
    streams: dict[str, bytes] = {
        name: _DIAGNOSTIC_IDENTITY.sub(rb'"\1":""', (capture / name).read_bytes())
        for name in ("stdout.log", "stderr.log", "diagnostics.jsonl")
    }
    return {"complete": (capture / "complete").exists(), "metadata": metadata, **streams}


def compute_log_files(project_dir: Path) -> list[dict[str, object]]:
    """Each capture's streams and metadata, without per-run identity and times."""

    return [
        _capture_files(metadata.parent)
        for metadata in sorted((project_dir / "logs").glob("*/*/metadata.json"))
    ]


def prepare_nothing(directory: Path) -> None:
    """Leave the directory empty."""

    _ = directory


def prepare_project(directory: Path) -> None:
    """Write the one-model project."""

    _ = write_project(directory)


def prepare_custom_skill(directory: Path) -> None:
    """Write a hand-written skill where `sqb init` installs the packaged one."""

    custom: Path = directory / ".claude" / "skills" / "sqlbuild" / "SKILL.md"
    custom.parent.mkdir(parents=True)
    _ = custom.write_text("my own skill\n", encoding="utf-8")


def prepare_stale_skills(directory: Path) -> None:
    """Write the project configured for Claude skills that are not installed yet."""

    _ = write_project(directory)
    _ = (directory / "sqlbuild_project.toml").write_text(
        PROJECT_FILES["sqlbuild_project.toml"] + '\n[skills]\ntargets = ["claude"]\n',
        encoding="utf-8",
    )


def prepare_custom_rules(directory: Path) -> None:
    """Write the project with a custom rules directory."""

    _ = write_project(directory)
    (directory / "rules").mkdir()


def prepare_removed_allow_exceptions(directory: Path) -> None:
    """Write the project still setting the removed `[rules] allow_exceptions`."""

    _ = write_project(directory)
    _ = (directory / "sqlbuild_project.toml").write_text(
        PROJECT_FILES["sqlbuild_project.toml"] + "\n[rules]\nallow_exceptions = true\n",
        encoding="utf-8",
    )


def prepare_published_layer_options(directory: Path) -> None:
    """Write the project with configured published-layer Rule options."""

    _ = write_project(directory)
    _ = (directory / "sqlbuild_project.toml").write_text(
        PROJECT_FILES["sqlbuild_project.toml"]
        + "\n[rules.rule_options.SQBRPROJECT107]\n"
        + 'published_layers = ["mart"]\n'
        + 'allowed_qualifiers = [{ qualifier = "csv_export", reason = "Export views." }]\n',
        encoding="utf-8",
    )


def prepare_named_pipe_project(directory: Path) -> None:
    """Make the project file a named pipe that blocks its reader."""

    os.mkfifo(directory / "sqlbuild_project.toml")


def restore_default_interrupt() -> None:
    """Give a spawned command default Ctrl-C handling even when its parent ignores SIGINT."""

    _ = signal.signal(signal.SIGINT, signal.SIG_DFL)


def run_without_reader(program: Path, *, args: tuple[str, ...], cwd: Path) -> LauncherRun:
    """Run with a stdout pipe whose reader has already closed; return exit code and stderr."""

    reader, writer = os.pipe()
    os.close(reader)
    environment: dict[str, str] = {**os.environ, "NO_COLOR": "1"}
    # The broken-pipe contract follows Python's default buffered stdout.
    _ = environment.pop("PYTHONUNBUFFERED", None)
    try:
        completed: subprocess.CompletedProcess[bytes] = subprocess.run(
            [str(program), *args],
            cwd=cwd,
            stdout=writer,
            stderr=subprocess.PIPE,
            env=environment,
            check=False,
            timeout=600,
        )
    finally:
        os.close(writer)
    return completed.returncode, b"", completed.stderr


def write_janitor_project(project_dir: Path, *, extra_toml: str) -> Path:
    """Write a DuckDB project with one model, janitor left disabled, and extra settings."""

    project_dir.mkdir(parents=True, exist_ok=True)
    (project_dir / "models").mkdir(exist_ok=True)
    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "orders_shop"\nadapter = "duckdb"\n\n[connection]\n'
        f'database = "orders.duckdb"\n{extra_toml}',
        encoding="utf-8",
    )
    (project_dir / "models" / "orders.sql").write_text(
        "MODEL (description 'Orders.');\n\nSELECT 1 AS order_id\n", encoding="utf-8"
    )
    return project_dir


def write_cost_project(project_dir: Path) -> Path:
    """Persist exact, asymmetric metrics through the public API, with ties and missing metrics."""

    project: Path = write_project(project_dir)
    origin: datetime = datetime(2026, 8, 23, tzinfo=UTC)
    for index, (name, usd, status) in enumerate(
        (
            ("negative", "-3", CostStatus.COMPLETE),
            ("small", "2", CostStatus.COMPLETE),
            ("large", "10.0000000000000000001", CostStatus.COMPLETE),
            ("tie", "2.00", CostStatus.COMPLETE),
            ("pending", "100", CostStatus.PENDING),
        )
    ):
        completed: datetime = origin + timedelta(days=index)
        resource: ResourceCost = ResourceCost(
            resource_type="model",
            resource_name="orders",
            warehouse_name="warehouse",
            warehouse_size="small",
            warehouse_type="STANDARD",
            cluster_number=1,
            query_count=1,
            attributed_seconds=Decimal(index + 1),
            bytes_scanned=index + 1,
            estimated_compute_credits=Decimal(usd),
            estimated_usd=Decimal(usd),
        )
        record: CostRunRecord = CostRunRecord(
            run_id=f"run-{name}",
            adapter_name="duckdb",
            target_name="dev",
            build_status="success",
            started_at=completed - timedelta(seconds=index + 1),
            completed_at=completed,
            cost=RunCostSummary(
                status=status,
                usd_per_credit=Decimal(1),
                rate_source="default",
                resources=(resource,),
                estimated_compute_credits=Decimal(usd),
                estimated_usd=Decimal(usd),
            ),
        )
        _ = write_cost_run(project_dir=project, record=record)
    return project
