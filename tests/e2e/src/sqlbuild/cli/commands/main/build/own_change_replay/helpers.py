"""Project builders for own-change replay e2e tests."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)

DATABASE_FILE: str = "warehouse.duckdb"
FIRST_DAY: str = "2026-09-01"
_PROJECT_TOML: str = (
    'name = "own_change_replay"\nadapter = "duckdb"\n\n'
    f'[connection]\ndatabase = "{DATABASE_FILE}"\n'
)
_EVENTS_SEED_YML: str = (
    "seeds:\n"
    "- name: events\n  description: Test seed events.\n"
    "  columns:\n"
    "  - name: id\n"
    "    type: INTEGER\n"
    "  - name: event_date\n"
    "    type: DATE\n"
)
_EVENTS_CSV: str = "id,event_date\n1,2026-09-01\n2,2026-09-02\n3,2026-09-03\n"
_UPSTREAM_SQL: str = (
    "MODEL (description 'Test model.',\n  materialized table,\n);\n\n"
    "SELECT\n  id,\n  CAST(event_date AS DATE) AS event_date\n"
    'FROM __seed("events")\n'
)
_DOWNSTREAM_MB_SQL: str = (
    "MODEL (description 'Test model.',\n"
    "  materialized incremental,\n"
    "  incremental_strategy delete_insert,\n"
    "  cursor event_date,\n"
    "  cursor_type timestamp,\n"
    "  cursor_grain day,\n"
    "  cursor_inputs (\n"
    "    upstream_v (\n"
    "      column event_date,\n"
    "      roles [filter, watermark],\n"
    "    ),\n"
    "  ),\n"
    "  incremental_mode microbatch,\n"
    "  microbatch_strategy watermark,\n"
    "  batch_size 1d,\n"
    "  cursor_watermark_mode all,\n"
    "  lookback 1d,\n"
    "  full_refresh false,\n"
    '  cursor_start "2026-09-01",\n'
    ");\n\n"
    'SELECT\n  id,\n  event_date\nFROM __ref("upstream_v")\n'
)
_DOWNSTREAM_MERGE_SQL: str = (
    "MODEL (description 'Test model.',\n"
    "  materialized incremental,\n"
    "  incremental_strategy merge,\n"
    "  unique_key [id],\n"
    "  cursor event_date,\n"
    "  cursor_type timestamp,\n"
    "  cursor_grain day,\n"
    "  lookback 1d,\n"
    "  cursor_inputs (\n"
    "    upstream_v event_date,\n"
    "  ),\n"
    "{extra_config}"
    ");\n\n"
    'SELECT\n  id,\n  event_date\nFROM __ref("upstream_v")\n'
    "WHERE\n  event_date >= __cursor_start()::DATE\n"
)
_ADJUST_FUNCTION_PATH: str = "sql/functions/sql/udf__adjust_amount.sql"


def _upstream_view_sql(*, upstream: str) -> str:
    return (
        "MODEL (description 'Test model udf__adjust_amount.',\n  materialized view,\n);\n\n"
        f'SELECT\n  id,\n  event_date\nFROM __ref("{upstream}")\n'
    )


def build_event_project(*, tmp_path: Path, extra_files: dict[str, str]) -> Path:
    """Write and build the event chain of table, view and microbatch plus any extra files."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": _PROJECT_TOML,
        "sql/seeds/seeds.yml": _EVENTS_SEED_YML,
        "sql/seeds/events.csv": _EVENTS_CSV,
        "sql/models/upstream.sql": _UPSTREAM_SQL,
        "sql/models/upstream_v.sql": _upstream_view_sql(upstream="upstream"),
        "sql/models/downstream_mb.sql": _DOWNSTREAM_MB_SQL,
        **extra_files,
    }
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="own_change_replay", repo_files=files
    )
    _build_ok(project_dir=project_dir)
    return project_dir


def merge_downstream_files(*, extra_config: str) -> dict[str, str]:
    """Return a merge model whose SQL reads its interval through cursor intrinsics."""

    return {"sql/models/downstream.sql": _DOWNSTREAM_MERGE_SQL.format(extra_config=extra_config)}


def mark_first_day(*, project_dir: Path, relations: tuple[str, ...]) -> None:
    """Offset ids of the first, out-of-window day so a replay of that day is detectable."""

    relation: str
    for relation in relations:
        execute_duckdb(
            db_path=project_dir / DATABASE_FILE,
            sql=f"UPDATE main.{relation} SET id = id + 100 WHERE event_date = DATE '{FIRST_DAY}'",
        )


