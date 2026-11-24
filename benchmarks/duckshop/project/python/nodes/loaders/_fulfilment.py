"""Fulfilment change logs and feeds: shipments, packages, webhooks, returns, slots and picking."""

from __future__ import annotations

import json
from dataclasses import dataclass
from datetime import date, datetime, timedelta
from functools import lru_cache

from python.nodes.loaders._core import (
    ChangeLog,
    EventLog,
    at,
    chance,
    emit,
    in_zone,
    local_naive,
    rng,
    weighted,
)
from python.nodes.loaders._world import (
    CARRIERS,
    STOREFRONTS,
    WAREHOUSE_FOR_REGION,
    WAREHOUSES,
    World,
    build_world,
)

Rows = list[dict[str, object]]
WAREHOUSE = {w.code: w for w in WAREHOUSES}
TRACKING_PREFIX = {"FLAPWING": "FW", "WINGBEAT": "WB", "POSTAL": "PS", "WEBFOOT": "WF"}


@dataclass(frozen=True)
class Shipment:
    shipment_id: str
    order_id: str
    warehouse: str
    carrier: str
    service: str | None
    tracking_number: str
    shipped_at: datetime | None
    delivered_at: datetime | None
    line_skus: tuple[tuple[str, int], ...]
    destination: str


@lru_cache(maxsize=4)
def shipment_plan(scale: int) -> tuple[Shipment, ...]:
    world = build_world(scale)
    r = rng("shipment_plan")
    plan: list[Shipment] = []
    count = 0
    for order in world.orders:
        packed = order.time_of("packed")
        if packed is None:
            continue
        shipped = order.time_of("shipped")
        delivered = order.time_of("delivered")
        region = STOREFRONTS[order.storefront].region
        warehouse = WAREHOUSE_FOR_REGION[region]
        groups = [order.lines]
        if len(order.lines) >= 4 and chance(r, 0.5):
            groups = [order.lines[:2], order.lines[2:]]
        for index, lines in enumerate(groups):
            count += 1
            carrier = str(weighted(r, {"FLAPWING": 45, "WINGBEAT": 20, "POSTAL": 30, "WEBFOOT": 5}))
            service = (
                r.choice(["standard", "Express", None, "next-day"])
                if carrier != "WEBFOOT"
                else "freight"
            )
            number = f"{TRACKING_PREFIX[carrier]}{count:08d}"
            ship_time = shipped + timedelta(hours=index * 20) if shipped else None
            if ship_time is not None and ship_time.date() > date(2026, 3, 31):
                ship_time = shipped
            plan.append(
                Shipment(
                    shipment_id=f"SH{count:06d}",
                    order_id=order.order_id,
                    warehouse=warehouse,
                    carrier=carrier,
                    service=service,
                    tracking_number=number,
                    shipped_at=ship_time,
                    delivered_at=delivered if delivered and index == 0 else None,
                    line_skus=tuple((line.sku, line.quantity) for line in lines),
                    destination=order.storefront,
                )
            )
    return tuple(plan)


def _pretty_tracking(number: str, r) -> str:
    roll = r.random()
    if roll < 0.6:
        return number
    if roll < 0.8:
        return f"{number[:2]}-{number[2:6]} {number[6:]}"
    return number.lower()


