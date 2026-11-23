from __future__ import annotations

from pathlib import Path
from typing import cast

from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project


def prepare_lineage_cache_project(*, tmp_path: Path) -> Path:
    return prepare_inline_project(
        tmp_path=tmp_path,
        project_name="lineage_cache_project",
        repo_files={
            "sqlbuild_project.toml": 'name = "lineage_cache_project"\nadapter = "duckdb"\n',
            "sql/models/stg_orders.sql": (
                "MODEL (description 'Test model stg_orders.', "
                "materialized view);\n\nSELECT 1 AS order_id, 10 AS customer_id\n"
            ),
            "sql/models/stg_customers.sql": (
                "MODEL (description 'Test model stg_customers.', "
                "materialized view);\n\nSELECT 10 AS customer_id\n"
            ),
            "sql/models/fact_orders.sql": (
                'MODEL (description "Test model fact_orders.", '
                'materialized view);\n\nSELECT * FROM __ref("stg_orders")\n'
            ),
        },
    )


def lineage_node_ids(*, payload: dict[str, object]) -> tuple[str, ...]:
    nodes: list[dict[str, object]] = cast(list[dict[str, object]], payload["nodes"])
    return tuple(str(node["id"]) for node in nodes)


DIAMOND_LAYERS: int = 14
DIAMOND_EDGE_COUNT: int = 4 * DIAMOND_LAYERS


def diamond_model_names() -> tuple[str, ...]:
    """Return every model in the layered diamond project, in layer order."""

    layer_names: tuple[tuple[str, str, str], ...] = tuple(
        (f"orders_left_{layer}", f"orders_right_{layer}", f"orders_{layer}")
        for layer in range(1, DIAMOND_LAYERS + 1)
    )
    return ("orders_0", *sum(layer_names, ()))


def prepare_diamond_lineage_project(*, tmp_path: Path) -> Path:
    """Write a project whose layers each fork into two models and join again."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": 'name = "diamond_lineage"\nadapter = "duckdb"\n',
        "sql/models/orders_0.sql": "MODEL (description 'Test model orders_0.', "
        "materialized view);\n\nSELECT 1 AS order_id, 10 AS amount\n",
    }
    for layer in range(1, DIAMOND_LAYERS + 1):
        previous: str = f"orders_{layer - 1}"
        for side in ("left", "right"):
            files[f"sql/models/orders_{side}_{layer}.sql"] = (
                'MODEL (description "Test model.", '
                f'materialized view);\n\nSELECT order_id, amount FROM __ref("{previous}")\n'
            )
        files[f"sql/models/orders_{layer}.sql"] = (
            "MODEL (description 'Test model.', materialized view);\n\n"
            "SELECT l.order_id, l.amount + r.amount AS amount\n"
            f'FROM __ref("orders_left_{layer}") AS l\n'
            f'JOIN __ref("orders_right_{layer}") AS r ON l.order_id = r.order_id\n'
        )
    return prepare_inline_project(
        tmp_path=tmp_path, project_name="diamond_lineage", repo_files=files
    )