def insert_table_between_upstream_and_view(*, project_dir: Path) -> None:
    """Add a new table between the upstream table and its view."""

    (project_dir / "sql/models/upstream_new.sql").write_text(
        "MODEL (description 'Test model upstream_new.',\n  materialized table,\n);\n\n"
        'SELECT\n  id,\n  event_date\nFROM __ref("upstream")\n',
        encoding="utf-8",
    )
    _write_upstream_view(project_dir=project_dir, upstream="upstream_new")


def rename_upstream_with_migrate_from(*, project_dir: Path, extra_sql: str) -> None:
    """Rename the upstream table by hand, declaring migrate_from, optionally changing its SQL."""

    (project_dir / "sql/models/upstream.sql").unlink()
    (project_dir / "sql/models/upstream_renamed.sql").write_text(
        _UPSTREAM_SQL.replace(
            "  materialized table,\n", "  materialized table,\n  migrate_from upstream,\n"
        )
        + extra_sql,
        encoding="utf-8",
    )
    _write_upstream_view(project_dir=project_dir, upstream="upstream_renamed")


def rename_upstream_with_cli(*, project_dir: Path, extra_sql: str) -> None:
    """Rename the upstream table with sqb rename, then append SQL to the renamed model."""

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "rename", "model:upstream", "upstream_renamed"),
        project_dir=project_dir,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    renamed_path: Path = project_dir / "sql/models/upstream_renamed.sql"
    renamed_path.write_text(renamed_path.read_text(encoding="utf-8") + extra_sql, encoding="utf-8")


def _write_upstream_view(*, project_dir: Path, upstream: str) -> None:
    (project_dir / "sql/models/upstream_v.sql").write_text(
        _upstream_view_sql(upstream=upstream), encoding="utf-8"
    )


def build_function_project(*, tmp_path: Path, caller_config: str) -> Path:
    """Write and build a UDF called by an incremental model that feeds another incremental."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": _PROJECT_TOML,
        "sql/seeds/seeds.yml": (
            "seeds:\n"
            "- name: orders\n  description: Test seed orders.\n"
            "  columns:\n"
            "  - name: id\n"
            "    type: INTEGER\n"
            "  - name: ordered_at\n"
            "    type: DATE\n"
            "  - name: amount\n"
            "    type: INTEGER\n"
        ),
        "sql/seeds/orders.csv": (
            "id,ordered_at,amount\n1,2026-09-01,10\n2,2026-09-02,20\n3,2026-09-03,30\n"
        ),
        _ADJUST_FUNCTION_PATH: (
            "FUNCTION (description 'Test function.',"
            "\n  arguments (amount INTEGER),\n  returns INTEGER,\n);\n\namount + 1\n"
        ),
        "sql/models/order_amounts.sql": (
            "MODEL (description 'Test model order_amounts.',\n"
            "  materialized incremental,\n"
            "  incremental_strategy merge,\n"
            "  unique_key [id],\n"
            "  cursor ordered_at,\n"
            "  cursor_type timestamp,\n"
            "  cursor_grain day,\n"
            "  cursor_inputs (\n    orders ordered_at,\n  ),\n"
            f"{caller_config}"
            ");\n\n"
            "SELECT\n  id,\n  CAST(ordered_at AS DATE) AS ordered_at,\n"
            '  __udf("udf__adjust_amount")(amount) AS adjusted_amount\n'
            'FROM __seed("orders")\n'
        ),
        "sql/models/order_totals.sql": (
            "MODEL (description 'Test model order_totals.',\n"
            "  materialized incremental,\n"
            "  incremental_strategy merge,\n"
            "  unique_key [id],\n"
            "  cursor ordered_at,\n"
            "  cursor_type timestamp,\n"
            "  cursor_grain day,\n"
            "  cursor_inputs (\n    order_amounts ordered_at,\n  ),\n"
            ");\n\n"
            "SELECT\n  id,\n  ordered_at,\n  adjusted_amount * 2 AS doubled_amount\n"
            'FROM __ref("order_amounts")\n'
        ),
    }
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="function_caller_replay", repo_files=files
    )
    _build_ok(project_dir=project_dir)
    (project_dir / _ADJUST_FUNCTION_PATH).write_text(
        (project_dir / _ADJUST_FUNCTION_PATH)
        .read_text(encoding="utf-8")
        .replace("amount + 1", "amount + 100"),
        encoding="utf-8",
    )
    return project_dir


def direct_reference_merge_files() -> dict[str, str]:
    """Return a protected full-replay merge model that reads the upstream table directly."""

    return {
        "sql/models/down_merge.sql": (
            "MODEL (description 'Test model down_merge.',\n"
            "  materialized incremental,\n"
            "  incremental_strategy merge,\n"
            "  unique_key [id],\n"
            "  cursor event_date,\n"
            "  cursor_type timestamp,\n"
            "  cursor_grain day,\n"
            "  cursor_inputs (\n    upstream event_date,\n  ),\n"
            "  replay_on_change full,\n"
            "  full_refresh false,\n"
            ");\n\n"
            'SELECT\n  id,\n  event_date\nFROM __ref("upstream")\n'
        )
    }


def edit_direct_reference_merge(*, project_dir: Path) -> None:
    """Make a real query edit to the direct-reference merge model."""

    path: Path = project_dir / "sql/models/down_merge.sql"
    path.write_text(
        path.read_text(encoding="utf-8").replace(
            "  event_date\nFROM", "  event_date,\n  id * 2 AS doubled_id\nFROM"
        ),
        encoding="utf-8",
    )


def build_star_project(*, tmp_path: Path, downstream_config: str) -> Path:
    """Write and build a table and an append incremental that selects all of its columns."""

    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="star_projection",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_TOML,
            "sql/models/up.sql": (
                "MODEL (description 'Test model up.',\n  materialized table,\n);\n\n"
                "SELECT 1 AS id, DATE '2026-09-01' AS event_date\n"
            ),
            "sql/models/down.sql": (
                "MODEL (description 'Test model down.',\n"
                "  materialized incremental,\n"
                "  incremental_strategy append,\n"
                "  replay_on_change full,\n"
                f"{downstream_config}"
                ");\n\n"
                'SELECT * FROM __ref("up")\n'
            ),
        },
    )
    _build_ok(project_dir=project_dir)
    return project_dir


def add_upstream_column(*, project_dir: Path) -> None:
    """Add a column to the star project's upstream table."""

    path: Path = project_dir / "sql/models/up.sql"
    path.write_text(
        path.read_text(encoding="utf-8").replace(
            "AS event_date", "AS event_date, 'web' AS channel"
        ),
        encoding="utf-8",
    )


