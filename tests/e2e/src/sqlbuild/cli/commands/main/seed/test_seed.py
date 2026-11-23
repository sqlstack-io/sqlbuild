"""E2E tests for sqb seed command."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.seed._test_types import (
    SeedE2ETestCase,
    SeedJsonOutputTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    prepare_waffle_shop,
    query_duckdb,
    run_sqb,
    table_exists,
)


@pytest.mark.parametrize(
    "test_case",
    [
        SeedE2ETestCase(
            description="seed loads waffle_types CSV with correct data",
            expected_exit_code=0,
            expected_seed_name="waffle_types",
            expected_data=(
                (1, "Classic Belgian", "sweet", 850),
                (2, "Liege", "sweet", 950),
                (3, "Brussels", "sweet", 750),
                (4, "Cheddar Herb", "savory", 1050),
                (5, "Everything Bagel", "savory", 1100),
                (6, "Chicken and Waffle", "savory", 1450),
            ),
            expected_stdout_fragments=(
                "Seed ready  1 selected",
                "Seeds (1)",
                "waffle_types",
                "Execution  sqb seed  (concurrency:",
                "1/1  seed      waffle_types",
                "\u2713 Completed successfully",
                "PASS=1  WARN=0  FAIL=0  SKIP=0  TOTAL=1",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_waffle_shop_project_when_running_seed_then_seed_data_matches_expected(
    test_case: SeedE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)
    db_path: Path = project_dir / "waffle_shop.duckdb"

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "seed"), project_dir=project_dir
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert all(fragment in result.stdout for fragment in test_case.expected_stdout_fragments)
    assert table_exists(db_path=db_path, table_name=test_case.expected_seed_name)

    seed_sql: str = (
        "SELECT waffle_type_id, waffle_name, category, price_cents "
        f"FROM main.{test_case.expected_seed_name} ORDER BY waffle_type_id"
    )
    rows: list[tuple[Any, ...]] = query_duckdb(db_path=db_path, sql=seed_sql)
    assert tuple(tuple(r) for r in rows) == test_case.expected_data


@pytest.mark.parametrize(
    "test_case",
    [
        SeedE2ETestCase(
            description="empty typed seed fields load as null",
            expected_exit_code=0,
            expected_seed_name="nullable_mappings",
            expected_data=((1, "mapped"), (None, None)),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_empty_typed_seed_fields_when_loading_then_persists_nulls(
    test_case: SeedE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="nullable_seed_fields",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "nullable_seed_fields"\n'
                'adapter = "duckdb"\n'
                'default_target = "dev"\n\n'
                "[targets.dev]\n"
                'schema = "main"\n'
                "[targets.dev.connection]\n"
                'database = "warehouse.duckdb"\n'
            ),
            "sql/seeds/schema.yml": (
                "seeds:\n"
                "  - name: nullable_mappings\n    description: Test seed nullable_mappings.\n"
                "    columns:\n"
                "      - name: mapping_id\n"
                "        type: INTEGER\n"
                "      - name: mapping_name\n"
                "        type: VARCHAR\n"
            ),
            "sql/seeds/nullable_mappings.csv": "mapping_id,mapping_name\n1,mapped\n,\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "seed"), project_dir=project_dir
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    rows: list[tuple[Any, ...]] = query_duckdb(
        db_path=project_dir / "warehouse.duckdb",
        sql=f"SELECT mapping_id, mapping_name FROM main.{test_case.expected_seed_name} ORDER BY mapping_id",
    )
    assert tuple(tuple(row) for row in rows) == test_case.expected_data


@pytest.mark.parametrize(
    "test_case",
    (
        SeedJsonOutputTestCase(
            description="successful standalone seeds retain unique assets",
            expected_exit_code=0,
            expected_status="success",
            expected_summary={"success_count": 2, "failure_count": 0, "total_count": 2},
            expected_assets=(("first_seed", "success"), ("second_seed", "success")),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_standalone_seeds_when_writing_json_file_then_assets_are_complete_and_unique(
    test_case: SeedJsonOutputTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="seed_json_success",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "seed_json_success"\n'
                'adapter = "duckdb"\n'
                'default_target = "dev"\n\n'
                "[settings]\n"
                "concurrency = 2\n\n"
                "[targets.dev]\n"
                'schema = "main"\n'
                "[targets.dev.connection]\n"
                'database = "warehouse.duckdb"\n'
            ),
            "sql/seeds/schema.yml": (
                "seeds:\n"
                "  - name: first_seed\n    description: Test seed first_seed.\n"
                "    columns:\n"
                "      - name: id\n"
                "        type: INTEGER\n"
                "  - name: second_seed\n    description: Test seed second_seed.\n"
                "    columns:\n"
                "      - name: id\n"
                "        type: INTEGER\n"
            ),
            "sql/seeds/first_seed.csv": "id\n1\n",
            "sql/seeds/second_seed.csv": "id\n2\n",
        },
    )
    output_path: Path = project_dir / "seed-results.json"

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("seed", "--json-output", str(output_path)), project_dir=project_dir
    )

    payload: dict[str, object] = json.loads(output_path.read_text(encoding="utf-8"))
    assets: list[dict[str, object]] = payload["assets"]  # type: ignore[assignment]
    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert "first_seed" in result.stdout
    assert "second_seed" in result.stdout
    assert payload["status"] == test_case.expected_status
    assert payload["summary"] == test_case.expected_summary
    assert tuple(sorted((asset["name"], asset["status"]) for asset in assets)) == (
        test_case.expected_assets
    )
    assert len(assets) == len({asset["name"] for asset in assets}) == 2


@pytest.mark.parametrize(
    "test_case",
    (
        SeedJsonOutputTestCase(
            description="failed standalone seed retains honest result",
            expected_exit_code=1,
            expected_status="failed",
            expected_summary={"success_count": 0, "failure_count": 1, "total_count": 1},
            expected_assets=(("invalid_seed", "failed"),),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_failing_standalone_seed_when_writing_json_file_then_failure_is_honest(
    test_case: SeedJsonOutputTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="seed_json_failure",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "seed_json_failure"\n'
                'adapter = "duckdb"\n'
                'default_target = "dev"\n\n'
                "[targets.dev]\n"
                'schema = "main"\n'
                "[targets.dev.connection]\n"
                'database = "warehouse.duckdb"\n'
            ),
            "sql/seeds/schema.yml": (
                "seeds:\n"
                "  - name: invalid_seed\n    description: Test seed invalid_seed.\n"
                "    columns:\n"
                "      - name: id\n"
                "        type: INTEGER\n"
            ),
            "sql/seeds/invalid_seed.csv": "id\nnot-an-integer\n",
        },
    )
    output_path: Path = project_dir / "seed-results.json"

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("seed", "--json-output", str(output_path)), project_dir=project_dir
    )

    payload: dict[str, object] = json.loads(output_path.read_text(encoding="utf-8"))
    assets: list[dict[str, object]] = payload["assets"]  # type: ignore[assignment]
    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert "invalid_seed" in result.stdout
    assert payload["status"] == test_case.expected_status
    assert payload["summary"] == test_case.expected_summary
    assert tuple((asset["name"], asset["status"]) for asset in assets) == (
        test_case.expected_assets
    )
