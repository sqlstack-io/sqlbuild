"""`sqb migrate orchestration`: rewrite Python jobs, job groups and schedules as TOML, once.

Jobs built in loops or from constants exist only once their module runs, so this imports each
orchestration module with the removed records restored, then writes what it found.
"""

from __future__ import annotations

import ast
import json
import re
import sys
from collections.abc import Iterator
from contextlib import contextmanager
from dataclasses import dataclass, field
from pathlib import Path

from sqlbuild.compiler.discovery._helpers.filesystem.project_modules import (
    load_orchestration_module,
)
from sqlbuild.orchestration_declarations.models import legacy_authoring as legacy

_ROOT = "orchestration"
_RENAMES = "renames.toml"
_REMOVED = {
    "job": legacy.Job,
    "job_group": legacy.JobGroup,
    "schedule": legacy.Schedule,
    "calendar_schedule": legacy.CalendarSchedule,
    "on_failure": legacy.OnFailure,
    "resources": legacy.Resources,
    "sensor": legacy.Sensor,
    "run_event": legacy.RunEventCondition,
}
_JOB_PARTS = (legacy.Job, legacy.JobGroup, legacy.Schedule, legacy.OnFailure, legacy.Resources)
_BARE_KEY = re.compile(r"^[A-Za-z0-9_-]+$")


class MigrationError(Exception):
    """A declaration the migration cannot write without a person deciding."""


@dataclass
class _Found:
    """The legacy declarations of a project and the module each was found in first."""

    modules: dict[int, Path] = field(default_factory=dict)
    jobs: list[legacy.Job] = field(default_factory=list)
    groups: list[legacy.JobGroup] = field(default_factory=list)
    calendars: list[legacy.CalendarSchedule] = field(default_factory=list)
    sensors: list[legacy.Sensor] = field(default_factory=list)


@contextmanager
def _removed_api() -> Iterator[None]:
    import sqlbuild.orchestration as public

    missing = object()
    saved = {name: getattr(public, name, missing) for name in _REMOVED}
    for name, value in _REMOVED.items():
        setattr(public, name, value)
    try:
        yield
    finally:
        for name, value in saved.items():
            if value is missing:
                delattr(public, name)
            else:
                setattr(public, name, value)


def _collect(value: object, module: Path, found: _Found) -> None:
    pending = [value]
    visited: set[int] = set()
    while pending:
        item = pending.pop()
        if isinstance(item, list | tuple):
            if id(item) not in visited:
                visited.add(id(item))
                pending.extend(item)
            continue
        kinds = (
            (legacy.Job, found.jobs),
            (legacy.JobGroup, found.groups),
            (legacy.CalendarSchedule, found.calendars),
            (legacy.Sensor, found.sensors),
        )
        for kind, bucket in kinds:
            if isinstance(item, kind) and id(item) not in found.modules:
                found.modules[id(item)] = module
                bucket.append(item)
                if isinstance(item, legacy.JobGroup):
                    pending.extend(item.jobs)


def _suffixed(name: str, suffix: str) -> str:
    return name if name.endswith(suffix) and len(name) > len(suffix) else f"{name}{suffix}"


def _value(value: object) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int | float):
        return repr(value)
    if isinstance(value, str):
        return json.dumps(value, ensure_ascii=False)
    if isinstance(value, list | tuple):
        return "[" + ", ".join(_value(item) for item in value) + "]"
    if isinstance(value, dict):
        items = [f"{_key(key)} = {_value(item)}" for key, item in value.items() if item is not None]
        return "{ " + ", ".join(items) + " }" if items else "{}"
    raise MigrationError(f"cannot write {value!r} as TOML")


def _key(key: object) -> str:
    text = str(key)
    return text if _BARE_KEY.match(text) else json.dumps(text, ensure_ascii=False)


