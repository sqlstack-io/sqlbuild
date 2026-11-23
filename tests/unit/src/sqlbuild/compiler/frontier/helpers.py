"""Shared values and helpers for compiler frontier tests."""

from __future__ import annotations

from dataclasses import dataclass
from enum import StrEnum
from pathlib import PurePosixPath
from typing import Any, cast

from sqlbuild._native import BindingCatalog, CompactLineageFacts, DeclarationScopeBuild

HELPERS_MODULE: str = "tests.unit.src.sqlbuild.compiler.frontier.helpers"


class OrderStatus(StrEnum):
    """A neutral enum captured by name."""

    PLACED = "placed"


@dataclass(frozen=True)
class OrderLine:
    """A neutral dataclass captured field by field."""

    order_id: int
    status: OrderStatus
    path: PurePosixPath


def order_total(amount_cents: int) -> int:
    """A neutral callable captured by its qualified name."""

    return amount_cents


class LinkedCustomer:
    """A plain object that can point back at itself."""

    def __init__(self) -> None:
        self.name: str = "customer"
        self.peer: LinkedCustomer | None = None


def linked_customer() -> LinkedCustomer:
    """Return a customer whose peer is itself."""

    customer: LinkedCustomer = LinkedCustomer()
    customer.peer = customer
    return customer


class MemoCatalog:
    """A plain object with one memo map and one ordered map."""

    def __init__(self, *, memo: dict[str, int], ordered: dict[str, int]) -> None:
        self.memo: dict[str, int] = memo
        self.ordered: dict[str, int] = ordered


def memo_catalog(*, memo_keys: tuple[str, ...], ordered_keys: tuple[str, ...]) -> MemoCatalog:
    """Return a catalog whose maps were filled in the given key orders."""

    return MemoCatalog(
        memo={key: len(key) for key in memo_keys},
        ordered={key: len(key) for key in ordered_keys},
    )


def orders_lineage(*, string_pool: tuple[str, ...], source_index: int) -> CompactLineageFacts:
    """Return `order_id` read directly from a source, indexed into a batch-wide string pool."""

    position: dict[str, int] = {value: index for index, value in enumerate(string_pool)}
    return CompactLineageFacts(
        string_pool=string_pool,
        rows=(
            (
                position["order_id"],
                0,
                1,
                ((position["source"], source_index, position["order_id"]),),
            ),
        ),
    )


def orders_binding_catalog(*, relations: tuple[str, ...]) -> BindingCatalog:
    """Return a catalog whose relation map was filled in the given order."""

    catalog: BindingCatalog = BindingCatalog(
        dialect="duckdb",
        quoted_ignore_case=False,
        known_functions=(),
        known_types=(),
        relations={name: {"order_id": "INTEGER"} for name in relations},
    )
    return catalog


def repeated_orders() -> dict[str, object]:
    """Return equal large order lines and one long SQL text, each repeated several times."""

    query_sql: str = "SELECT " + ", ".join(f"amount_{index:03d}" for index in range(120))
    return {
        "orders": [
            {"order_id": index, "lines": _order_lines(), "query_sql": query_sql}
            for index in range(3)
        ],
    }


def _order_lines() -> list[dict[str, object]]:
    return [{"sku": f"product-{line:03d}", "quantity": line} for line in range(40)]


def order_line() -> OrderLine:
    """Return one neutral order line."""

    return OrderLine(order_id=7, status=OrderStatus.PLACED, path=PurePosixPath("sql/models/a.sql"))


def declaration_scope_build() -> DeclarationScopeBuild:
    """Return a declaration scope build whose process-local scan cache is not captured."""

    return DeclarationScopeBuild(
        loaded_macros={},
        index=cast(Any, "orders_index"),
        resolver=cast(Any, "orders_resolver"),
        sql_test_scans=cast(Any, object()),
    )
