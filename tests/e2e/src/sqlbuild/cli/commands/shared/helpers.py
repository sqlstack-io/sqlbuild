"""Shared e2e test helpers for CLI command tests."""

from __future__ import annotations

import fcntl
import os
import pty
import re
import shlex
import signal
import struct
import subprocess
import termios
import threading
from collections.abc import Callable, Mapping
from functools import partial
from pathlib import Path
from shutil import copytree
from typing import Any, cast

REPO_ROOT: Path = Path(__file__).resolve().parents[7]
WAFFLE_SHOP_DIR: Path = REPO_ROOT / "tests" / "e2e" / "fixtures" / "waffle_shop"
SPINNER_GLYPHS: tuple[str, ...] = ("⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏")
SOURCE_LOADER_STRATEGIES_DIR: Path = (
    REPO_ROOT / "tests" / "e2e" / "fixtures" / "source_loader_strategies"
)
# Far above any healthy command, so only a hang trips it: one test fails instead of the CI job.
SQB_COMMAND_TIMEOUT_SECONDS: float = 600.0
_HANG_DUMP_GRACE_SECONDS: float = 30.0
_HANG_OUTPUT_TAIL_CHARACTERS: int = 20_000


def prepare_waffle_shop(tmp_path: Path) -> Path:
    """Copy waffle shop project to tmp dir with a fresh DuckDB target path."""

    project_dir: Path = tmp_path / "waffle_shop"
    copytree(WAFFLE_SHOP_DIR, project_dir)

    db_path: Path = project_dir / "waffle_shop.duckdb"
    db_path.unlink(missing_ok=True)

    return project_dir


def prepare_source_loader_strategies(
    *,
    tmp_path: Path,
    project_toml: str | None = None,
) -> Path:
    """Copy source-loader strategy fixture to tmp dir with optional warehouse config."""

    project_dir: Path = tmp_path / "source_loader_strategies"
    copytree(SOURCE_LOADER_STRATEGIES_DIR, project_dir)

    db_path: Path = project_dir / "source_loader_strategies.duckdb"
    db_path.unlink(missing_ok=True)

    for project_config in (project_toml,) * int(project_toml is not None):
        assert project_config is not None
        (project_dir / "sqlbuild_project.toml").write_text(project_config, encoding="utf-8")

    return project_dir


def prepare_inline_project(
    *, tmp_path: Path, project_name: str, repo_files: Mapping[str, str]
) -> Path:
    """Write an inline-authored project to tmp dir and return its root path."""

    project_dir: Path = tmp_path / project_name
    project_dir.mkdir(parents=True, exist_ok=True)

    relative_path: str
    contents: str
    for relative_path, contents in repo_files.items():
        file_path: Path = project_dir / relative_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(contents, encoding="utf-8")

    return project_dir


def prepare_dbt_profile_workspace(*, tmp_path: Path, profiles_yml: str) -> Path:
    """Write a minimal dbt project/profiles pair for dbt-profile E2Es."""

    workspace: Path = tmp_path / "workspace"
    dbt_project_dir: Path = workspace / "dbt_project"
    profiles_dir: Path = workspace / "profiles"
    (dbt_project_dir / "models").mkdir(parents=True)
    profiles_dir.mkdir(parents=True)
    (dbt_project_dir / "dbt_project.yml").write_text(
        "name: analytics\n"
        "profile: analytics\n"
        "model-paths: ['models']\n"
        "target-path: target\n"
        "models:\n"
        "  analytics:\n"
        "    +materialized: table\n",
        encoding="utf-8",
    )
    (profiles_dir / "profiles.yml").write_text(profiles_yml, encoding="utf-8")
    write_dbt_profile_orders_model(workspace=workspace, order_id=1)
    return workspace


def write_dbt_profile_orders_model(*, workspace: Path, order_id: int) -> None:
    """Write the mutable dbt model used by adapter dbt-profile E2Es."""

    (workspace / "dbt_project" / "sql" / "models" / "dbt_orders.sql").write_text(
        f"select {order_id} as order_id\n",
        encoding="utf-8",
    )


