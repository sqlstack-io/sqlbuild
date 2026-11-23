"""Synthetic DuckDB projects and process helpers for local Recall tests."""

import contextlib
import itertools
import json
import operator
import os
import re
import shlex
import signal
import sqlite3
import stat
import subprocess
import sys
import time
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path
from typing import Any

from tests.unit.src.sqlbuild.python_nodes.helpers import node_module_files

PYTHON_CLI: Path = Path(sys.executable).with_name("sqlbuild-python")
NATIVE_SQB: Path = Path(sys.executable).with_name("sqb")
EMITTED_STDOUT: bytes = b"binary \x00\xff\xfe chunk\n"
EMITTED_STDERR: bytes = "warning \u2713 from the task\n".encode()
EMITTED_TAIL: bytes = b"after the warning\n"
MEBIBYTE: int = 1024 * 1024
HANG_SECONDS: int = 600
HANG_DUMP_GRACE_SECONDS: int = 30
# Only a hang takes this long: a signalled build stops within a second or two.
STOP_HANG_SECONDS: int = 120
EMPTY_RESULT: bytes = b'{"rows": []}'
NUMBERED_LINE: re.Pattern[bytes] = re.compile(rb"^(\d+) std(?:out|err)$", re.MULTILINE)
INCOMPLETE_HISTORY: re.Pattern[str] = re.compile(r"incomplete \((\d+) dropped, (\d+) failed\)")
PROJECT_TOML: str = (
    'name = "orders_shop"\n'
    'adapter = "duckdb"\n'
    'default_target = "dev"\n\n'
    "[connections.local]\n"
    'database = "warehouse.duckdb"\n\n'
    "[targets.dev]\n"
    'connection = "local"\n'
    'schema = "dev"\n'
)
ORDERS_MODEL: str = (
    'MODEL (\n  description "One row per order",\n  materialized table,\n);\n\n'
    "SELECT 1 AS order_id, CAST('2026-04-01' AS DATE) AS ordered_on\n"
)
EMIT_TASK: str = (
    '"""A task that writes exact bytes to both streams."""\n\n'
    "import itertools\n"
    "import os\n"
    "import sys\n"
    "import threading\n"
    "import time\n\n"
    "from sqlbuild.assets import asset\n\n\n"
    "@asset()\n"
    "def emit_bytes(ctx):\n"
    '    """Write binary stdout, UTF-8 stderr, then more stdout."""\n\n'
    "    sys.stdout.flush()\n"
    f"    sys.stdout.buffer.write({EMITTED_STDOUT!r})\n"
    "    sys.stdout.buffer.flush()\n"
    f"    sys.stderr.write({EMITTED_STDERR.decode()!r})\n"
    "    sys.stderr.flush()\n"
    f"    sys.stdout.write({EMITTED_TAIL.decode()!r})\n"
    "    sys.stdout.flush()\n"
    '    time.sleep(float(os.environ.get("ORDERS_TASK_SLEEP_SECONDS", "0")))\n'
    '    sys.stdout.write(os.environ.get("ORDERS_TASK_AFTER_SLEEP", ""))\n'
    '    _write_concurrently(int(os.environ.get("ORDERS_TASK_THREAD_LINES", "0")))\n'
    '    sys.stdout.write("y" * int(os.environ.get("ORDERS_TASK_STDOUT_BYTES", "0")))\n'
    "    sys.stdout.flush()\n"
    "    return ctx.result()\n\n\n"
    "def _write_concurrently(lines):\n"
    '    """Two threads write numbered lines; the number gives the true global order."""\n\n'
    "    lock = threading.Lock()\n"
    "    counter = itertools.count()\n\n"
    "    def writer(name, stream):\n"
    "        for _ in range(lines):\n"
    "            with lock:\n"
    '                stream.write(f"{next(counter)} {name}\\n")\n'
    "                stream.flush()\n\n"
    "    threads = [\n"
    '        threading.Thread(target=writer, args=("stdout", sys.stdout)),\n'
    '        threading.Thread(target=writer, args=("stderr", sys.stderr)),\n'
    "    ]\n"
    "    for thread in threads:\n"
    "        thread.start()\n"
    "    for thread in threads:\n"
    "        thread.join()\n"
)


