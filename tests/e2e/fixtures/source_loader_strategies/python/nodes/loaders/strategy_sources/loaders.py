from datetime import datetime
from typing import Any

from sqlbuild.factories import factory
from sqlbuild.loaders import loader


@factory
def strategy_sources_loaders() -> list[object]:
    @loader(write_strategy="replace")
    def raw_countries(ctx: object) -> list[dict[str, object]]:
        """Test loader raw_countries."""
        _ = ctx
        return [
            {"country_id": 1, "country_code": "US", "country_name": "United States"},
            {"country_id": 2, "country_code": "CA", "country_name": "Canada"},
        ]

    @loader(write_strategy="append")
    def raw_webhook_events(ctx: object) -> list[dict[str, object]]:
        """Test loader raw_webhook_events."""
        _ = ctx
        return [
            {"event_id": 101, "event_name": "signup"},
            {"event_id": 102, "event_name": "checkout"},
        ]

    @loader(write_strategy="delete_insert", cursor_column="event_at")
    def raw_order_events(ctx: Any) -> list[dict[str, object]]:
        """Test loader raw_order_events."""
        if ctx.cursor_value is None:
            return [
                {
                    "event_id": 201,
                    "event_at": datetime(2026, 5, 1, 0, 0, 0),
                    "amount_cents": 1000,
                },
                {
                    "event_id": 202,
                    "event_at": datetime(2026, 5, 1, 1, 0, 0),
                    "amount_cents": 2000,
                },
            ]
        return [
            {
                "event_id": 202,
                "event_at": datetime(2026, 5, 1, 1, 0, 0),
                "amount_cents": 2500,
            },
            {
                "event_id": 203,
                "event_at": datetime(2026, 5, 1, 2, 0, 0),
                "amount_cents": 3000,
            },
        ]

    @loader(write_strategy="merge", unique_key="customer_id", cursor_column="updated_at")
    def raw_customers(ctx: Any) -> list[dict[str, object]]:
        """Test loader raw_customers."""
        if ctx.cursor_value is None:
            return [
                {
                    "customer_id": 1,
                    "plan_name": "basic",
                    "updated_at": datetime(2026, 5, 1, 0, 0, 0),
                },
                {
                    "customer_id": 2,
                    "plan_name": "trial",
                    "updated_at": datetime(2026, 5, 1, 0, 0, 0),
                },
            ]
        return [
            {
                "customer_id": 1,
                "plan_name": "pro",
                "updated_at": datetime(2026, 5, 2, 0, 0, 0),
            },
            {
                "customer_id": 3,
                "plan_name": "enterprise",
                "updated_at": datetime(2026, 5, 2, 0, 0, 0),
            },
        ]

    @loader
    def raw_loader_status(ctx: Any) -> None:
        """Test loader raw_loader_status."""
        ctx.execute_sql(f"DROP TABLE IF EXISTS {ctx.destination}")
        ctx.execute_sql(
            f"CREATE TABLE {ctx.destination} AS "
            "SELECT 1 AS status_id, 'loaded' AS status_name, 'self_managed' AS loaded_by"
        )

    return [raw_countries, raw_webhook_events, raw_order_events, raw_customers, raw_loader_status]
