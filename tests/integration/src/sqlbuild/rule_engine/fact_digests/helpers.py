from pathlib import Path

from sqlbuild._native import (
    build_compiled_project,
    custom_rule_fact_digest,
    discover_project_inputs,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView

PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\nextra_paths = ["config"]\n',
    "sql/sources/raw.yml": (
        "sources:\n"
        "  - name: raw_orders\n    description: Raw orders.\n"
        '    expression: "(SELECT 1 AS id, 2 AS customer_id)"\n'
    ),
    "sql/seeds/regions.csv": "region_id,name\n1,north\n",
    "sql/seeds/regions.yml": (
        "seeds:\n  - name: regions\n    description: Regions.\n    columns:\n"
        "      - name: region_id\n        type: INTEGER\n"
        "      - name: name\n        type: VARCHAR\n"
    ),
    "sql/constants/policy.sql": "CONSTANT (name minimum_amount, value 7);\n",
    "sql/enums/order_status.sql": "ENUM (name order_status, members [PAID, OPEN]);\n",
    "sql/functions/sql/is_paid.sql": (
        "FUNCTION (\n  arguments (status STRING),\n  returns BOOLEAN,\n);\n\nstatus = 'PAID'\n"
    ),
    "sql/models/staging/stg_orders.sql": (
        "MODEL (\n  columns (\n    order_id (type INTEGER, audits [not_null]),\n  ),\n);\n\n"
        'SELECT CAST(id AS INTEGER) AS order_id FROM __source("raw_orders")\n'
    ),
    "sql/models/marts/order_totals.sql": (
        'MODEL (description "Order totals");\n\nSELECT o.order_id FROM __ref("stg_orders") AS o\n'
    ),
    "tests/unit/test_order_totals.sql": (
        "TEST ();\n\nWITH\n__ref__stg_orders AS (SELECT 1 AS order_id),\n"
        "__expected__order_totals AS (SELECT 1 AS order_id)\nSELECT 1\n"
    ),
    "config/owners.yaml": "owners: [orders]\n",
}


def write_files(*, root: Path, files: dict[str, str]) -> None:
    """Write each project file below `root`."""

    for relative_path, contents in files.items():
        path: Path = root / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")


def compiled(root: Path) -> CompiledProjectView:
    """Compile the DuckDB project at `root`."""

    return build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=root), adapter=DuckDbAdapter()
    )


def fact_digest(*, root: Path, key: tuple[str, ...]) -> str | None:
    """Compile the project at `root` and digest one fact of it."""

    return custom_rule_fact_digest(project=compiled(root), project_dir=root, key=key)