def write_project(project_dir: Path, *, extra_toml: str = "", targets: int = 1) -> Path:
    """Write a DuckDB project with one model and one byte-emitting task."""

    toml: str = PROJECT_TOML + extra_toml
    for index in range(1, targets):
        toml += (
            f"\n[connections.local_{index}]\n"
            f'database = "warehouse_{index}.duckdb"\n\n'
            f"[targets.dev_{index}]\n"
            f'connection = "local_{index}"\n'
            'schema = "dev"\n'
        )
    files: dict[str, str] = {
        "sqlbuild_project.toml": toml,
        "sql/models/orders.sql": ORDERS_MODEL,
        **node_module_files("python/nodes/tasks/emit.py", EMIT_TASK),
    }
    for relative, contents in files.items():
        path: Path = project_dir / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")
    return project_dir


def environment(**overrides: str) -> dict[str, str]:
    """The process environment for one CLI run; a hang dump prints every thread's stack."""

    return {**os.environ, "NO_COLOR": "1", "PYTHONFAULTHANDLER": "1", **overrides}


def run(project_dir: Path, *args: str, **env: str) -> subprocess.CompletedProcess[bytes]:
    """Run the Python CLI once in the project and capture exact streams, or fail with its stacks."""

    process: subprocess.Popen[bytes] = start(
        project_dir, *args, interrupts=signal.SIG_DFL, program=PYTHON_CLI, **env
    )
    try:
        stdout, stderr = process.communicate(timeout=HANG_SECONDS)
    except subprocess.TimeoutExpired:
        raise AssertionError(
            f"sqlbuild {shlex.join(args)} hung for {HANG_SECONDS}s:\n{hang_dump(process)}"
        ) from None
    return subprocess.CompletedProcess(process.args, process.returncode, stdout, stderr)


def hang_dump(process: subprocess.Popen[bytes]) -> str:
    """Abort a hung CLI's process group so it prints every thread's stack, and return its stderr."""

    with contextlib.suppress(ProcessLookupError):
        os.killpg(process.pid, signal.SIGABRT)
    try:
        _, stderr = process.communicate(timeout=HANG_DUMP_GRACE_SECONDS)
    except subprocess.TimeoutExpired:
        with contextlib.suppress(ProcessLookupError):
            os.killpg(process.pid, signal.SIGKILL)
        _, stderr = process.communicate()
    return stderr.decode(errors="replace")


def start(
    project_dir: Path,
    *args: str,
    interrupts: signal.Handlers = signal.SIG_DFL,
    program: Path = PYTHON_CLI,
    **env: str,
) -> subprocess.Popen[bytes]:
    """Start `program` in its own process group with SIGINT set to `interrupts`.

    The default gives the CLI Ctrl-C handling even when this runner ignores SIGINT."""

    return subprocess.Popen(
        [str(program), *args],
        cwd=project_dir,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=environment(**env),
        start_new_session=True,
        preexec_fn=lambda: signal.signal(signal.SIGINT, interrupts),
    )


def recall_rows(project_dir: Path, sql: str) -> list[list[object]]:
    """The rows `sqb recall query --json` returns."""

    completed: subprocess.CompletedProcess[bytes] = run(
        project_dir, "recall", "query", "--json", "--no-limit", sql
    )
    assert completed.returncode == 0, completed.stderr.decode(errors="replace")
    return json.loads(completed.stdout)["rows"]


def wait_for(predicate_sql: str, project_dir: Path, *, timeout: float) -> list[list[object]]:
    """Poll the store until `predicate_sql` returns a row; a store not yet created has none."""

    deadline: float = time.monotonic() + timeout
    rows: list[list[object]] = []
    while time.monotonic() < deadline and not rows:
        time.sleep(0.2)
        completed: subprocess.CompletedProcess[bytes] = run(
            project_dir, "recall", "query", "--json", predicate_sql
        )
        rows = json.loads(completed.stdout or EMPTY_RESULT).get("rows", [])
    return rows


def build_with_read_only_logs(
    project_dir: Path,
) -> tuple[subprocess.CompletedProcess[bytes], subprocess.CompletedProcess[bytes]]:
    """Two builds while the project's `logs` directory cannot be written."""

    logs: Path = project_dir / "logs"
    logs.mkdir(parents=True)
    logs.chmod(stat.S_IRUSR | stat.S_IXUSR)
    try:
        return run(project_dir, "build"), run(project_dir, "build")
    finally:
        logs.chmod(stat.S_IRWXU)


