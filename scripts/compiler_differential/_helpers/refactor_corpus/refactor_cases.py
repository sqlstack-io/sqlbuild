"""`sqb rename` and `sqb mv` cases that refactor the base project, then compile the result."""

from __future__ import annotations

from scripts.compiler_differential.constants import (
    COLD_COMPILE,
    FAILURE_BASE_FILES,
    FAILURE_MART_PATH,
    FAILURE_SOURCES_PATH,
    FAILURE_STAGING_PATH,
    GOLDEN_FULL_REPORT_LABEL_PREFIX,
)
from scripts.compiler_differential.models import (
    CorpusProject,
    DifferentialCommand,
    ExpectedOutcome,
    WritableProject,
)

_STAR_MART: str = """MODEL (
  description "Every staged order",
);

SELECT *
FROM __ref("stg_orders")
"""
_TABLE_STAGING: str = """MODEL (
  description "Staged orders",
  materialized table,
);

SELECT order_id, customer_id, amount, status
FROM __source("raw_orders")
"""
_ANCHORED_SOURCES: str = """sources:
  - name: raw_orders
    description: &feed "Feeds __ref('stg_orders')."
    expression: &orders >-
      (SELECT 1 AS order_id, 10 AS customer_id, CAST(5 AS DOUBLE) AS amount,
      'placed' AS status)
    columns: &order_columns
      - name: order_id
        type: !!str INTEGER
      - name: customer_id
        type: INTEGER
      - name: amount
        type: DOUBLE
      - name: status
        type: VARCHAR
  - name: raw_orders_copy
    description: *feed
    expression: *orders
    columns: *order_columns
"""
_HEADER_MART: str = """MODEL (
  description "Order totals per customer, from __ref('stg_orders')",
  materialized table,
);

SELECT customer_id, SUM(amount) AS total_amount
FROM __ref("stg_orders")
GROUP BY customer_id
"""


def _refactor(*arguments: str) -> DifferentialCommand:
    return DifferentialCommand(
        label=f"{GOLDEN_FULL_REPORT_LABEL_PREFIX}{arguments[0]}",
        arguments=(*arguments, "--json"),
    )


def _case(
    *,
    name: str,
    refactor: DifferentialCommand,
    files: dict[str, str] | None = None,
    refused: bool = False,
) -> CorpusProject:
    return CorpusProject(
        name=f"failure/refactor-{name}",
        commands=(refactor, COLD_COMPILE),
        expected=ExpectedOutcome(refused_commands=(refactor.label,) * refused),
        writer=WritableProject(files={**FAILURE_BASE_FILES, **(files or {})}).write,
    )


def refactor_cases() -> tuple[CorpusProject, ...]:
    """Return every refactoring case in a stable order."""

    return (
        _case(
            name="rename-model",
            refactor=_refactor("rename", "stg_orders", "stg_order_lines"),
            files={FAILURE_STAGING_PATH: _TABLE_STAGING, FAILURE_MART_PATH: _HEADER_MART},
        ),
        _case(
            name="rename-model-yaml-anchors",
            refactor=_refactor("rename", "stg_orders", "stg_order_lines"),
            files={FAILURE_SOURCES_PATH: _ANCHORED_SOURCES},
        ),
        _case(
            name="rename-model-dry-run",
            refactor=_refactor("rename", "stg_orders", "stg_order_lines", "--dry-run"),
        ),
        _case(name="mv-model", refactor=_refactor("mv", "customer_totals", "sql/models/finance/")),
        _case(
            name="rename-column",
            refactor=_refactor("rename", "stg_orders.amount", "order_amount"),
        ),
        _case(
            name="rename-column-cascade",
            refactor=_refactor("rename", "stg_orders.amount", "order_amount", "--cascade"),
            files={FAILURE_MART_PATH: _STAR_MART},
        ),
        _case(
            name="rename-column-star-refused",
            refactor=_refactor("rename", "stg_orders.amount", "order_amount"),
            files={FAILURE_MART_PATH: _STAR_MART},
            refused=True,
        ),
        _case(
            name="rename-model-collision-refused",
            refactor=_refactor("rename", "stg_orders", "customer_totals"),
            refused=True,
        ),
    )