def add_dbt_profile_downstream_model(*, sqlbuild_project_dir: Path) -> None:
    """Add a SQLBuild model downstream of the dbt profile E2E model."""

    models_dir: Path = sqlbuild_project_dir / "sql" / "models"
    models_dir.mkdir(exist_ok=True)
    (models_dir / "downstream_orders.sql").write_text(
        "MODEL (description 'Test model downstream_orders.', materialized table);\n\n"
        'SELECT order_id FROM __dbt_ref("analytics", "dbt_orders")\n',
        encoding="utf-8",
    )


def assert_dbt_profile_lifecycle(
    *,
    tmp_path: Path,
    profiles_yml: str,
    env: Mapping[str, str] | None,
    fetch_rows: Callable[[str], tuple[tuple[object, ...], ...]],
    no_profile_tables_exist: Callable[[], bool],
    dbt_orders_sql: str,
    downstream_orders_sql: str,
    expected_toml_fragments: tuple[str, ...],
    unexpected_toml_fragments: tuple[str, ...] = (),
) -> None:
    """Assert the common dbt-profile init/build lifecycle against a real warehouse."""

    workspace: Path = prepare_dbt_profile_workspace(
        tmp_path=tmp_path,
        profiles_yml=profiles_yml,
    )
    sqlbuild_project_dir: Path = workspace / "sqlbuild_project"

    init_result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "dbt",
            "init",
            "--project-dir",
            "dbt_project",
            "--profiles-dir",
            "profiles",
            "--skip-dbt-debug",
        ),
        project_dir=workspace,
        env=env,
    )
    assert init_result.returncode == 0, init_result.stdout + init_result.stderr
    generated_toml: str = (sqlbuild_project_dir / "sqlbuild_project.toml").read_text(
        encoding="utf-8"
    )
    for fragment in expected_toml_fragments:
        assert fragment in generated_toml
    for fragment in unexpected_toml_fragments:
        assert fragment not in generated_toml

    add_dbt_profile_downstream_model(sqlbuild_project_dir=sqlbuild_project_dir)
    assert no_profile_tables_exist()

    plain_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "dbt", "build", "--select", "+downstream_orders"),
        project_dir=sqlbuild_project_dir,
        env=env,
    )
    assert plain_result.returncode == 0, plain_result.stdout + plain_result.stderr
    assert "added for SQLBuild dependencies: 1 dbt resources" in plain_result.stdout, (
        plain_result.stdout + plain_result.stderr
    )
    assert fetch_rows(dbt_orders_sql) == ((1,),)
    assert fetch_rows(downstream_orders_sql) == ((1,),)

    noop_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "dbt", "build", "--select", "+downstream_orders"),
        project_dir=sqlbuild_project_dir,
        env=env,
    )
    assert noop_result.returncode == 0, noop_result.stdout + noop_result.stderr
    assert "Skipping dbt" in noop_result.stdout, noop_result.stdout + noop_result.stderr
    assert "Skipping SQLBuild" in noop_result.stdout, noop_result.stdout + noop_result.stderr

    write_dbt_profile_orders_model(workspace=workspace, order_id=2)
    changed_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "dbt", "build", "--select", "+downstream_orders"),
        project_dir=sqlbuild_project_dir,
        env=env,
    )
    assert changed_result.returncode == 0, changed_result.stdout + changed_result.stderr
    assert fetch_rows(dbt_orders_sql) == ((2,),)
    assert fetch_rows(downstream_orders_sql) == ((2,),)


