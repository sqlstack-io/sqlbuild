"""Helpers for native project graph integration tests."""

from __future__ import annotations

from pathlib import Path

ORDERS_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
    "sql/models/staging/stg_orders.sql": (
        "MODEL (description 'Staged orders.', tags [daily]);\nSELECT 1 AS order_id\n"
    ),
    "sql/models/marts/orders.sql": (
        "MODEL (description 'Orders.');\nSELECT order_id FROM __ref(\"stg_orders\")\n"
    ),
    "sql/models/marts/customers.sql": "MODEL (description 'Customers.');\nSELECT 1 AS customer_id\n",
}
MODEL_NAMES: frozenset[str] = frozenset({"stg_orders", "orders", "customers"})


def write_files(*, project_dir: Path, files: dict[str, str]) -> None:
    """Write project files under `project_dir`."""

    for relative_path, text in files.items():
        path: Path = project_dir / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")


def compiled_models(*, stdout: str) -> dict[str, bool]:
    """Whether the compile report lists each project model."""

    return {name: f"── {name} " in stdout for name in sorted(MODEL_NAMES)}