def shipments(world: World) -> Rows:
    r = rng("fulfilment__shipments")
    log = ChangeLog("fulfilment__shipments")
    for s in shipment_plan(world.scale):
        zone = WAREHOUSE[s.warehouse].timezone
        events: list[dict[str, object]] = []
        if s.shipped_at:
            events.append(
                {
                    "code": "picked-up",
                    "ts": in_zone(s.shipped_at, zone).isoformat(),
                    "location": f"{WAREHOUSE[s.warehouse].name}, "
                    f"{WAREHOUSE[s.warehouse].country.lower()}",
                }
            )
            hub = s.shipped_at + timedelta(hours=r.randint(3, 20))
            stamp = (
                in_zone(hub, zone).strftime("%d/%m/%Y %H:%M %z")
                if chance(r, 0.3)
                else in_zone(hub, "UTC").isoformat()
            )
            events.append(
                {
                    "code": "IN_TRANSIT",
                    "ts": stamp,
                    "location": r.choice(["sorting hub ,GB", "Hub 4", None]),
                }
            )
        if s.delivered_at:
            events.append(
                {
                    "code": "Delivered!",
                    "ts": in_zone(s.delivered_at, STOREFRONTS[s.destination].timezone).isoformat(),
                    "location": f"doorstep, {s.destination}",
                }
            )
        tracking: dict[str, object] | None = {
            "number": _pretty_tracking(s.tracking_number, r),
            "events": events,
        }
        row = {
            "shipment_id": s.shipment_id,
            "order_id": s.order_id,
            "warehouse_code": s.warehouse.lower() if chance(r, 0.1) else s.warehouse,
            "carrier_code": s.carrier,
            "service_level": s.service,
            "shipped_at": in_zone(s.shipped_at, zone) if s.shipped_at else None,
            "delivered_at": in_zone(s.delivered_at, STOREFRONTS[s.destination].timezone)
            if s.delivered_at
            else None,
            "tracking": tracking,
        }
        first = s.shipped_at or build_world(world.scale).order_by_id[s.order_id].placed_at
        versions = [
            (first, {**row, "delivered_at": None, "tracking": {**tracking, "events": events[:1]}})
        ]
        if s.delivered_at:
            versions.append((s.delivered_at, row))
        emit(log, r, versions)
    return log.rows()


def shipment_packages(world: World) -> Rows:
    r = rng("fulfilment__shipment_packages")
    log = ChangeLog("fulfilment__shipment_packages")
    variants = world.variant_by_sku
    count = 0
    for s in shipment_plan(world.scale):
        parts = (
            [s.line_skus]
            if len(s.line_skus) < 3 or chance(r, 0.5)
            else [s.line_skus[:1], s.line_skus[1:]]
        )
        for part in parts:
            count += 1
            grams = sum(variants[sku].weight_grams * qty for sku, qty in part) + r.randint(80, 400)
            dimensions: dict[str, object] | None = {
                "l": r.choice([20, 30, 45]),
                "w": r.choice([15, "20.5", 30]),
                "h": r.choice([10, 12.75, 25]),
            }
            if chance(r, 0.05):
                dimensions = {"l": 30, "w": 20}
            row = {
                "package_id": f"PK{count:06d}",
                "shipment_id": s.shipment_id,
                "weight_grams": grams if chance(r, 0.97) else None,
                "dimensions": dimensions,
                "contents": [{"sku": sku, "qty": qty} for sku, qty in part],
            }
            emit(log, r, [(s.shipped_at or at(date(2026, 3, 31)), row)])
    return log.rows()


