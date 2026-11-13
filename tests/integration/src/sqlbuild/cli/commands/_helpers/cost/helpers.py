from __future__ import annotations

from collections.abc import Callable
from decimal import Decimal
from typing import Any

from sqlbuild._native import ResourceCost
from sqlbuild.adapter.contract.classes.resource_costs import ResourceCostsMixin
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.cost.models import RunCostSummary
from sqlbuild.cost.types import CostCapability, CostStatus


def scripted_summary(
    status: CostStatus, observed: int, credits: str
) -> Callable[[Decimal, str], RunCostSummary]:
    """A collection of four statements, `observed` of them seen, all on one model and warehouse."""

    def collected(usd_per_credit: Decimal, rate_source: str) -> RunCostSummary:
        amount: Decimal = Decimal(credits)
        return RunCostSummary(
            status=status,
            usd_per_credit=usd_per_credit,
            rate_source=rate_source,
            resources=(
                ResourceCost(
                    resource_type="model",
                    resource_name="orders",
                    warehouse_name="TRANSFORM_WH",
                    warehouse_size="Small",
                    warehouse_type="STANDARD",
                    cluster_number=1,
                    query_count=observed,
                    attributed_seconds=Decimal(observed * 30),
                    bytes_scanned=observed * 1024,
                    estimated_compute_credits=amount,
                    estimated_usd=amount * usd_per_credit,
                ),
            ),
            query_count=observed,
            attributed_seconds=Decimal(observed * 30),
            bytes_scanned=observed * 1024,
            estimated_compute_credits=amount,
            estimated_usd=amount * usd_per_credit,
            expected_statement_count=4,
            observed_statement_count=observed,
            missing_statement_count=4 - observed,
        )

    return collected


class ScriptedCostAdapter(ResourceCostsMixin, DuckDbAdapter):
    """DuckDB keeps the state table; each run's collections are scripted in order."""

    def __init__(
        self, collections: dict[str, list[Callable[[Decimal, str], RunCostSummary]]]
    ) -> None:
        self.collections = collections
        self.collected: list[str] = []

    def cost_capability(self) -> CostCapability:
        return CostCapability.SNOWFLAKE_QUERY_HISTORY

    def collect_run_cost(self, **kwargs: Any) -> RunCostSummary:
        run_id: str = kwargs["run_id"]
        self.collected.append(run_id)
        return self.collections[run_id].pop(0)(kwargs["usd_per_credit"], kwargs["rate_source"])
