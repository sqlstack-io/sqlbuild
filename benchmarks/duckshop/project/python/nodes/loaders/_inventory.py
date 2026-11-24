"""Inventory change logs and feeds: warehouses, stock movements, counts, purchasing, transfers."""

from __future__ import annotations

from collections import defaultdict
from datetime import date, datetime, time, timedelta
from functools import lru_cache

from python.nodes.loaders._catalogue import SUPPLIER_CURRENCY, SUPPLIER_FOR_FAMILY
from python.nodes.loaders._core import (
    ChangeLog,
    EventLog,
    at,
    chance,
    emit,
    in_zone,
    local_naive,
    messy_sku,
    rng,
)
from python.nodes.loaders._fulfilment import WAREHOUSE, return_plan, shipment_plan
from python.nodes.loaders._world import (
    CURRENCY_EXPONENT,
    STOREFRONTS,
    WAREHOUSE_FOR_REGION,
    WAREHOUSES,
    World,
    build_world,
)

Rows = list[dict[str, object]]
STOCK_START = date(2026, 1, 1)


def warehouses(world: World) -> Rows:
    r = rng("inventory__warehouses")
    log = ChangeLog("inventory__warehouses")
    details = {
        "POND": (
            time(7, 0),
            time(22, 0),
            {
                "services": ["same_day", "next_day"],
                "cold_chain": False,
                "limits": {"max_parcel_kg": 31.5},
            },
        ),
        "NEST": (
            time(6, 30),
            time(23, 30),
            {"services": ["next_day"], "limits": {"max_parcel_kg": "68.04"}},
        ),
        "MARSH": (time(8, 0), None, {"services": [], "cold_chain": "true"}),
    }
    for w in WAREHOUSES:
        opens, closes, capabilities = details[w.code]
        row = {
            "warehouse_code": w.code if w.code != "MARSH" else "marsh",
            "warehouse_name": w.name,
            "country_code": w.country,
            "iana_timezone": w.timezone,
            "opens_local": opens,
            "closes_local": closes,
            "capabilities": capabilities,
        }
        versions = [(at(date(2025, 6, 1)), row)]
        if w.code == "NEST":
            versions.append(
                (
                    at(date(2026, 2, 16), 14),
                    {
                        **row,
                        "closes_local": time(23, 59),
                        "capabilities": {**capabilities, "services": ["next_day", "same_day"]},
                    },
                )
            )
        emit(log, r, versions, duplicate_share=0.0)
    closed = {
        "warehouse_code": "REED",
        "warehouse_name": "The Reed Bed (closed)",
        "country_code": "NL",
        "iana_timezone": "Europe/Amsterdam",
        "opens_local": time(8, 0),
        "closes_local": time(17, 0),
        "capabilities": None,
    }
    emit(
        log,
        r,
        [(at(date(2025, 6, 1)), closed)],
        delete_at=at(date(2025, 12, 31)),
        duplicate_share=0.0,
    )
    return log.rows()


def _stocked(world: World) -> list[tuple[str, str]]:
    """(sku, warehouse) pairs carried in stock; The Marsh only carries a smaller range."""

    r = rng("stocked")
    pairs = []
    for variant in world.variants:
        for w in WAREHOUSES:
            if w.code == "MARSH" and not chance(r, 0.5):
                continue
            pairs.append((variant.sku, w.code))
    return pairs