def _schedule(
    run: legacy.Job | legacy.JobGroup, calendar: tuple[str, legacy.CalendarSchedule] | None
) -> dict[str, object] | None:
    if run.schedule is not None and calendar is not None:
        raise MigrationError(
            f"'{run.name}' has both a cron schedule and the calendar schedule "
            f"'{calendar[0]}'; a job or job group now has one schedule, so remove one first"
        )
    written: dict[str, object]
    source: legacy.Schedule | legacy.CalendarSchedule
    if run.schedule is not None:
        source = run.schedule
        written = {"cron": source.cron}
    elif calendar is not None:
        source = calendar[1]
        written = {"calendar": calendar[0]}
    else:
        return None
    written["timezone"] = source.timezone
    if source.starts_at is not None:
        written["starts_at"] = source.starts_at
    if source.default_status != "running":
        written["default_status"] = source.default_status
    return written


def _run_lines(run: legacy.Job | legacy.JobGroup, name: str) -> list[str]:
    return [f"name = {_value(name)}", f"description = {_value(run.description)}"]


def _common(
    run: legacy.Job | legacy.JobGroup, calendar: tuple[str, legacy.CalendarSchedule] | None
) -> tuple[list[str], list[str]]:
    lines: list[str] = []
    if run.targets:
        lines.append(f"targets = {_value(list(run.targets))}")
    if run.owner is not None:
        lines.append(f"owner = {_value(run.owner)}")
    if run.overlap != "queue":
        lines.append(f"overlap = {_value(run.overlap)}")
    if run.concurrency_groups:
        lines.append(f"concurrency_groups = {_value(list(run.concurrency_groups))}")
    schedule = _schedule(run, calendar)
    if schedule is not None:
        lines.append(f"schedule = {_value(schedule)}")
    tables: list[str] = []
    if run.on_failure is not None:
        tables += ["", "[on_failure]", f"retries = {_value(run.on_failure.retries)}"]
        if run.on_failure.retry_delay_seconds:
            tables.append(f"retry_delay_seconds = {_value(run.on_failure.retry_delay_seconds)}")
    return lines, tables


def _job_toml(
    job: legacy.Job, name: str, calendar: tuple[str, legacy.CalendarSchedule] | None
) -> str:
    lines = _run_lines(job, name)
    lines.append(f"select = {_value(job.select)}")
    if job.exclude.strip():
        lines.append(f"exclude = {_value(job.exclude)}")
    common, tables = _common(job, calendar)
    lines += common + tables
    if job.resources is not None:
        limits = {
            key: value
            for key, value in (
                ("memory", job.resources.memory),
                ("cpus", job.resources.cpus),
                ("pids", job.resources.pids),
            )
            if value is not None
        }
        lines += ["", "[resources]"] + [f"{key} = {_value(value)}" for key, value in limits.items()]
    settings = dict(job.settings)
    if job.concurrency is not None:
        if "concurrency" in settings:
            raise MigrationError(f"job '{job.name}' declares concurrency twice")
        settings["concurrency"] = job.concurrency
    if settings:
        lines += ["", "[settings]"] + [
            f"{_key(key)} = {_value(value)}" for key, value in settings.items() if value is not None
        ]
    return "\n".join(lines) + "\n"


def _group_toml(
    group: legacy.JobGroup,
    name: str,
    members: list[str],
    calendar: tuple[str, legacy.CalendarSchedule] | None,
) -> str:
    lines = _run_lines(group, name)
    lines.append(f"jobs = {_value(members)}")
    common, tables = _common(group, calendar)
    return "\n".join(lines + common + tables) + "\n"


