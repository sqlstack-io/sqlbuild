"""Project helpers for header string quoting and folding E2E tests."""

from __future__ import annotations

from pathlib import Path

from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project

_BASE_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "shop"\nadapter = "duckdb"\n\n[connection]\ndatabase = "shop.duckdb"\n\n'
        "[scopes]\nenforce_placement = false\n"
    ),
    "sql/seeds/raw_customers.csv": "customer_id,title\n1,Miss\n2,Mr\n",
    "sql/seeds/raw_customers.yml": (
        "seeds:\n  - name: raw_customers\n    description: Raw customers.\n"
        "    columns:\n      - name: customer_id\n        type: BIGINT\n"
        "      - name: title\n        type: VARCHAR\n"
    ),
}


def header_strings_project(tmp_path: Path, files: dict[str, str]) -> Path:
    """Write a DuckDB customer project holding `files` and return its directory."""

    return prepare_inline_project(
        tmp_path=tmp_path, project_name="shop", repo_files={**_BASE_FILES, **files}
    )
