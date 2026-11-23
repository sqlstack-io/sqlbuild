"""Every generated source table by name, for the loaders and for offline checks."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any

from python.nodes.loaders import (
    _catalogue,
    _clickstream,
    _finance,
    _fulfilment,
    _growth,
    _inventory,
    _partners,
    _storefront,
    _support,
)
from python.nodes.loaders._world import World, build_world
from python.nodes.loaders._writer import statements

SCALE_VAR = "data_scale"
"""Project variable that multiplies generated volume; 1 is the small default."""

GENERATORS: dict[str, Callable[[World], list[dict[str, object]]]] = {
    **_catalogue.TABLES,
    **_clickstream.TABLES,
    **_finance.TABLES,
    **_fulfilment.TABLES,
    **_growth.TABLES,
    **_inventory.TABLES,
    **_partners.TABLES,
    **_storefront.TABLES,
    **_support.TABLES,
}


def generate(source_name: str, scale: int = 1) -> list[dict[str, object]]:
    """The rows of one source table at one scale; one code version always returns the same rows."""

    return GENERATORS[source_name](build_world(scale))


def scale_from_vars(variables: dict[str, object]) -> int:
    value = variables.get(SCALE_VAR, 1)
    scale = int(value)  # type: ignore[call-overload]
    if scale < 1:
        raise ValueError(f"{SCALE_VAR} must be a positive integer, got {value!r}")
    return scale


def load(ctx: Any, source_name: str) -> object:
    """Replace one source table with its generated rows, at the scale the project variables set."""

    scale = scale_from_vars(ctx.vars)
    rows = generate(source_name, scale)
    for sql in statements(ctx.destination, source_name, rows):
        ctx.execute_sql(sql)
    return ctx.result(metadata={"rows": len(rows), "scale": scale})
