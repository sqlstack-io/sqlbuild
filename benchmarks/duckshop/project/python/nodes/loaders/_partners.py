"""Marketplace partner feeds: listings, order messages in two payload versions, and settlements."""

from __future__ import annotations

import hashlib
from datetime import date, timedelta

from python.nodes.loaders._core import ChangeLog, EventLog, at, chance, emit, in_zone, random_instant, rng
from python.nodes.loaders._world import CURRENCY_EXPONENT, PER_USD, World

Rows = list[dict[str, object]]
# Partner code, payload version, currency.
PARTNERS = [("MKT_EU", 1, "EUR"), ("mkt_global", 2, "USD"), ("GIFTBOX", 1, "KWD")]


def clean_email(email: str | None) -> str | None:
    """The staging layer's email normalisation, for hashing buyer emails the way partners do."""

    if email is None:
        return None
    lowered = email.strip().lower()
    if "@" not in lowered or " " in lowered:
        return None
    local, _, domain = lowered.partition("@")
    return f"{local.split('+', 1)[0]}@{domain}"


def partner_listings(world: World) -> Rows:
    r = rng("partners__partner_listings")
    log = ChangeLog("partners__partner_listings")
    statuses = [
        "Live",
        "active",
        "on sale",
        "on_sale",
        "out_of_stock",
        "error: image too small",
        "pending review",
        "archived",
        "draft",
        "mystery",
    ]
    count = 0
    for code, _, _ in PARTNERS:
        for variant in r.sample(world.variants, k=min(len(world.variants), 30 * world.scale)):
            count += 1
            listed = random_instant(r, date(2025, 11, 1), date(2026, 2, 28))
            row = {
                "listing_id": f"LS{count:05d}",
                "partner_code": code,
                "sku": variant.raw_sku if chance(r, 0.95) else "UNKNOWN-SKU-01",
                "listing_status": r.choice(statuses[:2]),
                "listed_at": in_zone(listed, "UTC"),
                "terms": r.choice(
                    [
                        {"commission_pct": 12.5, "fulfilment": "partner"},
                        {"commission_pct": "15", "fulfilment": "merchant"},
                        None,
                    ]
                ),
            }
            versions = [(listed, row)]
            if chance(r, 0.4):
                changed = listed + timedelta(days=r.randint(5, 60))
                versions.append((changed, {**row, "listing_status": r.choice(statuses)}))
            emit(log, r, versions)
    return log.rows()


def partner_orders(world: World) -> Rows:
    r = rng("partners__partner_orders")
    log = EventLog("partners__partner_orders")
    buyers = [c for c in world.customers if clean_email(c.email)]
    count = 0
    for code, version, currency in PARTNERS:
        exponent = CURRENCY_EXPONENT[currency]
        for n in range(1, 100 * world.scale + 1):
            placed = random_instant(r, date(2026, 1, 1), date(2026, 3, 31))
            buyer = r.choice(buyers) if chance(r, 0.7) else None
            email = buyer.email if buyer else f"guest{n}@example.org"
            external_id = f"{code[:3].upper()}-{n:06d}"
            items = []
            for variant in r.sample(world.variants, k=r.randint(1, 3)):
                price = round(variant.base_usd * PER_USD[currency], 3 if exponent == 3 else 2)
                items.append((variant, r.choice([1, 1, 2]), price))
            total = round(sum(qty * price for _, qty, price in items), 3)
            if version == 1:
                payload: dict[str, object] = {
                    "orderId": external_id,
                    "buyer": {"email": email},
                    "total": {
                        "currency": currency.lower(),
                        "amount": f"{total:.3f}" if exponent == 3 else f"{total:.2f}",
                    },
                    "created": in_zone(placed, "Europe/Paris").isoformat(),
                    "lines": [
                        {"sku": v.raw_sku, "qty": qty, "price": str(price)}
                        for v, qty, price in items
                    ],
                }
                if chance(r, 0.1):
                    # The id moved into a nested object and the top-level key is a JSON null.
                    payload["orderId"] = None
                    payload["order"] = {"id": external_id}
            else:
                hashed = hashlib.sha256((clean_email(email) or "").encode()).hexdigest()
                payload = {
                    "order": {
                        "id": external_id,
                        "currency": currency,
                        "totals": {"gross_minor": int(round(total * 10**exponent))},
                        "placed_at_ms": int((placed - at(date(1970, 1, 1))).total_seconds() * 1000),
                    },
                    "customer": {"email_hash": hashed.upper() if chance(r, 0.1) else hashed},
                    "items": [
                        {
                            "seller_sku": v.raw_sku,
                            "quantity": qty,
                            "unit_price_minor": int(round(price * 10**exponent)),
                        }
                        for v, qty, price in items
                    ],
                }
                if chance(r, 0.1):
                    payload["orderId"] = None
            count += 1
            received = placed + timedelta(seconds=r.randint(5, 900))
            row = {
                "message_id": f"MSG{count:07d}",
                "partner_code": code,
                "schema_version": version,
                "received_at": received,
                "payload": payload,
            }
            log.add(row, received + timedelta(minutes=2))
            if chance(r, 0.03):
                log.add(row, received + timedelta(hours=4))
            if chance(r, 0.05):
                # A corrected copy of the same order arrives later under a new message id.
                count += 1
                later = received + timedelta(hours=r.randint(1, 30))
                log.add(
                    {**row, "message_id": f"MSG{count:07d}", "received_at": later},
                    later + timedelta(minutes=2),
                )
    return log.rows()


def partner_settlements(world: World) -> Rows:
    r = rng("partners__partner_settlements")
    log = ChangeLog("partners__partner_settlements")
    count = 0
    for code, _, currency in PARTNERS:
        exponent = CURRENCY_EXPONENT[currency]
        for month in (1, 2, 3):
            count += 1
            start = date(2026, month, 1)
            end = (date(2026, month + 1, 1) if month < 12 else date(2027, 1, 1)) - timedelta(days=1)
            gross = r.randint(2000, 9000) * 10**exponent
            statement: dict[str, object] = {
                "gross_sales_minor": gross,
                "commission_minor": int(gross * r.choice([0.12, 0.15, 0.125])),
                "fees": r.choice(
                    [
                        [
                            {"amount_minor": 120 * 10 ** max(exponent - 2, 0)},
                            {"amount_minor": "35"},
                        ],
                        [],
                        None,
                    ]
                ),
            }
            row = {
                "settlement_id": f"PS{count:04d}",
                "partner_code": code,
                "period_start": start,
                "period_end": end,
                "currency_code": currency,
                "statement": statement,
            }
            emit(log, r, [(at(end + timedelta(days=3)), row)])
    return log.rows()


TABLES = {
    "partners__partner_orders": partner_orders,
    "partners__partner_listings": partner_listings,
    "partners__partner_settlements": partner_settlements,
}