def killed_build(project_dir: Path, *, heartbeat_seconds: str) -> tuple[str, list[list[object]]]:
    """The running invocation a SIGKILLed build left, and the store's rows for it afterwards."""

    process: subprocess.Popen[bytes] = start(
        project_dir,
        "build",
        SQLBUILD_RECALL_HEARTBEAT_SECONDS=heartbeat_seconds,
        ORDERS_TASK_SLEEP_SECONDS="60",
    )
    running: list[list[object]] = wait_for(
        "SELECT invocation_id FROM invocations WHERE status = 'running' "
        "AND invocation_id IN (SELECT invocation_id FROM recall_output)",
        project_dir,
        timeout=120,
    )
    os.killpg(process.pid, signal.SIGKILL)
    _ = process.wait(timeout=30)
    time.sleep(5 * float(heartbeat_seconds))
    recorded: list[list[object]] = recall_rows(
        project_dir,
        "SELECT i.invocation_id, i.status, i.ended_at IS NOT NULL, i.ended_at_estimated, "
        "o.complete "
        "FROM invocations i JOIN output_invocations o USING (invocation_id)",
    )
    return str(running[0][0]), recorded


def killed_during_discovery(
    project_dir: Path, *, heartbeat_seconds: str
) -> tuple[list[list[object]], subprocess.CompletedProcess[bytes]]:
    """Rows left by a build SIGKILLed while importing a slow sink, and `recall output last`."""

    process: subprocess.Popen[bytes] = start(
        write_slow_import_sink(project_dir),
        "build",
        SQLBUILD_RECALL_HEARTBEAT_SECONDS=heartbeat_seconds,
        ORDERS_IMPORT_SECONDS="60",
    )
    _ = wait_for(
        "SELECT invocation_id FROM invocations WHERE status = 'running'",
        project_dir,
        timeout=120,
    )
    os.killpg(process.pid, signal.SIGKILL)
    _ = process.wait(timeout=30)
    time.sleep(5 * float(heartbeat_seconds))
    recorded: list[list[object]] = recall_rows(
        project_dir, "SELECT command, status, started_at IS NOT NULL FROM invocations"
    )
    return recorded, run(project_dir, "recall", "output", "last")


def write_slow_import_sink(project_dir: Path) -> Path:
    """Add a sink module whose import sleeps for `ORDERS_IMPORT_SECONDS`, slowing discovery."""

    sink: Path = project_dir / "python" / "event_sinks" / "slow_import.py"
    sink.parent.mkdir(parents=True, exist_ok=True)
    _ = sink.write_text(
        "import os\nimport sys\nimport time\n\n"
        "time.sleep(float(os.environ.get('ORDERS_IMPORT_SECONDS', '0')))\n"
        "sys.stdout.write(os.environ.get('ORDERS_IMPORT_AFTER_SLEEP', ''))\n",
        encoding="utf-8",
    )
    return project_dir


def terminated_build(
    project_dir: Path,
    *,
    wait_sql: str,
    interrupts: signal.Handlers = signal.SIG_DFL,
    program: Path = PYTHON_CLI,
    **env: str,
) -> subprocess.CompletedProcess[bytes]:
    """A build sent SIGTERM, as an orchestrator stops it, once `wait_sql` returns a row."""

    process: subprocess.Popen[bytes] = start(
        project_dir, "build", interrupts=interrupts, program=program, **env
    )
    try:
        _ = wait_for(wait_sql, project_dir, timeout=120)
        process.send_signal(signal.SIGTERM)
        stdout, stderr = process.communicate(timeout=STOP_HANG_SECONDS)
    except subprocess.TimeoutExpired:
        raise AssertionError(f"a terminated build hung:\n{hang_dump(process)}") from None
    return subprocess.CompletedProcess(process.args, process.returncode, stdout, stderr)


def main_thread_state(pid: int) -> tuple[str, str, str]:
    """The scheduler state and user and system CPU ticks of the process's main thread."""

    fields: list[str] = (
        Path(f"/proc/{pid}/task/{pid}/stat").read_text(encoding="utf-8").rsplit(")", 1)[1].split()
    )
    return fields[0], fields[11], fields[12]


