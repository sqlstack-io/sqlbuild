from __future__ import annotations

import json
import os
import subprocess
from collections.abc import Callable
from pathlib import Path
from shutil import copytree

import pytest

from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import REPO_ROOT

DBT_INTEROP_FIXTURE_DIR: Path = REPO_ROOT / "tests" / "e2e" / "fixtures" / "dbt_interop"


def dbt_executable() -> str:
    """Return the dbt executable for e2e tests, honoring DBT_EXECUTABLE."""

    return os.environ.get("DBT_EXECUTABLE", "dbt").strip() or "dbt"


def skip_unless_dbt_is_runnable() -> None:
    """Skip e2e dbt tests when the dbt CLI is unavailable."""

    try:
        subprocess.run(
            (dbt_executable(), "--version"),
            capture_output=True,
            check=True,
            text=True,
        )
    except subprocess.CalledProcessError as error:
        pytest.skip(f"dbt CLI is not runnable: {error.stderr or error.stdout}")


def prepare_dbt_init_duckdb_workspace(*, tmp_path: Path, workspace_name: str) -> Path:
    """Write a minimal dbt project and profile for dbt init E2Es."""

    workspace: Path = tmp_path / workspace_name
    dbt_project_dir: Path = workspace / "dbt_project"
    profiles_dir: Path = workspace / "profiles"
    dbt_models_dir: Path = dbt_project_dir / "models"
    dbt_models_dir.mkdir(parents=True)
    profiles_dir.mkdir(parents=True)
    db_path: Path = workspace / "warehouse.duckdb"
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
    write_dbt_init_orders_model(workspace=workspace, amount_cents=900)
    (profiles_dir / "profiles.yml").write_text(
        "analytics:\n"
        "  target: dev\n"
        "  outputs:\n"
        "    dev:\n"
        "      type: duckdb\n"
        f"      path: '{db_path.as_posix()}'\n"
        "      schema: main\n"
        "    prod:\n"
        "      type: duckdb\n"
        f"      path: '{db_path.as_posix()}'\n"
        "      schema: prod\n",
        encoding="utf-8",
    )
    return workspace


def write_dbt_init_orders_model(*, workspace: Path, amount_cents: int) -> None:
    """Write the mutable dbt model used by dbt init E2Es."""

    workspace.joinpath("dbt_project", "models", "dbt_orders.sql").write_text(
        f"select 1 as order_id, {amount_cents} as amount_cents\n",
        encoding="utf-8",
    )


def prepare_dbt_interop_project(*, tmp_path: Path) -> Path:
    """Copy the reusable dbt interop fixture and return its SQLBuild project root."""

    root_dir: Path = tmp_path / "dbt_interop"
    copytree(DBT_INTEROP_FIXTURE_DIR, root_dir)
    local_config_path: Path = root_dir / "sqlbuild_project" / "sqlbuild_local.toml"
    local_config_path.unlink(missing_ok=True)
    db_path: Path = root_dir / "sqlbuild_project" / "dbt_interop.duckdb"
    db_path.unlink(missing_ok=True)
    profiles_path: Path = root_dir / "profiles" / "profiles.yml"
    profiles_path.write_text(
        "analytics:\n"
        "  target: dev\n"
        "  outputs:\n"
        "    dev:\n"
        "      type: duckdb\n"
        f"      path: '{db_path.as_posix()}'\n",
        encoding="utf-8",
    )
    return root_dir / "sqlbuild_project"


def write_dbt_interop_manifest(*, project_dir: Path) -> None:
    """Parse the interop fixture's dbt project so SQLBuild can resolve its dbt refs."""

    dbt_project_dir: Path = project_dir.parent / "dbt_project"
    subprocess.run(
        (
            dbt_executable(),
            "parse",
            "--project-dir",
            str(dbt_project_dir),
            "--profiles-dir",
            str(project_dir.parent / "profiles"),
        ),
        cwd=dbt_project_dir,
        capture_output=True,
        check=True,
        text=True,
    )


def write_sqlbuild_defer_target_models(*, project_dir: Path) -> None:
    """Configure native SQLBuild target deferral inside a dbt interop fixture."""

    config_path: Path = project_dir / "sqlbuild_project.toml"
    config_text: str = config_path.read_text(encoding="utf-8").replace(
        'adapter = "duckdb"\n',
        'adapter = "duckdb"\ndefault_target = "dev"\n',
    )
    config_path.write_text(
        config_text
        + "\n[targets.dev]\n"
        + 'schema = "dev"\n\n'
        + "[targets.prod]\n"
        + 'schema = "prod"\n',
        encoding="utf-8",
    )
    project_dir.joinpath("sql", "models", "deferred_upstream.sql").write_text(
        "MODEL (description 'Test model deferred_upstream.', "
        "materialized table);\n\nSELECT 42 AS order_id\n",
        encoding="utf-8",
    )
    project_dir.joinpath("sql", "models", "deferred_consumer.sql").write_text(
        'MODEL (description "Test model deferred_consumer.", '
        'materialized table);\n\nSELECT order_id FROM __ref("deferred_upstream")\n',
        encoding="utf-8",
    )