@lru_cache(maxsize=4)
def movement_plan(
    scale: int,
) -> tuple[tuple[str, str, str, int, str, str, datetime, str | None], ...]:
    """(movement_id, sku, warehouse, qty_delta, reason, reference, when in UTC, operator)."""

    world = build_world(scale)
    r = rng("movement_plan")
    plan = []
    count = 0

    def add(sku: str, warehouse: str, qty: int, reason: str, reference: str, when: datetime) -> str:
        nonlocal count
        count += 1
        movement_id = f"MV{count:07d}"
        plan.append(
            (
                movement_id,
                sku,
                warehouse,
                qty,
                reason,
                reference,
                when,
                None if chance(r, 0.1) else f"op-{r.randint(1, 9)}",
            )
        )
        return movement_id

    for sku, warehouse in _stocked(world):
        # Some lines open with very little stock so they sell out for days at a time.
        opening = r.choice([3, 5, 40, 80, 150]) if chance(r, 0.85) else 0
        add(sku, warehouse, opening, "opening", "stocktake:2026-01", at(STOCK_START, 0, 30))
    for shipment in shipment_plan(scale):
        when = shipment.shipped_at or world.order_by_id[shipment.order_id].time_of("packed")
        for sku, qty in shipment.line_skus:
            add(
                sku,
                shipment.warehouse,
                -qty,
                "sale",
                f"order:{shipment.order_id}",
                when - timedelta(minutes=30),
            )  # type: ignore[operator]
    for _, order_id, sku, qty, _, received, warehouse in return_plan(scale):
        if received is not None:
            add(sku, warehouse, qty, "return", f"order:{order_id}", received + timedelta(hours=3))
    for sku, warehouse in _stocked(world):
        if chance(r, 0.15):
            when = at(STOCK_START + timedelta(days=r.randint(5, 85)), r.randint(8, 18))
            moved = add(
                sku,
                warehouse,
                r.choice([-1, -2, 1, -5]),
                r.choice(["adjust", "damage", "Damage"]),
                "",
                when,
            )
            if chance(r, 0.2):
                add(sku, warehouse, 0, "void", f"void:{moved}", when + timedelta(hours=2))
    # Movements during the spring-forward gap in London: 01:30 local does not exist on 29 March.
    for minute in (15, 45):
        add(
            world.variants[0].sku,
            "POND",
            -1,
            "damage",
            "note:dst-gap",
            datetime(2026, 3, 29, 1, minute),
        )
    return tuple(plan)


def stock_movements(world: World) -> Rows:
    r = rng("inventory__stock_movements")
    log = EventLog("inventory__stock_movements")
    for movement_id, sku, warehouse, qty, reason, reference, when, operator in movement_plan(
        world.scale
    ):
        # The DST-gap movements carry wall-clock times that London skips when clocks go forward.
        local = (
            when
            if reference == "note:dst-gap"
            else local_naive(when, WAREHOUSE[warehouse].timezone)
        )
        row = {
            "movement_id": movement_id,
            "sku": messy_sku(r, sku),
            "warehouse_code": warehouse if chance(r, 0.9) else warehouse.lower(),
            "qty_delta": qty,
            "reason_code": reason,
            "reference": reference or None,
            "moved_at_local": local,
            "metadata": {"operator": operator} if operator else None,
        }
        loaded = when + timedelta(minutes=r.randint(1, 30))
        if chance(r, 0.03):
            loaded += timedelta(days=r.randint(1, 3))
        log.add(row, loaded)
        if chance(r, 0.02):
            log.add(row, loaded + timedelta(hours=1))
    return log.rows()


def stock_counts(world: World) -> Rows:
    r = rng("inventory__stock_counts")
    log = ChangeLog("inventory__stock_counts")
    balance: dict[tuple[str, str], int] = defaultdict(int)
    for _, sku, warehouse, qty, _, _, _, _ in movement_plan(world.scale):
        balance[(sku, warehouse)] += qty
    count = 0
    for (sku, warehouse), on_hand in sorted(balance.items()):
        if not chance(r, 0.3):
            continue
        for week in range(r.randint(1, 3)):
            count += 1
            when = at(
                date(2026, 2, 2) + timedelta(days=7 * week * 3 + r.randint(0, 4)), r.randint(6, 20)
            )
            counted = max(on_hand, 0) + r.choice([0, 0, 0, -1, 2, -3])
            if chance(r, 0.02):
                counted = -1
            row = {
                "count_id": f"CT{count:06d}",
                "sku": sku,
                "warehouse_code": warehouse,
                "counted_qty": counted,
                "counted_at_local": local_naive(when, WAREHOUSE[warehouse].timezone),
                "counter": r.choice(
                    [{"id": f"op-{r.randint(1, 9)}", "blind": True}, {"id": None}, None]
                ),
            }
            emit(log, r, [(when, row)])
    return log.rows()