def wait_until_blocked(pid: int, *, samples: int = 5) -> None:
    """Wait until the main thread has slept without using CPU for `samples` polls in a row.

    A thread waiting for the CPU reports `R`, so load does not pass for blocked."""

    seen: list[tuple[str, str, str]] = [main_thread_state(pid)]
    deadline: float = time.monotonic() + STOP_HANG_SECONDS
    while (seen[-samples:] != [seen[-1]] * samples or seen[-1][0] != "S") and (
        time.monotonic() < deadline
    ):
        time.sleep(0.1)
        seen.append(main_thread_state(pid))
    assert seen[-samples:] == [seen[-1]] * samples and seen[-1][0] == "S", (
        f"the build's main thread never blocked, so it hung: {seen[-samples:]}"
    )


def terminated_while_closing(project_dir: Path) -> subprocess.CompletedProcess[bytes]:
    """A build sent SIGTERM while it waits for a locked Recall store at exit."""

    with held_store_lock(project_dir):
        process: subprocess.Popen[bytes] = start(project_dir, "build", PYTHONUNBUFFERED="1")
        assert process.stdout is not None
        printed: bytes = b""
        while b"Completed" not in printed and process.poll() is None:
            printed += process.stdout.readline()
        wait_until_blocked(process.pid)
        process.send_signal(signal.SIGTERM)
        try:
            stdout, stderr = process.communicate(timeout=STOP_HANG_SECONDS)
        except subprocess.TimeoutExpired:
            raise AssertionError(f"a closing build hung:\n{hang_dump(process)}") from None
    return subprocess.CompletedProcess(process.args, process.returncode, printed + stdout, stderr)


def interrupted_build(project_dir: Path, **env: str) -> int | None:
    """Exit code of a build sent SIGINT once its task has printed; `None` when it hung."""

    process: subprocess.Popen[bytes] = start(
        project_dir,
        "build",
        interrupts=signal.SIG_DFL,
        program=PYTHON_CLI,
        PYTHONUNBUFFERED="1",
        **env,
    )
    try:
        assert process.stdout is not None
        printed: bytes = b""
        while EMITTED_TAIL not in printed and process.poll() is None:
            printed += process.stdout.readline()
        os.killpg(process.pid, signal.SIGINT)
        try:
            _ = process.communicate(timeout=STOP_HANG_SECONDS)
        except subprocess.TimeoutExpired:
            return None
        return process.returncode
    finally:
        with contextlib.suppress(ProcessLookupError):
            os.killpg(process.pid, signal.SIGKILL)
        _ = process.wait(timeout=30)


def concurrent_builds(project_dir: Path, targets: tuple[str, ...]) -> tuple[list[int], int, int]:
    """Exit codes of concurrent builds, reads made meanwhile and unexpected read failures."""

    _ = run(project_dir, "recall", "query", "SELECT 1")
    builds: list[subprocess.Popen[bytes]] = [
        start(project_dir, "build", "--target", target) for target in targets
    ]
    reads: int = 0
    unexpected_failures: int = 0
    deadline: float = time.monotonic() + HANG_SECONDS
    while any(build.poll() is None for build in builds) and time.monotonic() < deadline:
        reader: subprocess.CompletedProcess[bytes] = run(
            project_dir, "recall", "query", "--json", "SELECT COUNT(*) FROM invocations"
        )
        reads += 1
        unexpected_failures += int(reader.returncode != 0 and b"[C971]" not in reader.stderr)
    hung: list[str] = list(map(hang_dump, filter(_still_running, builds)))
    assert not hung, f"{len(hung)} builds still ran after {HANG_SECONDS}s:\n" + "\n".join(hung)
    return [build.wait(timeout=600) for build in builds], reads, unexpected_failures


def _still_running(process: subprocess.Popen[bytes]) -> bool:
    return process.poll() is None


def timed_build(project_dir: Path) -> tuple[subprocess.CompletedProcess[bytes], float]:
    """A build with the seconds until it exited."""

    started: float = time.monotonic()
    completed: subprocess.CompletedProcess[bytes] = run(project_dir, "build")
    return completed, time.monotonic() - started


@contextmanager
def held_store_lock(project_dir: Path) -> Iterator[None]:
    """Another process holds the store's write lock while the block runs, creating it if absent."""

    store: Path = project_dir / "logs" / "recall.sqlite3"
    store.parent.mkdir(parents=True, exist_ok=True)
    holder: subprocess.Popen[bytes] = subprocess.Popen(
        [
            sys.executable,
            "-c",
            "import sqlite3, sys, time\n"
            "connection = sqlite3.connect(sys.argv[1], isolation_level=None)\n"
            "connection.execute('BEGIN EXCLUSIVE')\n"
            "print('locked', flush=True)\n"
            "time.sleep(600)\n",
            str(store),
        ],
        stdout=subprocess.PIPE,
    )
    try:
        assert holder.stdout is not None
        _ = holder.stdout.readline()
        yield
    finally:
        holder.kill()
        _ = holder.wait(timeout=30)