def declare_missing_origin_requiring_confirmation(*, project_dir: Path) -> None:
    """Give local_only a migrate_from origin that was never built, under require_confirmation."""

    config_path: Path = project_dir / "sqlbuild_project.toml"
    config_path.write_text(
        config_path.read_text(encoding="utf-8").replace(
            'adapter = "duckdb"\n', 'adapter = "duckdb"\ndefault_target = "main"\n'
        )
        + '\n[targets.main]\nschema = "main"\nmissing_migration_origin = "require_confirmation"\n',
        encoding="utf-8",
    )
    model_path: Path = project_dir / "sql" / "models" / "local_only.sql"
    model_path.write_text(
        model_path.read_text(encoding="utf-8").replace(
            "MODEL (\n", "MODEL (\n  materialized table,\n  migrate_from retired_local_orders,\n", 1
        ),
        encoding="utf-8",
    )


def load_json_stdout(stdout: str) -> dict[str, object]:
    """Load JSON command output."""

    payload: object = json.loads(stdout)
    assert isinstance(payload, dict)
    return payload


def break_dbt_interop_fact_orders_model(project_dir: Path) -> None:
    """Make the dbt fact_orders model fail at run time so the dbt build errors."""

    fact_orders_path: Path = (
        project_dir.parent / "dbt_project" / "models" / "marts" / "fact_orders.sql"
    )
    fact_orders_path.write_text(
        "{{ config(tags=['finance']) }}\n"
        "select * from this_relation_does_not_exist_for_failure_test\n",
        encoding="utf-8",
    )


def assert_dbt_local_replay_rows(
    *,
    project_dir: Path,
    scenario_name: str,
    rows_sql: str,
    expected_rows: tuple[tuple[object, ...], ...],
) -> None:
    """Assert replayed rows in the retained local DuckDB for a dbt scenario."""

    assertion_strategy: Callable[..., None] = {
        False: _skip_dbt_local_replay_row_assertion,
        True: _assert_dbt_local_replay_row_query,
    }[bool(rows_sql)]
    assertion_strategy(
        project_dir=project_dir,
        scenario_name=scenario_name,
        rows_sql=rows_sql,
        expected_rows=expected_rows,
    )


def _skip_dbt_local_replay_row_assertion(**_kwargs: object) -> None:
    return


def _assert_dbt_local_replay_row_query(
    *,
    project_dir: Path,
    scenario_name: str,
    rows_sql: str,
    expected_rows: tuple[tuple[object, ...], ...],
) -> None:
    from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import query_duckdb

    db_path: Path = project_dir / "target" / "run" / "scenarios" / scenario_name / "local.duckdb"
    rows: list[tuple[object, ...]] = query_duckdb(db_path=db_path, sql=rows_sql)
    assert tuple(rows) == expected_rows


def declare_sqlbuild_main_target(*, project_dir: Path, target_settings: str) -> None:
    """Make `main` the default SQLBuild target of a dbt interop fixture with extra settings."""

    config_path: Path = project_dir / "sqlbuild_project.toml"
    config_path.write_text(
        config_path.read_text(encoding="utf-8").replace(
            'adapter = "duckdb"\n', 'adapter = "duckdb"\ndefault_target = "main"\n'
        )
        + '\n[targets.main]\nschema = "main"\n'
        + target_settings,
        encoding="utf-8",
    )


def write_sqlbuild_order_status_snapshot(*, project_dir: Path) -> None:
    """Add a current-state SQLBuild snapshot whose full refresh needs confirmation."""

    config_path: Path = project_dir / "sqlbuild_project.toml"
    config_path.write_text(
        config_path.read_text(encoding="utf-8")
        + '\n[snapshots]\ncurrent_state_full_refresh = "require_confirmation"\n',
        encoding="utf-8",
    )
    project_dir.joinpath("sql", "models", "order_status_history.sql").write_text(
        "MODEL (description 'Test model order_status_history.',"
        "\n  materialized snapshot,\n  unique_key [order_id],\n"
        "  snapshot_strategy check,\n  check_columns [status],\n);\n\n"
        "SELECT 10 AS order_id, 'open' AS status\n",
        encoding="utf-8",
    )