ENFORCED_CONTRACT_CONFIG: str = (
    "  contract enforced,\n  columns (\n    id (type INTEGER),\n    event_date (type DATE),\n  ),\n"
)


def declare_downstream_column_type(*, project_dir: Path) -> None:
    """Change the declared type of the star downstream's id column in its own contract."""

    path: Path = project_dir / "sql/models/down.sql"
    path.write_text(
        path.read_text(encoding="utf-8").replace("id (type INTEGER)", "id (type BIGINT)"),
        encoding="utf-8",
    )


def remove_downstream_declared_column(*, project_dir: Path) -> None:
    """Remove event_date from the star downstream's own contract and select list."""

    path: Path = project_dir / "sql/models/down.sql"
    path.write_text(
        path.read_text(encoding="utf-8")
        .replace("    event_date (type DATE),\n", "")
        .replace("SELECT *", "SELECT id"),
        encoding="utf-8",
    )


def udf_reference_merge_files() -> dict[str, str]:
    """Return a protected full-replay merge model that reads the upstream through a UDF."""

    return {
        "sql/functions/sql/udf__plus_one.sql": (
            "FUNCTION (description 'Test function udf__plus_one.',"
            "\n  arguments (value INTEGER),\n  returns INTEGER,\n);\n\nvalue + 1\n"
        ),
        "sql/models/down_merge.sql": (
            "MODEL (description 'Test model down_merge.',\n"
            "  materialized incremental,\n"
            "  incremental_strategy merge,\n"
            "  unique_key [id],\n"
            "  cursor event_date,\n"
            "  cursor_type timestamp,\n"
            "  cursor_grain day,\n"
            "  cursor_inputs (\n    upstream event_date,\n  ),\n"
            "  replay_on_change full,\n"
            "  full_refresh false,\n"
            ");\n\n"
            '-- reads __ref("upstream")\n'
            "SELECT\n  u.id,\n  u.event_date,\n"
            '  __udf("udf__plus_one")(u.id) AS next_id\n'
            'FROM __ref("upstream") AS u\n'
        ),
    }