def build_while_store_locked(
    project_dir: Path,
) -> tuple[subprocess.CompletedProcess[bytes], float]:
    """A timed build while another process holds the store's write lock."""

    with held_store_lock(project_dir):
        return timed_build(project_dir)


def compute_log_streams(project_dir: Path, invocation_id: str) -> tuple[bytes, bytes]:
    """The stdout and stderr bytes compute logs captured for one invocation."""

    capture: Path = next((project_dir / "logs").glob(f"*/{invocation_id}"))
    return (capture / "stdout.log").read_bytes(), (capture / "stderr.log").read_bytes()


def write_corrupt_store(project_dir: Path) -> None:
    """Replace the Recall store with a file that is not a database."""

    store: Path = project_dir / "logs" / "recall.sqlite3"
    store.parent.mkdir(parents=True, exist_ok=True)
    _ = store.write_bytes(b"this file is not a database\n")


FAILING_RECALL_STDERR: str = (
    "import sys\n\n\n"
    "class _RejectsRecallNotices:\n"
    "    def __init__(self, stream):\n"
    "        self._stream = stream\n\n"
    "    def write(self, text):\n"
    '        if "Recall" in text:\n'
    '            raise OSError("stderr rejected the Recall notice")\n'
    "        return self._stream.write(text)\n\n"
    "    def __getattr__(self, name):\n"
    "        return getattr(self._stream, name)\n\n\n"
    "sys.stderr = _RejectsRecallNotices(sys.stderr)\n"
)


def build_with_failing_recall_notices(project_dir: Path, site_dir: Path) -> int:
    """The exit code of a build whose Recall notices fail to write, while the store is locked."""

    site_dir.mkdir(parents=True, exist_ok=True)
    _ = (site_dir / "sitecustomize.py").write_text(FAILING_RECALL_STDERR, encoding="utf-8")
    with held_store_lock(project_dir):
        return run(project_dir, "build", PYTHONPATH=str(site_dir)).returncode


def prepared_project(parent: Path, extra_toml: str, *, builds: int) -> Path:
    """A project under `parent`, built `builds` times first."""

    project_dir: Path = write_project(parent / "orders_shop", extra_toml=extra_toml)
    _ = [run(project_dir, "build") for _ in range(builds)]
    return project_dir


def numbered_lines(output: bytes) -> list[int]:
    """The leading numbers of the numbered lines in `output`, in order."""

    return [int(number) for number in NUMBERED_LINE.findall(output)]


def simultaneous_builds(project_dir: Path, targets: tuple[str, ...]) -> list[int]:
    """Exit codes of builds of `targets`, each with its own warehouse, started together."""

    started: list[subprocess.Popen[bytes]] = [
        start(project_dir, "build", "--target", target) for target in targets
    ]
    return [process.wait(timeout=600) for process in started]


RECALL_SINK_IMPORTS: str = (
    "from pathlib import Path\n\n"
    "from sqlbuild.recall import RecallWriter\n"
    "from sqlbuild.event_sinks import (\n"
    "    CommandOutputRecord, LifecycleEvent, command_output_event_sink, lifecycle_event_sink,\n"
    ")\n\n"
)
EVENT_LOG_SINK: str = (
    "@lifecycle_event_sink\n"
    "def event_log(event: LifecycleEvent) -> None:\n"
    "    with (Path(__file__).parents[1] / 'events.log').open('a') as log:\n"
    "        log.write(event.event_type + '\\n')\n"
)
OUTPUT_LOG_SINK: str = (
    "@command_output_event_sink\n"
    "def output_log(record: CommandOutputRecord) -> None:\n"
    "    with (Path(__file__).parents[1] / 'output.log').open('a') as log:\n"
    "        log.write(record.message)\n"
)
SLOW_LIFECYCLE_SINK: str = (
    "@lifecycle_event_sink\n"
    "def recall_events(event: LifecycleEvent, writer: RecallWriter) -> None:\n"
    "    import os\n"
    "    import time\n\n"
    "    time.sleep(float(os.environ.get('ORDERS_SINK_SLEEP_SECONDS', '0')))\n"
    "    writer.publish_lifecycle(event)\n"
)
FAILING_SINKS: str = (
    "@lifecycle_event_sink\n"
    "def recall_events(event: LifecycleEvent, writer: RecallWriter) -> None:\n"
    "    raise RuntimeError('lifecycle sink failed')\n\n"
    "@command_output_event_sink\n"
    "def recall_output(record: CommandOutputRecord, writer: RecallWriter) -> None:\n"
    "    raise RuntimeError('output sink failed')\n"
)