def _job_parts_only(module: Path, project_dir: Path) -> bool:
    """Whether a module holds nothing but imports, a docstring and job, group or schedule values."""
    tree = ast.parse(module.read_text(encoding="utf-8"))
    loaded = sys.modules.get(_module_name(module, project_dir))
    for statement in tree.body:
        if isinstance(statement, ast.Import | ast.ImportFrom):
            continue
        if isinstance(statement, ast.Expr) and isinstance(statement.value, ast.Constant):
            continue
        targets = (
            statement.targets
            if isinstance(statement, ast.Assign)
            else [statement.target]
            if isinstance(statement, ast.AnnAssign)
            else None
        )
        if targets is None or loaded is None:
            return False
        for target in targets:
            if not isinstance(target, ast.Name):
                return False
            if target.id == "__all__":
                continue
            value = getattr(loaded, target.id, None)
            values = value if isinstance(value, list | tuple) else [value]
            if not values or not all(isinstance(item, _JOB_PARTS) for item in values):
                return False
    return True


def _module_name(module: Path, project_dir: Path) -> str:
    return ".".join(module.relative_to(project_dir).with_suffix("").parts)


def _site(declaration: object, fallback: Path, project_dir: Path) -> str:
    site = getattr(declaration, "_declared_at", None)
    if site is None:
        return fallback.relative_to(project_dir).as_posix()
    path = Path(site[0])
    try:
        path = path.relative_to(project_dir.resolve())
    except ValueError:
        pass
    return f"{path.as_posix()}:{site[1]}"


def migrate_orchestration(*, project_dir: Path, dry_run: bool) -> int:
    """Write TOML for each Python job and job group, print every rename and manual edit."""
    try:
        return _migrate(project_dir, dry_run)
    except MigrationError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


def _migrate(project_dir: Path, dry_run: bool) -> int:
    root = project_dir / _ROOT
    if not root.is_dir():
        print("No orchestration/ folder; nothing to migrate.")
        return 0
    if (root / _RENAMES).exists():
        raise MigrationError(
            f"{_ROOT}/{_RENAMES} already exists; publish every location with it, delete it, "
            "then run the migration again"
        )
    found = _Found()
    modules = sorted(
        path
        for path in root.rglob("*.py")
        if "__pycache__" not in path.parts and path.name != "__init__.py"
    )
    with _removed_api():
        for module in modules:
            loaded = load_orchestration_module(file_path=module, project_dir=project_dir)
            for value in vars(loaded).values():
                _collect(value, module, found)
    if not (found.jobs or found.groups or found.calendars or found.sensors):
        print("No Python jobs, job groups or schedules found; nothing to migrate.")
        return 0
    names: dict[int, str] = {}
    renames: dict[str, dict[str, str]] = {"jobs": {}, "sensors": {}, "triggers": {}}
    printed: list[str] = []
    for runs, suffix, kind, section in (
        (found.jobs, "_job", "job", "jobs"),
        (found.groups, "_job_group", "job group", "jobs"),
        (found.sensors, "_sensor", "sensor", "sensors"),
    ):
        for run in runs:
            new = _suffixed(run.name, suffix)
            names[id(run)] = new
            if new != run.name:
                renames[section][run.name] = new
                printed.append(f"Renamed {kind} {run.name} -> {new}")
    calendars: dict[int, tuple[str, legacy.CalendarSchedule]] = {}
    for schedule in found.calendars:
        if schedule.function is None:
            raise MigrationError("a calendar_schedule does not decorate a function")
        old = schedule.function.__name__
        new = _suffixed(old, "_calendar")
        if id(schedule.job) in calendars:
            raise MigrationError(
                f"'{schedule.job.name}' has two calendar schedules; keep one before migrating"
            )
        calendars[id(schedule.job)] = (new, schedule)
        if new != old:
            printed.append(f"Renamed calendar {old} -> {new}")
        job_name = names.get(id(schedule.job), schedule.job.name)
        if old != job_name:
            renames["triggers"][old] = job_name
    taken: set[str] = set()
    for new in [*names.values(), *(name for name, _ in calendars.values())]:
        if new in taken:
            raise MigrationError(f"two declarations would both be named '{new}'")
        taken.add(new)
    writes: list[tuple[Path, str]] = []
    for job in found.jobs:
        relative = found.modules[id(job)].relative_to(root).with_suffix("")
        writes.append(
            (
                root / "jobs" / relative / f"{names[id(job)]}.toml",
                _job_toml(job, names[id(job)], calendars.get(id(job))),
            )
        )
    for group in found.groups:
        relative = found.modules[id(group)].relative_to(root).with_suffix("")
        members = [names[id(member)] for member in group.jobs]
        writes.append(
            (
                root / "job_groups" / relative / f"{names[id(group)]}.toml",
                _group_toml(group, names[id(group)], members, calendars.get(id(group))),
            )
        )
    if any(renames.values()):
        sections = [
            f"[{section}]\n"
            + "".join(f"{_key(old)} = {_value(new)}\n" for old, new in pairs.items())
            for section, pairs in renames.items()
            if pairs
        ]
        writes.append((root / _RENAMES, "\n".join(sections)))
    for path, _ in writes:
        if path.exists():
            raise MigrationError(f"{path.relative_to(project_dir).as_posix()} already exists")
    holding_others = {found.modules[id(item)] for item in [*found.calendars, *found.sensors]}
    deletions = [
        module
        for module in sorted({found.modules[id(run)] for run in [*found.jobs, *found.groups]})
        if module not in holding_others and _job_parts_only(module, project_dir)
    ]
    manual = _manual_edits(found, (names, calendars), deletions, project_dir)
    verb = "Would write" if dry_run else "Wrote"
    for line in printed:
        print(line)
    for path, text in writes:
        if not dry_run:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
        print(f"{verb} {path.relative_to(project_dir).as_posix()}")
    for module in deletions:
        if not dry_run:
            module.unlink()
        deleted: str = module.relative_to(project_dir).as_posix()
        print(f"{'Would delete' if dry_run else 'Deleted'} {deleted}")
    if manual:
        print("Edit by hand:")
        for line in manual:
            print(f"  {line}")
    if any(renames.values()):
        print(
            f"{_ROOT}/{_RENAMES} keeps run history, pause and sensor state under the new names. "
            "Publish every location, then delete it."
        )
    return 0


