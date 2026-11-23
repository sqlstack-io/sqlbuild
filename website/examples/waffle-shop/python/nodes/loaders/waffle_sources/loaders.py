from collections.abc import Iterator
from datetime import datetime

from sqlbuild.factories import factory
from sqlbuild.loaders import loader

CUSTOMERS: list[dict[str, object]] = [
    {
        "id": 1,
        "first_name": "Leslie",
        "last_name": "Knope",
        "email": "leslie@pawnee.gov",
        "created_at": datetime(2026, 1, 15, 9, 0, 0),
    },
    {
        "id": 2,
        "first_name": "Ron",
        "last_name": "Swanson",
        "email": "ron@pawnee.gov",
        "created_at": datetime(2026, 2, 1, 8, 0, 0),
    },
    {
        "id": 3,
        "first_name": "Ann",
        "last_name": "Perkins",
        "email": "ann@pawnee.gov",
        "created_at": datetime(2026, 2, 14, 10, 0, 0),
    },
    {
        "id": 4,
        "first_name": "Ben",
        "last_name": "Wyatt",
        "email": "ben@pawnee.gov",
        "created_at": datetime(2026, 3, 1, 7, 30, 0),
    },
    {
        "id": 5,
        "first_name": "April",
        "last_name": "Ludgate",
        "email": "april@pawnee.gov",
        "created_at": datetime(2026, 3, 15, 11, 0, 0),
    },
]

ORDERS: list[tuple[int, int, int, int, datetime, str]] = [
    (1, 1, 1, 2, datetime(2026, 4, 1, 9, 15, 0), "completed"),
    (2, 1, 4, 1, datetime(2026, 4, 1, 9, 15, 0), "completed"),
    (3, 2, 6, 3, datetime(2026, 4, 1, 10, 30, 0), "completed"),
    (4, 3, 2, 1, datetime(2026, 4, 2, 8, 45, 0), "completed"),
    (5, 4, 1, 1, datetime(2026, 4, 2, 12, 0, 0), "completed"),
    (6, 4, 3, 1, datetime(2026, 4, 2, 12, 0, 0), "completed"),
    (7, 5, 5, 2, datetime(2026, 4, 3, 9, 0, 0), "cancelled"),
    (8, 1, 2, 1, datetime(2026, 4, 3, 11, 0, 0), "completed"),
    (9, 2, 6, 2, datetime(2026, 4, 4, 10, 0, 0), "preparing"),
    (10, 3, 1, 4, datetime(2026, 4, 4, 14, 30, 0), "placed"),
]


@factory
def waffle_sources_loaders() -> list[object]:
    @loader
    def raw__customers(ctx: object) -> list[dict[str, object]]:
        _ = ctx
        return CUSTOMERS

    @loader
    def raw__orders(ctx: object) -> Iterator[dict[str, object]]:
        _ = ctx
        for order_id, customer_id, waffle_type_id, quantity, ordered_at, status in ORDERS:
            yield {
                "id": order_id,
                "customer_id": customer_id,
                "waffle_type_id": waffle_type_id,
                "quantity": quantity,
                "ordered_at": ordered_at,
                "status": status,
            }

    return [raw__customers, raw__orders]