@lru_cache(maxsize=4)
def purchase_plan(scale: int):
    world = build_world(scale)
    r = rng("purchase_plan")
    by_supplier: dict[str, list[str]] = defaultdict(list)
    for variant in world.variants:
        by_supplier[SUPPLIER_FOR_FAMILY[variant.family][0]].append(variant.sku)
    plan = []
    count = 0
    for supplier, skus in sorted(by_supplier.items()):
        for month in (1, 2, 3):
            for warehouse in ("POND", "NEST"):
                count += 1
                ordered = at(date(2026, month, r.randint(1, 10)), r.randint(8, 17))
                expected = ordered + timedelta(days=r.randint(10, 25))
                lines = [
                    (sku, r.choice([24, 48, 100, 250]))
                    for sku in r.sample(skus, k=min(len(skus), r.randint(2, 6)))
                ]
                if chance(r, 0.15):
                    lines.append(lines[0])
                received = expected + timedelta(days=r.randint(-3, 9))
                status = "received" if received.date() <= date(2026, 3, 31) else "open"
                if chance(r, 0.05):
                    status = "cancelled"
                plan.append(
                    (
                        f"PO{count:05d}",
                        supplier,
                        warehouse,
                        ordered,
                        expected,
                        received if status == "received" else None,
                        status,
                        tuple(lines),
                    )
                )
    return tuple(plan)


def purchase_orders(world: World) -> Rows:
    r = rng("inventory__purchase_orders")
    log = ChangeLog("inventory__purchase_orders")
    variants = world.variant_by_sku
    for po_id, supplier, warehouse, ordered, expected, received, status, lines in purchase_plan(
        world.scale
    ):
        currency = SUPPLIER_CURRENCY[supplier]
        zone = WAREHOUSE[warehouse].timezone
        line_rows: list[dict[str, object]] = []
        for sku, qty in lines:
            cost = int(variants[sku].base_usd * 0.35 * 10 ** CURRENCY_EXPONENT[currency])
            line: dict[str, object] = {"sku": variants[sku].raw_sku, "qty": qty}
            if chance(r, 0.8):
                line["unit_cost_minor"] = cost
            else:
                line["unit_cost"] = cost
            if chance(r, 0.1):
                line["currency"] = currency.lower()
            line_rows.append(line)
        row = {
            "po_id": po_id,
            "supplier_code": supplier,
            "warehouse_code": warehouse,
            "currency_code": currency,
            "po_status": "open",
            "ordered_at": in_zone(ordered, zone),
            "expected_at": in_zone(expected, zone),
            "received_at": None,
            "lines": line_rows,
        }
        versions = [(ordered, row)]
        if status == "received":
            versions.append(
                (
                    received,
                    {
                        **row,
                        "po_status": r.choice(["Received", "received"]),
                        "received_at": in_zone(received, zone),
                    },
                )
            )
        elif status == "cancelled":
            versions.append((ordered + timedelta(days=2), {**row, "po_status": "CANCELLED"}))
        emit(log, r, versions)
    return log.rows()


def po_receipts(world: World) -> Rows:
    r = rng("inventory__po_receipts")
    log = ChangeLog("inventory__po_receipts")
    count = 0
    for po_id, _, warehouse, _, _, received, status, lines in purchase_plan(world.scale):
        if status != "received" or received is None:
            continue
        seen: set[str] = set()
        for sku, qty in lines:
            if sku in seen:
                continue
            seen.add(sku)
            for part in range(2 if chance(r, 0.1) else 1):
                count += 1
                when = received + timedelta(hours=part * 30)
                damaged = r.choice([None, 0, 0, 1, 3])
                short = chance(r, 0.1)
                row = {
                    "receipt_id": f"RC{count:06d}",
                    "po_id": po_id,
                    "sku": messy_sku(r, sku),
                    "warehouse_code": warehouse,
                    "received_qty": (qty - r.randint(1, 5) if short else qty) // (2 if part else 1),
                    "damaged_qty": damaged,
                    "received_at_local": local_naive(when, WAREHOUSE[warehouse].timezone),
                    "discrepancies": {"codes": ["short"]}
                    if short
                    else r.choice([None, {"codes": []}]),
                }
                emit(log, r, [(when, row)])
    return log.rows()