def incremental_upstream_files() -> dict[str, str]:
    """Return an incremental upstream so a rename moves its data instead of renaming it."""

    return {
        "sql/models/upstream.sql": (
            "MODEL (description 'Test model upstream.',\n"
            "  materialized incremental,\n"
            "  incremental_strategy merge,\n"
            "  unique_key [id],\n"
            "  cursor event_date,\n"
            "  cursor_type timestamp,\n"
            "  cursor_grain day,\n"
            "  cursor_inputs (\n    events event_date,\n  ),\n"
            ");\n\n"
            "SELECT\n  id,\n  CAST(event_date AS DATE) AS event_date\n"
            'FROM __seed("events")\n'
        ),
    }


def build_selected(*, project_dir: Path, selector: str) -> None:
    """Build only the selected nodes."""

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--select", selector), project_dir=project_dir
    )
    assert result.returncode == 0, result.stdout + result.stderr


def remove_migrate_from(*, project_dir: Path) -> None:
    """Drop the migrate_from declaration from the renamed upstream model."""

    path: Path = project_dir / "sql/models/upstream_renamed.sql"
    path.write_text(
        path.read_text(encoding="utf-8").replace("  migrate_from upstream,\n", ""),
        encoding="utf-8",
    )


def drop_upstream_column(*, project_dir: Path) -> None:
    """Drop the event_date column from the star project's upstream table."""

    (project_dir / "sql/models/up.sql").write_text(
        "MODEL (description 'Test model up.',\n  materialized table,\n);\n\nSELECT 1 AS id\n",
        encoding="utf-8",
    )


def plan(*, project_dir: Path, args: tuple[str, ...] = ()) -> subprocess.CompletedProcess[str]:
    """Run sqb plan without colour."""

    return run_sqb(command=("--no-color", "plan", *args), project_dir=project_dir)


def plan_models(*, project_dir: Path) -> dict[str, dict[str, Any]]:
    """Return plan JSON model entries keyed by model name."""

    result: subprocess.CompletedProcess[str] = plan(project_dir=project_dir, args=("--json",))
    assert result.returncode == 0, result.stdout + result.stderr
    return {str(model["name"]): model for model in json.loads(result.stdout)["models"]}


def build(*, project_dir: Path) -> subprocess.CompletedProcess[str]:
    """Run sqb build without colour."""

    return run_sqb(command=("--no-color", "build"), project_dir=project_dir)


def _build_ok(*, project_dir: Path) -> None:
    result: subprocess.CompletedProcess[str] = build(project_dir=project_dir)
    assert result.returncode == 0, result.stdout + result.stderr


def relation_rows(*, project_dir: Path, relation: str) -> list[tuple[Any, ...]]:
    """Return every row of a relation ordered by its first column."""

    return query_duckdb(
        db_path=project_dir / DATABASE_FILE, sql=f"SELECT * FROM main.{relation} ORDER BY 1"
    )


def relation_types(*, project_dir: Path, name: str) -> list[str]:
    """Return the information-schema table types of a main-schema relation name."""

    rows: list[tuple[Any, ...]] = query_duckdb(
        db_path=project_dir / DATABASE_FILE,
        sql=(
            "SELECT table_type FROM information_schema.tables "
            f"WHERE table_schema = 'main' AND table_name = '{name}'"
        ),
    )
    return [str(row[0]) for row in rows]


def build_and_assert_rows(
    *, project_dir: Path, expected_rows: dict[str, list[tuple[Any, ...]]]
) -> None:
    """Build the project and compare every listed relation's rows."""

    result: subprocess.CompletedProcess[str] = build(project_dir=project_dir)
    assert result.returncode == 0, result.stdout + result.stderr
    assert {
        relation: relation_rows(project_dir=project_dir, relation=relation)
        for relation in expected_rows
    } == expected_rows


def assert_plan_text(
    *, project_dir: Path, expected: tuple[str, ...], unexpected: tuple[str, ...]
) -> None:
    """Plan the project and check the text output for expected and unexpected fragments."""

    result: subprocess.CompletedProcess[str] = plan(project_dir=project_dir)
    assert result.returncode == 0, result.stdout + result.stderr
    assert all(fragment in result.stdout for fragment in expected), result.stdout
    assert not any(fragment in result.stdout for fragment in unexpected), result.stdout


def assert_plan_fails(
    *, project_dir: Path, args: tuple[str, ...], expected: tuple[str, ...]
) -> None:
    """Plan the project, expect failure, and check the output for expected fragments."""

    result: subprocess.CompletedProcess[str] = plan(project_dir=project_dir, args=args)
    output: str = result.stdout + result.stderr
    assert result.returncode != 0, output
    assert all(fragment in output for fragment in expected), output