def filtering_lifecycle_sink(dropped_prefix: str) -> str:
    """A Recall lifecycle replacement keeping every event whose type lacks `dropped_prefix`."""

    return (
        "@lifecycle_event_sink\n"
        "def recall_events(event: LifecycleEvent, writer: RecallWriter) -> None:\n"
        f"    if not event.event_type.startswith({dropped_prefix!r}):\n"
        "        writer.publish_lifecycle(event)\n"
    )


def rewriting_output_sink(replaced: tuple[bytes, bytes]) -> str:
    """A Recall output replacement storing each chunk with `replaced[0]` as `replaced[1]`."""

    old, new = replaced
    return (
        "@command_output_event_sink\n"
        "def recall_output(record: CommandOutputRecord, writer: RecallWriter) -> None:\n"
        f"    data = record.data.replace({old!r}, {new!r})\n"
        "    writer.publish_command_output(record.replace(data=data))\n"
    )


def write_sinks(project_dir: Path, sinks: dict[str, str]) -> Path:
    """Add `python/event_sinks/<name>.py` modules holding the given sink definitions."""

    for name, source in sinks.items():
        path: Path = project_dir / "python" / "event_sinks" / f"{name}.py"
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(RECALL_SINK_IMPORTS + source, encoding="utf-8")
    return project_dir


def logged_event_types(project_dir: Path, dropped_prefix: str) -> tuple[list[str], list[str]]:
    """The sorted event types `event_log` wrote, then those not starting with `dropped_prefix`."""

    logged: list[str] = sorted(
        (project_dir / "events.log").read_text(encoding="utf-8").splitlines()
    )
    dropped: operator.methodcaller = operator.methodcaller("startswith", dropped_prefix)
    return logged, list(itertools.filterfalse(dropped, logged))


DEPLOYED_REVISION: str = "4f1c2e9"
SPLIT_CREDENTIALS_TOML: str = (
    '\n[connections.writer]\nadapter = "sqlite"\npath = "${env:ORDERS_RECALL_WRITER_PATH}"\n'
    '\n[connections.reader]\nadapter = "sqlite"\npath = "${env:ORDERS_RECALL_READER_PATH}"\n'
    '\n[recall]\nwrite_connection = "writer"\nread_connection = "reader"\n'
)
SHARED_STORE_TOML: str = (
    '\n[connections.history]\nadapter = "sqlite"\npath = "{path}"\n\n[recall]\n{keys}'
)


def shared_store_project(*, parent: Path, store: Path, keys: str) -> Path:
    """A project whose Recall settings set `keys` for the SQLite connection `history` at `store`."""

    return write_project(
        parent / "orders_shop",
        extra_toml=SHARED_STORE_TOML.format(path=store.as_posix(), keys=keys),
    )


def store_schema(store: Path) -> list[tuple[object, ...]]:
    """Every table and view of a SQLite store with its columns, to show reads changed nothing."""

    with sqlite3.connect(store) as connection:
        return connection.execute(
            "SELECT m.type, m.name, p.name FROM sqlite_schema AS m "
            "LEFT JOIN pragma_table_xinfo(m.name) AS p ORDER BY m.type, m.name, p.cid"
        ).fetchall()


def make_store_behind(store: Path) -> None:
    """Make a store look written by an older version: no newer column, views or version row."""

    with sqlite3.connect(store) as connection:
        views: list[str] = [
            row[0]
            for row in connection.execute("SELECT name FROM sqlite_schema WHERE type = 'view'")
        ]
        connection.executescript(
            "".join(f'DROP VIEW "{view}";' for view in views)
            + 'ALTER TABLE "recall_heartbeats" DROP COLUMN "subcommand";'
            + "DELETE FROM recall_meta WHERE component = 'schema';"
        )
    connection.close()