def run_sqb(
    *,
    command: tuple[str, ...],
    project_dir: Path,
    env: Mapping[str, str] | None = None,
    input_text: str | None = None,
    working_dir: Path | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run an sqb CLI command via subprocess and return the result."""

    process_env: dict[str, str] = dict(os.environ)
    process_env.update(env or {})
    process_env.setdefault("PYTHONFAULTHANDLER", "1")

    return run_with_hang_dump(
        argv=("uv", "run", "sqb", "--project-dir", str(project_dir), *command),
        cwd=working_dir or REPO_ROOT,
        env=process_env,
        input_text=input_text,
        timeout_seconds=SQB_COMMAND_TIMEOUT_SECONDS,
    )


def run_with_hang_dump(
    *,
    argv: tuple[str, ...],
    cwd: Path,
    env: Mapping[str, str],
    input_text: str | None,
    timeout_seconds: float,
) -> subprocess.CompletedProcess[str]:
    """Run `argv` to completion, or fail with every Python thread's stack once it hangs.

    Standard input carries `input_text`, or ends at once without it. The command runs in its own
    process group. Past `timeout_seconds` the whole group receives
    SIGABRT, so Python processes started with `PYTHONFAULTHANDLER` print all thread stacks to
    stderr before they exit, and the group is then killed.
    """

    process: subprocess.Popen[str] = subprocess.Popen(
        argv,
        cwd=cwd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=dict(env),
        process_group=0,
    )
    try:
        stdout, stderr = process.communicate(input=input_text, timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        stdout, stderr = _dump_and_kill_group(process=process)
        raise AssertionError(
            f"{shlex.join(argv)} did not finish within {timeout_seconds:.0f}s; "
            "its process group was aborted to dump thread stacks.\n"
            f"--- stdout (last {_HANG_OUTPUT_TAIL_CHARACTERS} characters) ---\n"
            f"{stdout[-_HANG_OUTPUT_TAIL_CHARACTERS:]}\n"
            f"--- stderr (last {_HANG_OUTPUT_TAIL_CHARACTERS} characters) ---\n"
            f"{stderr[-_HANG_OUTPUT_TAIL_CHARACTERS:]}"
        ) from None
    return subprocess.CompletedProcess(
        args=list(argv), returncode=process.returncode, stdout=stdout, stderr=stderr
    )


def _dump_and_kill_group(*, process: subprocess.Popen[str]) -> tuple[str, str]:
    _signal_group(process_group=process.pid, signal_number=signal.SIGABRT)
    try:
        stdout, stderr = process.communicate(timeout=_HANG_DUMP_GRACE_SECONDS)
    except subprocess.TimeoutExpired:
        _signal_group(process_group=process.pid, signal_number=signal.SIGKILL)
        stdout, stderr = process.communicate()
    _signal_group(process_group=process.pid, signal_number=signal.SIGKILL)
    return stdout or "", stderr or ""


def _signal_group(*, process_group: int, signal_number: signal.Signals) -> None:
    try:
        os.killpg(process_group, signal_number)
    except ProcessLookupError:
        return


def run_sqb_with_pty(
    *,
    command: tuple[str, ...],
    project_dir: Path,
    input_text: str = "",
    timeout_seconds: float = 60.0,
    columns: int = 120,
) -> subprocess.CompletedProcess[str]:
    """Run sqb through a real PTY of the given width and return the raw terminal output."""

    master_fd: int
    slave_fd: int
    master_fd, slave_fd = pty.openpty()
    fcntl.ioctl(slave_fd, termios.TIOCSWINSZ, struct.pack("HHHH", 40, columns, 0, 0))
    process_env: dict[str, str] = dict(os.environ)
    for variable in _PTY_UNSET_ENVIRONMENT_VARIABLES:
        process_env.pop(variable, None)
    process_env["TERM"] = "xterm-256color"
    process: subprocess.Popen[bytes] = subprocess.Popen(
        ["uv", "run", "sqb", "--project-dir", str(project_dir), *command],
        cwd=REPO_ROOT,
        stdin=slave_fd,
        stdout=slave_fd,
        stderr=slave_fd,
        env=process_env,
        close_fds=True,
    )
    os.close(slave_fd)
    output_parts: list[bytes] = []
    reader_done: threading.Event = threading.Event()
    reader: threading.Thread = threading.Thread(
        target=_capture_pty_output,
        kwargs={"master_fd": master_fd, "output_parts": output_parts, "reader_done": reader_done},
        daemon=True,
    )
    reader.start()
    try:
        os.write(master_fd, input_text.encode())
        process.wait(timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()
        raise subprocess.TimeoutExpired(command, timeout_seconds) from None
    finally:
        reader_done.wait(timeout=1.0)
        os.close(master_fd)
        reader.join(timeout=1.0)
    output: str = b"".join(output_parts).decode(errors="replace")
    return subprocess.CompletedProcess(
        args=("sqb", *command),
        returncode=cast(int, process.returncode),
        stdout=output,
        stderr="",
    )


_PTY_UNSET_ENVIRONMENT_VARIABLES: frozenset[str] = frozenset(
    {"COLUMNS", "LINES", "NO_COLOR", "SQLBUILD_NO_PROGRESS"}
)


def _capture_pty_output(
    *, master_fd: int, output_parts: list[bytes], reader_done: threading.Event
) -> None:
    try:
        for chunk in iter(partial(os.read, master_fd, 4096), b""):
            output_parts.append(chunk)
    except OSError:
        return
    finally:
        reader_done.set()


_TERMINAL_TOKEN_PATTERN: re.Pattern[str] = re.compile(
    r"(?P<control>\x1b\[(?P<parameters>[?0-9;]*)(?P<command>[A-Za-z]))"
    r"|(?P<carriage_return>\r)|(?P<line_feed>\n)|(?P<character>[^\x1b\r\n])"
)


class _TerminalScreen:
    """Minimal auto-wrapping terminal that understands the progress control sequences."""

    def __init__(self, *, columns: int) -> None:
        self.columns: int = columns
        self.rows: list[list[str]] = [[]]
        self.row: int = 0
        self.column: int = 0

    def carriage_return(self, match: re.Match[str]) -> None:
        del match
        self.column = 0

    def line_feed(self, match: re.Match[str]) -> None:
        del match
        self._move_to_row(self.row + 1)
        self.column = 0

    def character(self, match: re.Match[str]) -> None:
        wrap: bool = self.column >= self.columns
        self._move_to_row(self.row + int(wrap))
        self.column *= int(not wrap)
        line: list[str] = self.rows[self.row]
        line.extend(" " for _ in range(self.column + 1 - len(line)))
        line[self.column] = match.group(0)
        self.column += 1

    def control(self, match: re.Match[str]) -> None:
        handlers: dict[str, Callable[[str], None]] = {
            "A": self._cursor_up,
            "K": self._erase_line,
        }
        handlers.get(match.group("command"), self._ignore)(match.group("parameters"))

    def lines(self) -> tuple[str, ...]:
        return tuple("".join(line).rstrip() for line in self.rows)

    def _move_to_row(self, row: int) -> None:
        self.row = row
        self.rows.extend([] for _ in range(row + 1 - len(self.rows)))

    def _cursor_up(self, parameters: str) -> None:
        self.row = max(self.row - int(parameters or "1"), 0)

    def _erase_line(self, parameters: str) -> None:
        del self.rows[self.row][self.column * int(parameters != "2") :]

    def _ignore(self, parameters: str) -> None:
        del parameters


def render_terminal_screen(*, output: str, columns: int) -> tuple[str, ...]:
    """Replay raw terminal output on a minimal emulator and return the visible final lines."""

    screen: _TerminalScreen = _TerminalScreen(columns=columns)
    for match in _TERMINAL_TOKEN_PATTERN.finditer(output):
        getattr(screen, str(match.lastgroup))(match)
    return screen.lines()


def query_duckdb(*, db_path: Path, sql: str) -> list[tuple[Any, ...]]:
    """Open a DuckDB file and execute a query, returning all rows."""

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path), read_only=True)
    try:
        result: list[tuple[Any, ...]] = connection.execute(sql).fetchall()
    finally:
        connection.close()
    return result


def execute_duckdb(*, db_path: Path, sql: str) -> None:
    """Open a DuckDB file and execute a mutating statement."""

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    try:
        connection.execute(sql)
    finally:
        connection.close()


def table_exists(*, db_path: Path, table_name: str, schema: str = "main") -> bool:
    """Check if a table or view exists in the DuckDB file."""

    rows: list[tuple[Any, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            f"SELECT 1 FROM information_schema.tables "
            f"WHERE table_schema = '{schema}' AND table_name = '{table_name}'"
        ),
    )
    return len(rows) > 0


def row_count(*, db_path: Path, table_name: str, schema: str = "main") -> int:
    """Count rows in a table in the DuckDB file."""

    rows: list[tuple[Any, ...]] = query_duckdb(
        db_path=db_path,
        sql=f"SELECT COUNT(*) FROM {schema}.{table_name}",
    )
    return int(rows[0][0])


def normalize_cli_output(output: str) -> str:
    """Normalize dynamic CLI output fragments for stable assertions."""

    normalized: str = re.sub(r"\(\d+\.\d{2}s\)", "(<time>)", output)
    normalized = re.sub(
        r"PASS=\d+  WARN=\d+  FAIL=\d+  INSUFFICIENT=\d+  SKIP=\d+  TOTAL=\d+  \(<time>\)",
        "PASS=<n>  WARN=<n>  FAIL=<n>  INSUFFICIENT=<n>  SKIP=<n>  TOTAL=<n>  (<time>)",
        normalized,
    )
    normalized = re.sub(
        r"PASS=\d+  WARN=\d+  FAIL=\d+  SKIP=\d+  TOTAL=\d+  \(<time>\)",
        "PASS=<n>  WARN=<n>  FAIL=<n>  SKIP=<n>  TOTAL=<n>  (<time>)",
        normalized,
    )
    normalized = re.sub(
        r"PASS=\d+  WARN=\d+  FAIL=\d+  INSUFFICIENT=\d+  TOTAL=\d+",
        "PASS=<n>  WARN=<n>  FAIL=<n>  INSUFFICIENT=<n>  TOTAL=<n>",
        normalized,
    )
    normalized = re.sub(
        r"PASS=\d+  WARN=\d+  FAIL=\d+  TOTAL=\d+",
        "PASS=<n>  WARN=<n>  FAIL=<n>  TOTAL=<n>",
        normalized,
    )
    normalized = re.sub(
        r"PASS=\d+  FAIL=\d+  TOTAL=\d+", "PASS=<n>  FAIL=<n>  TOTAL=<n>", normalized
    )
    return normalized


def assert_fragments_in_order(output: str, fragments: tuple[str, ...]) -> None:
    """Assert that fragments appear in order within normalized output."""

    normalized_output: str = normalize_cli_output(output)
    position: int = 0
    fragment: str
    for fragment in fragments:
        index: int = normalized_output.find(fragment, position)
        assert index != -1, f"missing fragment in order: {fragment!r}\n\n{normalized_output}"
        position = index + len(fragment)


def assert_snapshot_scd2_invariants(
    *,
    db_path: Path,
    table_name: str,
    key_columns: tuple[str, ...],
    valid_from_column: str = "valid_from",
    valid_to_column: str = "valid_to",
    schema: str = "main",
) -> None:
    """Assert generic SCD2 safety invariants for a snapshot table."""

    key_sql: str = ", ".join(key_columns)
    key_match_sql: str = " AND ".join(
        f"left_row.{column} = right_row.{column}" for column in key_columns
    )
    tie_break_sql: str = " OR ".join(
        f"left_row.{column} <> right_row.{column}" for column in (*key_columns, valid_to_column)
    )
    qualified_table: str = f"{schema}.{table_name}"

    duplicate_open_rows: list[tuple[Any, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT COUNT(*) FROM ("
            f"SELECT {key_sql} FROM {qualified_table} WHERE {valid_to_column} IS NULL "
            f"GROUP BY {key_sql} HAVING COUNT(*) > 1"
            ")"
        ),
    )
    duplicate_valid_from_rows: list[tuple[Any, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT COUNT(*) FROM ("
            f"SELECT {key_sql}, {valid_from_column} FROM {qualified_table} "
            f"GROUP BY {key_sql}, {valid_from_column} HAVING COUNT(*) > 1"
            ")"
        ),
    )
    invalid_interval_rows: list[tuple[Any, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            f"SELECT COUNT(*) FROM {qualified_table} "
            f"WHERE {valid_to_column} IS NOT NULL "
            f"AND {valid_from_column} >= {valid_to_column}"
        ),
    )
    overlapping_rows: list[tuple[Any, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT COUNT(*) FROM ("
            f"SELECT 1 FROM {qualified_table} AS left_row "
            f"JOIN {qualified_table} AS right_row ON {key_match_sql} "
            f"AND ({tie_break_sql}) "
            f"AND left_row.{valid_from_column} < "
            f"COALESCE(right_row.{valid_to_column}, TIMESTAMP '9999-12-31') "
            f"AND right_row.{valid_from_column} < "
            f"COALESCE(left_row.{valid_to_column}, TIMESTAMP '9999-12-31')"
            ")"
        ),
    )

    assert duplicate_open_rows == [(0,)]
    assert duplicate_valid_from_rows == [(0,)]
    assert invalid_interval_rows == [(0,)]
    assert overlapping_rows == [(0,)]


def build_real_warehouse_snapshot_project_files(*, project_toml: str) -> dict[str, str]:
    """Build a compact real-warehouse project covering snapshot execution paths."""

    return {
        "sqlbuild_project.toml": project_toml,
        "sql/models/current_customers.sql": build_current_customers_model_sql(plan="basic"),
        "sql/models/current_customer_snapshot.sql": (
            "MODEL (description 'Test model current_customer_snapshot.',\n"
            "  materialized snapshot,\n"
            "  unique_key [customer_id, region_id],\n"
            "  snapshot_strategy timestamp,\n"
            "  updated_at updated_at,\n"
            "  valid_from_column effective_from,\n"
            "  valid_to_column effective_to,\n"
            "  audits [\n"
            "    expression_is_true (\n"
            '      name "plan_is_allowed",\n'
            "      expression \"plan <> 'blocked'\",\n"
            "    ),\n"
            "  ],\n"
            ");\n\n"
            "SELECT customer_id, region_id, plan, updated_at\n"
            'FROM __ref("current_customers")\n'
        ),
        "sql/models/historical_customer_extracts.sql": (
            "MODEL (description 'Test model historical_customer_extracts.', "
            "materialized table);\n\n"
            "SELECT 1 AS customer_id, 'basic' AS plan, "
            "CAST('2026-01-01 00:00:00' AS TIMESTAMP) AS updated_at, "
            "CAST('2026-01-02 00:00:00' AS TIMESTAMP) AS observed_at\n"
            "UNION ALL SELECT 1 AS customer_id, 'pro' AS plan, "
            "CAST('2026-01-03 00:00:00' AS TIMESTAMP) AS updated_at, "
            "CAST('2026-01-04 00:00:00' AS TIMESTAMP) AS observed_at\n"
            "UNION ALL SELECT 2 AS customer_id, 'trial' AS plan, "
            "CAST('2026-01-02 00:00:00' AS TIMESTAMP) AS updated_at, "
            "CAST('2026-01-04 00:00:00' AS TIMESTAMP) AS observed_at\n"
        ),
        "sql/models/historical_customer_snapshot.sql": (
            "MODEL (description 'Test model historical_customer_snapshot.',\n"
            "  materialized snapshot,\n"
            "  unique_key [customer_id],\n"
            "  snapshot_strategy timestamp,\n"
            "  updated_at updated_at,\n"
            "  observed_at observed_at,\n"
            "  historical_input snapshot\n"
            ");\n\n"
            "SELECT customer_id, plan, updated_at, observed_at\n"
            'FROM __ref("historical_customer_extracts")\n'
        ),
        "sql/models/historical_membership_daily.sql": (
            "MODEL (description 'Test model historical_membership_daily.', materialized table);\n\n"
            "SELECT 1 AS customer_id, 'active' AS status, "
            "CAST('2026-01-01 00:00:00' AS TIMESTAMP) AS observed_at\n"
            "UNION ALL SELECT 2 AS customer_id, 'active' AS status, "
            "CAST('2026-01-01 00:00:00' AS TIMESTAMP) AS observed_at\n"
            "UNION ALL SELECT 1 AS customer_id, 'active' AS status, "
            "CAST('2026-01-02 00:00:00' AS TIMESTAMP) AS observed_at\n"
            "UNION ALL SELECT 1 AS customer_id, 'paused' AS status, "
            "CAST('2026-01-03 00:00:00' AS TIMESTAMP) AS observed_at\n"
            "UNION ALL SELECT 2 AS customer_id, 'active' AS status, "
            "CAST('2026-01-03 00:00:00' AS TIMESTAMP) AS observed_at\n"
        ),
        "sql/models/historical_membership_snapshot.sql": (
            "MODEL (description 'Test model historical_membership_snapshot.',\n"
            "  materialized snapshot,\n"
            "  unique_key [customer_id],\n"
            "  snapshot_strategy check,\n"
            "  check_columns [status],\n"
            "  observed_at observed_at,\n"
            "  historical_input snapshot,\n"
            "  invalidate_hard_deletes true\n"
            ");\n\n"
            "SELECT customer_id, status, observed_at\n"
            'FROM __ref("historical_membership_daily")\n'
        ),
        "sql/audits/generic/expression_is_true.sql": (
            'AUDIT ();\n\nSELECT * FROM __ref("@model") WHERE NOT (@expression)\n'
        ),
    }


def build_current_customers_model_sql(*, plan: str, updated_at: str = "2026-01-01 00:00:00") -> str:
    """Build the mutable current-state model for real-warehouse snapshot tests."""

    return (
        "MODEL (description 'Test model.', materialized table);\n\n"
        "SELECT 1 AS customer_id, 10 AS region_id, "
        f"'{plan}' AS plan, CAST('{updated_at}' AS TIMESTAMP) AS updated_at\n"
    )


def build_real_warehouse_existing_snapshot_project_files(*, project_toml: str) -> dict[str, str]:
    """Build a live project that forces existing-target snapshot DML paths."""

    return {
        "sqlbuild_project.toml": project_toml,
        "sql/models/current_check_customers.sql": build_current_check_customers_model_sql(
            changed=False
        ),
        "sql/models/current_check_snapshot.sql": (
            "MODEL (description 'Test model current_check_snapshot.',\n"
            "  materialized snapshot,\n"
            "  unique_key [customer_id],\n"
            "  snapshot_strategy check,\n"
            "  check_columns [*]\n"
            ");\n\n"
            "SELECT customer_id, status\n"
            'FROM __ref("current_check_customers")\n'
        ),
        "sql/models/current_delete_customers.sql": build_current_delete_customers_model_sql(
            changed=False
        ),
        "sql/models/current_delete_snapshot.sql": (
            "MODEL (description 'Test model current_delete_snapshot.',\n"
            "  materialized snapshot,\n"
            "  unique_key [customer_id],\n"
            "  snapshot_strategy timestamp,\n"
            "  updated_at updated_at,\n"
            "  invalidate_hard_deletes true\n"
            ");\n\n"
            "SELECT customer_id, plan, updated_at\n"
            'FROM __ref("current_delete_customers")\n'
        ),
        "sql/models/historical_timestamp_extracts.sql": build_historical_timestamp_extracts_model_sql(
            changed=False
        ),
        "sql/models/historical_timestamp_snapshot.sql": (
            "MODEL (description 'Test model historical_timestamp_snapshot.',\n"
            "  materialized snapshot,\n"
            "  unique_key [customer_id],\n"
            "  snapshot_strategy timestamp,\n"
            "  updated_at updated_at,\n"
            "  observed_at observed_at,\n"
            "  historical_input snapshot,\n"
            "  invalidate_hard_deletes true\n"
            ");\n\n"
            "SELECT customer_id, plan, updated_at, observed_at\n"
            'FROM __ref("historical_timestamp_extracts")\n'
        ),
        "sql/models/historical_check_daily.sql": build_historical_check_daily_model_sql(changed=False),
        "sql/models/historical_check_snapshot.sql": (
            "MODEL (description 'Test model historical_check_snapshot.',\n"
            "  materialized snapshot,\n"
            "  unique_key [customer_id],\n"
            "  snapshot_strategy check,\n"
            "  check_columns [*],\n"
            "  observed_at observed_at,\n"
            "  historical_input snapshot,\n"
            "  invalidate_hard_deletes true\n"
            ");\n\n"
            "SELECT customer_id, status, observed_at\n"
            'FROM __ref("historical_check_daily")\n'
        ),
    }


def build_current_check_customers_model_sql(*, changed: bool) -> str:
    """Build mutable current-check source model for live snapshot apply tests."""

    return {
        False: (
            "MODEL (description 'Test model.', materialized table);\n\n"
            "SELECT 1 AS customer_id, 'active' AS status\n"
            "UNION ALL SELECT 2 AS customer_id, 'active' AS status\n"
        ),
        True: (
            "MODEL (description 'Test model.', materialized table);\n\n"
            "SELECT 1 AS customer_id, 'paused' AS status\n"
            "UNION ALL SELECT 2 AS customer_id, 'active' AS status\n"
        ),
    }[changed]


def build_current_delete_customers_model_sql(*, changed: bool) -> str:
    """Build mutable current timestamp source model for hard-delete apply tests."""

    return {
        False: (
            "MODEL (description 'Test model.', materialized table);\n\n"
            "SELECT 1 AS customer_id, 'basic' AS plan, "
            "CAST('2026-01-01 00:00:00' AS TIMESTAMP) AS updated_at\n"
            "UNION ALL SELECT 2 AS customer_id, 'trial' AS plan, "
            "CAST('2026-01-01 00:00:00' AS TIMESTAMP) AS updated_at\n"
        ),
        True: (
            "MODEL (description 'Test model.', materialized table);\n\n"
            "SELECT 1 AS customer_id, 'pro' AS plan, "
            "CAST('2026-01-03 00:00:00' AS TIMESTAMP) AS updated_at\n"
        ),
    }[changed]


def build_historical_timestamp_extracts_model_sql(*, changed: bool) -> str:
    """Build mutable historical timestamp source model for live apply tests."""

    sql: str = (
        "MODEL (description 'Test model.', materialized table);\n\n"
        "SELECT 1 AS customer_id, 'basic' AS plan, "
        "CAST('2026-01-01 00:00:00' AS TIMESTAMP) AS updated_at, "
        "CAST('2026-01-02 00:00:00' AS TIMESTAMP) AS observed_at\n"
        "UNION ALL SELECT 2 AS customer_id, 'trial' AS plan, "
        "CAST('2026-01-01 00:00:00' AS TIMESTAMP) AS updated_at, "
        "CAST('2026-01-02 00:00:00' AS TIMESTAMP) AS observed_at\n"
    )
    changed_sql: str = {
        False: "",
        True: (
            "UNION ALL SELECT 1 AS customer_id, 'pro' AS plan, "
            "CAST('2026-01-03 00:00:00' AS TIMESTAMP) AS updated_at, "
            "CAST('2026-01-04 00:00:00' AS TIMESTAMP) AS observed_at\n"
        ),
    }[changed]
    return sql + changed_sql


def build_historical_check_daily_model_sql(*, changed: bool) -> str:
    """Build mutable historical check source model for live apply tests."""

    sql: str = (
        "MODEL (description 'Test model.', materialized table);\n\n"
        "SELECT 1 AS customer_id, 'active' AS status, "
        "CAST('2026-01-01 00:00:00' AS TIMESTAMP) AS observed_at\n"
        "UNION ALL SELECT 2 AS customer_id, 'active' AS status, "
        "CAST('2026-01-01 00:00:00' AS TIMESTAMP) AS observed_at\n"
        "UNION ALL SELECT 1 AS customer_id, 'active' AS status, "
        "CAST('2026-01-02 00:00:00' AS TIMESTAMP) AS observed_at\n"
    )
    changed_sql: str = {
        False: "",
        True: (
            "UNION ALL SELECT 1 AS customer_id, 'paused' AS status, "
            "CAST('2026-01-03 00:00:00' AS TIMESTAMP) AS observed_at\n"
            "UNION ALL SELECT 2 AS customer_id, 'active' AS status, "
            "CAST('2026-01-03 00:00:00' AS TIMESTAMP) AS observed_at\n"
        ),
    }[changed]
    return sql + changed_sql


def stringify_warehouse_rows(
    rows: tuple[tuple[object, ...], ...],
) -> tuple[tuple[object, ...], ...]:
    """Normalize warehouse driver scalar differences for stable e2e assertions."""

    normalized_rows: list[tuple[object, ...]] = []
    stringifiers: dict[type, Callable[[object], object]] = {type(None): lambda _value: None}
    for row in rows:
        normalized_row: list[object] = []
        for value in row:
            normalized_row.append(stringifiers.get(type(value), str)(value))
        normalized_rows.append(tuple(normalized_row))
    return tuple(normalized_rows)