def carrier_webhooks(world: World) -> Rows:
    r = rng("fulfilment__carrier_webhooks")
    log = EventLog("fulfilment__carrier_webhooks")
    count = 0
    event_id = 700000
    for s in shipment_plan(world.scale):
        if s.carrier == "POSTAL" or s.shipped_at is None:
            continue
        updates: list[tuple[str, datetime]] = [
            ("in_transit", s.shipped_at + timedelta(hours=r.randint(2, 12)))
        ]
        if s.delivered_at:
            updates.append(("out_for_delivery", s.delivered_at - timedelta(hours=r.randint(2, 6))))
            updates.append(("delivered", s.delivered_at))
        payloads: list[dict[str, object]] = []
        for status, when in updates:
            event_id += 1
            if chance(r, 0.2):
                payloads.append(
                    {
                        "trackingNumber": _pretty_tracking(s.tracking_number, r),
                        "event": status,
                        "timestamp": in_zone(when, "UTC").isoformat(),
                        "event_id": str(event_id),
                    }
                )
            else:
                payloads.append(
                    {
                        "tracking_number": _pretty_tracking(s.tracking_number, r),
                        "status": {"code": status, "at": in_zone(when, "UTC").isoformat()},
                        "event_id": event_id,
                    }
                )
        batched = s.carrier == "WINGBEAT" and len(payloads) > 1
        bodies = (
            [(json.dumps({"events": payloads}), updates[-1][1])]
            if batched
            else [(json.dumps(p), when) for p, (_, when) in zip(payloads, updates, strict=True)]
        )
        for body, when in bodies:
            count += 1
            received = when + timedelta(seconds=r.randint(5, 600))
            if chance(r, 0.02):
                body = body[: len(body) // 2]
            row = {
                "webhook_id": f"WH{count:07d}",
                "carrier_code": s.carrier.lower() if chance(r, 0.1) else s.carrier,
                "received_at": received,
                "body": body,
            }
            log.add(row, received + timedelta(minutes=1))
            if chance(r, 0.04):
                # The carrier retried the same delivery under a new webhook id.
                count += 1
                log.add(
                    {
                        **row,
                        "webhook_id": f"WH{count:07d}",
                        "received_at": received + timedelta(minutes=5),
                    },
                    received + timedelta(minutes=6),
                )
    for index, body in enumerate(["<update status='delivered'/>", "", "null", "   "], start=1):
        count += 1
        when = at(date(2026, 3, 20 + index), 12)
        log.add(
            {
                "webhook_id": f"WH{count:07d}",
                "carrier_code": "FLAPWING",
                "received_at": when,
                "body": body or None,
            },
            when,
        )
    return log.rows()


RETURN_REASONS = [
    "Arrived damaged - beak snapped",
    "wrong item: ordered TEAL got MALLARD",
    "Too big for the bath (size L)",
    "Not as described, colour is darker",
    "changed my mind",
    "Arrived 10 days late; no longer needed",
    "   other   reason   ",
]


@lru_cache(maxsize=4)
def return_plan(
    scale: int,
) -> tuple[tuple[str, str, str, int, datetime, datetime | None, str], ...]:
    world = build_world(scale)
    r = rng("return_plan")
    plan = []
    count = 0
    for order in world.orders:
        returned = order.time_of("returned")
        delivered = order.time_of("delivered")
        if returned is None and not (delivered and chance(r, 0.02)):
            continue
        requested = (
            (returned or delivered) - timedelta(days=r.randint(1, 3))
            if returned
            else delivered + timedelta(days=2)
        )  # type: ignore[operator]
        for line in order.lines[: r.randint(1, len(order.lines))]:
            count += 1
            received = requested + timedelta(days=r.randint(2, 8)) if returned else None
            if received and received.date() > date(2026, 3, 31):
                received = None
            plan.append(
                (
                    f"RT{count:05d}",
                    order.order_id,
                    line.sku,
                    line.quantity,
                    requested,
                    received,
                    WAREHOUSE_FOR_REGION[STOREFRONTS[order.storefront].region],
                )
            )
    return tuple(plan)


def returns(world: World) -> Rows:
    r = rng("fulfilment__returns")
    log = ChangeLog("fulfilment__returns")
    for return_id, order_id, sku, qty, requested, received, _ in return_plan(world.scale):
        zone = STOREFRONTS[world.order_by_id[order_id].storefront].timezone
        row = {
            "return_id": return_id,
            "order_id": order_id,
            "sku": world.variant_by_sku[sku].raw_sku,
            "qty": qty if chance(r, 0.95) else 0,
            "reason_text": r.choice(RETURN_REASONS),
            "return_status": "requested",
            "requested_at": in_zone(requested, zone),
            "received_at": None,
            "photos": r.choice([None, [], ["https://media.example.com/returns/1.jpg"]]),
            "refund_preference": r.choice(
                [None, {"method": "Store_Credit"}, {"method": "original_payment"}]
            ),
        }
        versions = [(requested, row)]
        if received:
            versions.append(
                (
                    received,
                    {
                        **row,
                        "return_status": r.choice(["received", "RECEIVED"]),
                        "received_at": in_zone(received, zone),
                    },
                )
            )
        emit(log, r, versions)
    return log.rows()


def return_inspections(world: World) -> Rows:
    r = rng("fulfilment__return_inspections")
    log = ChangeLog("fulfilment__return_inspections")
    count = 0
    for return_id, _, _, _, _, received, warehouse in return_plan(world.scale):
        if received is None:
            continue
        for attempt in range(2 if chance(r, 0.1) else 1):
            count += 1
            inspected = received + timedelta(hours=r.randint(2, 48) + attempt * 24)
            row = {
                "inspection_id": f"IN{count:05d}",
                "return_id": return_id,
                "warehouse_code": warehouse.lower() if chance(r, 0.2) else warehouse,
                "grade": r.choice(["A", " b ", "C", "scrap", "B"]),
                "notes": r.choice([None, "Scuffed paint on left wing", "As new"]),
                "inspected_at_local": local_naive(inspected, WAREHOUSE[warehouse].timezone),
                "findings": r.choice([{"defects": ["scuff", "paint"]}, {"defects": []}, None]),
            }
            emit(log, r, [(inspected, row)])
    return log.rows()


def carrier_rate_cards(world: World) -> Rows:
    r = rng("fulfilment__carrier_rate_cards")
    log = ChangeLog("fulfilment__carrier_rate_cards")
    count = 0
    for carrier in CARRIERS:
        services = ["freight"] if carrier == "WEBFOOT" else ["standard", "express", "next-day"]
        for warehouse in WAREHOUSES:
            for destination in STOREFRONTS:
                for service in services:
                    windows: list[tuple[date, date | None, float]] = [(date(2025, 1, 1), None, 1.0)]
                    if chance(r, 0.2):
                        windows = [
                            (date(2025, 1, 1), date(2026, 2, 28), 1.0),
                            (date(2026, 3, 1), None, 1.08),
                        ]
                    for start, end, uplift in windows:
                        count += 1
                        base = 4.99 if destination == warehouse.country else 14.99
                        base *= {"standard": 1.0, "express": 1.6, "next-day": 2.2, "freight": 4.0}[
                            service
                        ]
                        bands = [
                            {"max_kg": kg, "amount_minor": int(base * 100 * uplift * factor)}
                            for kg, factor in ((1, 1.0), (5, 1.5), (20, 2.4), (70.5, 5.0))
                        ]
                        row = {
                            "rate_card_id": f"RC{count:05d}",
                            "carrier_code": carrier,
                            "service_level": service.upper() if chance(r, 0.1) else service,
                            "origin_country": warehouse.country,
                            "destination_country": destination.lower()
                            if chance(r, 0.1)
                            else destination,
                            "currency_code": "GBP"
                            if warehouse.code == "POND"
                            else ("USD" if warehouse.code == "NEST" else "AUD"),
                            "valid_from": start,
                            "valid_to": end,
                            "bands": bands,
                        }
                        emit(log, r, [(at(start), row)], duplicate_share=0.0)
    return log.rows()


def delivery_slots(world: World) -> Rows:
    r = rng("fulfilment__delivery_slots")
    log = ChangeLog("fulfilment__delivery_slots")
    count = 0
    day = date(2026, 3, 9)
    while day <= date(2026, 4, 5):
        for warehouse in WAREHOUSES:
            for hour in (
                range(0, 22, 2)
                if warehouse.code == "POND" and day == date(2026, 3, 29)
                else range(8, 20, 2)
            ):
                count += 1
                start = datetime.combine(day, datetime.min.time()) + timedelta(hours=hour)
                capacity = r.choice([20, 30, 40])
                row = {
                    "slot_id": f"DS{count:06d}",
                    "warehouse_code": warehouse.code,
                    "slot_start_local": start,
                    "slot_end_local": start + timedelta(hours=2),
                    "capacity": capacity,
                    "booked": None
                    if chance(r, 0.05)
                    else min(capacity + r.choice([0, 0, 0, 3]), r.randint(0, capacity + 3)),
                    "restrictions": r.choice(
                        [None, {"excluded_postcode_prefixes": ["BT", "IV"]}, {}]
                    ),
                }
                versions = [(start - timedelta(days=14), {**row, "booked": 0})]
                if start.date() <= date(2026, 3, 31):
                    versions.append((start - timedelta(hours=2), row))
                emit(log, r, versions)
        day += timedelta(days=1)
    return log.rows()


def pick_tasks(world: World) -> Rows:
    r = rng("fulfilment__pick_tasks")
    log = ChangeLog("fulfilment__pick_tasks")
    count = 0
    for s in shipment_plan(world.scale):
        order = world.order_by_id[s.order_id]
        packed = order.time_of("packed")
        if packed is None:
            continue
        count += 1
        zone = WAREHOUSE[s.warehouse].timezone
        started = packed - timedelta(minutes=r.randint(5, 50))
        duration = timedelta(seconds=r.randint(60, 1500))
        if chance(r, 0.01):
            duration = -duration
        row = {
            "task_id": f"PT{count:06d}",
            "order_id": s.order_id,
            "warehouse_code": s.warehouse,
            "picker_id": f"picker-{r.randint(1, 12):02d}",
            "started_at_local": local_naive(started, zone),
            "completed_at_local": local_naive(started + duration, zone)
            if chance(r, 0.98)
            else None,
            "lines": [
                {"sku": sku, "qty": qty if chance(r, 0.9) else str(qty)} for sku, qty in s.line_skus
            ],
        }
        emit(
            log, r, [(started, {**row, "completed_at_local": None}), (started + abs(duration), row)]
        )
    return log.rows()


EXCEPTIONS = [
    ("ADDR_INVALID", "Address incomplete - flat number missing"),
    ("not_home", "Recipient not home, card left"),
    ("REFUSED", "Customer refused delivery"),
    ("LOST", "Parcel lost in network"),
    ("DAMAGED_IN_TRANSIT", "Box crushed; contents damaged"),
    ("DELAY_WEATHER", "Storm delay at hub"),
    ("LABEL_UNREADABLE", "Label smudged, re-labelled at depot"),
    ("CUSTOMS_HOLD", "Held at customs pending paperwork"),
]


def delivery_exceptions(world: World) -> Rows:
    r = rng("fulfilment__delivery_exceptions")
    log = ChangeLog("fulfilment__delivery_exceptions")
    count = 0
    for s in shipment_plan(world.scale):
        if s.shipped_at is None or not chance(r, 0.06):
            continue
        count += 1
        code, text = r.choice(EXCEPTIONS)
        when = s.shipped_at + timedelta(hours=r.randint(6, 60))
        if when.date() > date(2026, 3, 31):
            continue
        resolution: dict[str, object] | None = None
        versions_at = [when]
        row = {
            "exception_id": f"EX{count:05d}",
            "shipment_id": s.shipment_id,
            "exception_code": code,
            "exception_text": f"  {text} ",
            "occurred_at": in_zone(when, "UTC"),
            "resolution": resolution,
        }
        versions = [(when, row)]
        resolved = when + timedelta(hours=r.randint(4, 72))
        if resolved.date() <= date(2026, 3, 31) and chance(r, 0.7):
            versions_at.append(resolved)
            versions.append(
                (
                    resolved,
                    {
                        **row,
                        "resolution": {
                            "type": r.choice(["Redelivered", "refunded", " "]),
                            "at": in_zone(resolved, "UTC").isoformat()
                            if chance(r, 0.9)
                            else "soon",
                            "compensation_minor": r.choice([0, 500, "250", None]),
                        },
                    },
                )
            )
        emit(log, r, versions)
    return log.rows()


TABLES = {
    "fulfilment__shipments": shipments,
    "fulfilment__shipment_packages": shipment_packages,
    "fulfilment__carrier_webhooks": carrier_webhooks,
    "fulfilment__returns": returns,
    "fulfilment__return_inspections": return_inspections,
    "fulfilment__carrier_rate_cards": carrier_rate_cards,
    "fulfilment__delivery_slots": delivery_slots,
    "fulfilment__pick_tasks": pick_tasks,
    "fulfilment__delivery_exceptions": delivery_exceptions,
}