def _manual_edits(
    found: _Found,
    named: tuple[dict[int, str], dict[int, tuple[str, legacy.CalendarSchedule]]],
    deletions: list[Path],
    project_dir: Path,
) -> list[str]:
    """The edits a person makes: leftover job declarations, calendars and sensors."""
    names, calendars = named
    manual: list[str] = []
    for module in sorted({found.modules[id(run)] for run in [*found.jobs, *found.groups]}):
        if module not in deletions:
            manual.append(
                f"{module.relative_to(project_dir).as_posix()}: delete the job, job group and "
                "schedule declarations; they are TOML now"
            )
    for new, schedule in calendars.values():
        function_name: str = getattr(schedule.function, "__name__", "")
        targets = (
            f" Its targets {list(schedule.targets)} are dropped: a calendar runs wherever its"
            " job does."
            if schedule.targets
            else ""
        )
        manual.append(
            f"{_site(schedule, found.modules[id(schedule)], project_dir)}: replace the "
            f"calendar_schedule(...) decorator of {function_name} with "
            f"@calendar(name={_value(new)}, description={_value(schedule.description)}, "
            f"horizon={_value(schedule.horizon)}), import calendar from sqlbuild.orchestration, "
            f"and move it to {_ROOT}/calendars/ in a module of its own.{targets}"
        )
    for sensor in found.sensors:
        edits = [
            f"name={_value(names[id(sensor)])}",
            f"run={_value(names.get(id(sensor.run), sensor.run.name))}",
        ]
        if isinstance(sensor.when, legacy.RunEventCondition):
            watched = names.get(id(sensor.when.job), sensor.when.job.name)
            edits.append(f"run_event(job={_value(watched)}, ...)")
        manual.append(
            f"{_site(sensor, found.modules[id(sensor)], project_dir)}: in sensor "
            f"'{sensor.name}', write {', '.join(edits)}, drop the job imports, and move it to "
            f"{_ROOT}/sensors/ in a module of its own"
        )
    return manual