CI_VARIABLES: tuple[str, ...] = (
    "CI",
    "GITHUB_ACTIONS",
    "GITLAB_CI",
    "BUILDKITE",
    "JENKINS_URL",
    "CIRCLECI",
    "TF_BUILD",
    "TEAMCITY_VERSION",
    "BITBUCKET_BUILD_NUMBER",
    "CODEBUILD_BUILD_ID",
)
NO_CI: dict[str, str] = dict.fromkeys(CI_VARIABLES, "")


def copy_invocation_row(store: Path, *, invocation_id: str, external_context: str) -> None:
    """Store a copy of the first context row under another ID, as an older writer left it."""

    with sqlite3.connect(store) as connection:
        cursor: sqlite3.Cursor = connection.execute("SELECT * FROM recall_invocations LIMIT 1")
        names: list[str] = [column[0] for column in cursor.description]
        row: dict[str, object] = dict(zip(names, cursor.fetchone(), strict=True))
        row.update(
            invocation_id=invocation_id,
            external_context=external_context,
            origin=None,
            command_mode=None,
            content_hash=f"copy-{invocation_id}",
        )
        _ = connection.execute(
            f"INSERT INTO recall_invocations ({', '.join(names)}) "
            f"VALUES ({', '.join('?' for _ in names)})",
            [row[name] for name in names],
        )
    connection.close()


def drop_store_views(store: Path) -> None:
    """Drop every view of a SQLite store, as a store whose creation is still under way has none."""

    with sqlite3.connect(store) as connection:
        views: list[str] = [
            row[0]
            for row in connection.execute("SELECT name FROM sqlite_schema WHERE type = 'view'")
        ]
        connection.executescript("".join(f'DROP VIEW "{view}";' for view in views))
    connection.close()


def git(directory: Path, *args: str) -> str:
    """Run git with a fixed synthetic identity and return its stdout."""

    completed: subprocess.CompletedProcess[str] = subprocess.run(
        ["git", "-c", "user.name=Orders", "-c", "user.email=orders@example.com", *args],
        cwd=directory,
        capture_output=True,
        check=True,
        text=True,
    )
    return completed.stdout.strip()


def committed_repository(parent: Path) -> tuple[Path, str]:
    """A git repository at `parent / "repository"` holding the project, and its commit."""

    repository: Path = parent / "repository"
    _ = write_project(repository / "orders_shop")
    _ = git(repository, "init", "-q", "-b", "main")
    _ = git(repository, "add", ".")
    _ = git(repository, "commit", "-q", "-m", "orders")
    return repository, git(repository, "rev-parse", "HEAD")


def deployed_project(parent: Path) -> tuple[Path, str]:
    """A project outside any repository, and the revision its deployment sets in the environment."""

    return write_project(parent / "orders_shop"), DEPLOYED_REVISION


def checkout_project(parent: Path) -> tuple[Path, str]:
    """A project in a plain checkout, and the commit HEAD names."""

    repository, revision = committed_repository(parent)
    return repository / "orders_shop", revision


def worktree_project(parent: Path) -> tuple[Path, str]:
    """A project in a linked git worktree, whose `.git` is a file, and the commit HEAD names."""

    repository, revision = committed_repository(parent)
    _ = git(repository, "worktree", "add", "-q", "-b", "orders", str(parent / "linked"))
    return parent / "linked" / "orders_shop", revision


RECALL_CONCEPT_PAGE: Path = (
    Path(__file__).resolve().parents[6]
    / "website/src/content/docs/docs/concepts/observability/recall.mdx"
)
_COMMON_QUESTIONS: str = "### Common questions"
_BASH_BLOCK: re.Pattern[str] = re.compile(r"```bash\n(.*?)```", re.DOTALL)


def documented_commands(*, invocation_id: str) -> list[list[str]]:
    """Each command's arguments under the Recall page's common questions, for one invocation."""

    page: str = RECALL_CONCEPT_PAGE.read_text(encoding="utf-8")
    section: str = page.split(_COMMON_QUESTIONS, 1)[1].split("\n### ", 1)[0]
    return [
        shlex.split(block.replace("<invocation_id>", invocation_id))[1:]
        for block in _BASH_BLOCK.findall(section)
    ]


def selected_columns(*, result: dict[str, Any], columns: tuple[str, ...]) -> list[list[object]]:
    """The `columns` of each row of a `--json` query result, in that order."""

    positions: list[int] = [result["columns"].index(column) for column in columns]
    rows: list[list[object]] = []
    for row in result["rows"]:
        rows.append([row[position] for position in positions])
    return rows
