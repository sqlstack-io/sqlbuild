from __future__ import annotations

import subprocess
from pathlib import Path

import pytest


@pytest.fixture(scope="session")
def real_dbt_executable() -> str:
    """Return a runnable dbt executable or skip real dbt integration tests."""

    result: subprocess.CompletedProcess[str] = subprocess.run(
        ("dbt", "--version"),
        capture_output=True,
        check=False,
        text=True,
    )
    if result.returncode != 0:
        pytest.skip(f"dbt CLI is not runnable: {result.stderr or result.stdout}")
    return "dbt"


@pytest.fixture
def dbt_project_dir(tmp_path: Path) -> Path:
    """Create a tiny dbt DuckDB project."""

    project_dir: Path = tmp_path / "dbt_project"
    models_dir: Path = project_dir / "sql" / "models"
    models_dir.mkdir(parents=True)
    project_dir.joinpath("dbt_project.yml").write_text(
        "\n".join(
            (
                'name: "analytics"',
                'version: "1.0"',
                'profile: "analytics"',
                'model-paths: ["models"]',
                "models:",
                "  analytics:",
                "    +materialized: view",
            )
        ),
        encoding="utf-8",
    )
    models_dir.joinpath("stg_orders.sql").write_text(
        "{{ config(tags=['nightly']) }}\nselect 1 as order_id\n",
        encoding="utf-8",
    )
    models_dir.joinpath("fact_orders.sql").write_text(
        "select order_id from {{ ref('stg_orders') }}\n",
        encoding="utf-8",
    )
    return project_dir


@pytest.fixture
def dbt_profiles_dir(tmp_path: Path) -> Path:
    """Create a dbt profiles directory for DuckDB."""

    profiles_dir: Path = tmp_path / "profiles"
    profiles_dir.mkdir()
    profiles_dir.joinpath("profiles.yml").write_text(
        "\n".join(
            (
                "analytics:",
                "  target: dev",
                "  outputs:",
                "    dev:",
                "      type: duckdb",
                "      path: ':memory:'",
            )
        ),
        encoding="utf-8",
    )
    return profiles_dir