def stock_transfers(world: World) -> Rows:
    r = rng("inventory__stock_transfers")
    log = ChangeLog("inventory__stock_transfers")
    for i in range(1, 40 * world.scale + 1):
        variant = r.choice(world.variants)
        source, target = r.sample(["POND", "NEST", "MARSH"], k=2)
        shipped = at(date(2026, 1, 10) + timedelta(days=r.randint(0, 80)), r.randint(6, 18))
        arrived = shipped + timedelta(days=r.randint(3, 12), hours=r.randint(0, 23))
        planned = chance(r, 0.1)
        row = {
            "transfer_id": f"TF{i:04d}",
            "sku": variant.raw_sku,
            "from_warehouse_code": source,
            "to_warehouse_code": target.lower() if chance(r, 0.1) else target,
            "qty": r.choice([12, 24, 48]),
            "shipped_at": None,
            "arrived_at": None,
            "tracking": None,
        }
        versions = [(shipped - timedelta(days=2), row)]
        if not planned:
            versions.append(
                (
                    shipped,
                    {
                        **row,
                        "shipped_at": in_zone(shipped, WAREHOUSE[source].timezone),
                        "tracking": {
                            "reference": r.choice(
                                [f"HL{r.randint(100000, 9999999)}", "pending", None]
                            )
                        },
                    },
                )
            )
            if arrived.date() <= date(2026, 3, 31):
                versions.append(
                    (
                        arrived,
                        {
                            **versions[-1][1],
                            "arrived_at": in_zone(arrived, WAREHOUSE[target].timezone),
                        },
                    )
                )
        emit(log, r, versions)
    return log.rows()


def stock_reservations(world: World) -> Rows:
    r = rng("inventory__stock_reservations")
    log = ChangeLog("inventory__stock_reservations")
    count = 0
    for order in world.orders:
        if order.placed_at.date() < date(2026, 2, 1):
            continue
        warehouse = WAREHOUSE_FOR_REGION[STOREFRONTS[order.storefront].region]
        released = order.time_of("shipped") or order.time_of("cancelled")
        for line in order.lines:
            count += 1
            row = {
                "reservation_id": f"RS{count:07d}",
                "order_id": order.order_id,
                "sku": world.variant_by_sku[line.sku].raw_sku,
                "warehouse_code": warehouse,
                "qty": line.quantity,
                "reserved_at": in_zone(order.placed_at, "UTC"),
                "released_at": None,
                "allocation": r.choice(
                    [
                        {"rule": "FIFO", "backorder": False},
                        {"rule": "nearest", "backorder": "true"},
                        None,
                    ]
                ),
            }
            versions = [(order.placed_at, row)]
            if released is not None:
                versions.append((released, {**row, "released_at": in_zone(released, "UTC")}))
            emit(log, r, versions)
    return log.rows()


def bin_locations(world: World) -> Rows:
    r = rng("inventory__bin_locations")
    log = ChangeLog("inventory__bin_locations")
    count = 0
    for w in WAREHOUSES:
        for aisle in ("A", "B", "c"):
            for bay in range(1, 6):
                for level in (1, 2):
                    count += 1
                    row = {
                        "bin_id": f"BIN{count:05d}",
                        "warehouse_code": w.code.lower() if chance(r, 0.1) else w.code,
                        "bin_label": f"{aisle}-{bay:02d}-{level}"
                        if chance(r, 0.95)
                        else f"{aisle}{bay}",
                        "attributes": r.choice(
                            [
                                {"pick_face": level == 1, "max_weight_kg": 25.5},
                                {"temperature": "chilled"},
                                {"max_weight_kg": "40"},
                                None,
                            ]
                        ),
                    }
                    delete_at = at(date(2026, 3, 1)) if chance(r, 0.03) else None
                    emit(
                        log,
                        r,
                        [(at(date(2025, 6, 1)), row)],
                        delete_at=delete_at,
                        duplicate_share=0.0,
                    )
    return log.rows()


TABLES = {
    "inventory__warehouses": warehouses,
    "inventory__stock_movements": stock_movements,
    "inventory__stock_counts": stock_counts,
    "inventory__purchase_orders": purchase_orders,
    "inventory__po_receipts": po_receipts,
    "inventory__stock_transfers": stock_transfers,
    "inventory__stock_reservations": stock_reservations,
    "inventory__bin_locations": bin_locations,
}
