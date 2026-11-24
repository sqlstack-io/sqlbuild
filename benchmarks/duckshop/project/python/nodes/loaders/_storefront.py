"""Storefront change logs: customers, orders, payments, refunds, loyalty, gift cards and more."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date, datetime, timedelta
from functools import lru_cache

from python.nodes.loaders._core import (
    ChangeLog,
    at,
    chance,
    emit,
    in_zone,
    random_instant,
    rng,
    weighted,
)
from python.nodes.loaders._world import (
    CITIES,
    GEO,
    STOREFRONTS,
    Order,
    World,
)

Rows = list[dict[str, object]]
TIERS = ["DUCKLING", "DRAKE", "GOLDEN_DUCK", "PLATINUM_DUCK"]
STREETS = ["Mill Lane", "Reed Street", "Harbour Road", "Canal Walk"]


def zone_of(world: World, customer_id: str) -> str:
    return STOREFRONTS[world.customer_by_id[customer_id].storefront].timezone


def customers(world: World) -> Rows:
    r = rng("storefront__customers")
    log = ChangeLog("storefront__customers")
    for c in world.customers:
        sf = STOREFRONTS[c.storefront]
        zone = sf.timezone
        language = {"DE": "de", "FR": "fr", "NL": "nl", "JP": "ja"}.get(c.storefront, "en")
        consents = r.choice(
            [["email"], ["email", "sms"], ["Email Marketing", "profiling"], [], None]
        )
        profile: dict[str, object] | None = {
            "preferences": {
                "language": language,
                "marketing_opt_in": r.choice([True, False, "true"]),
            },
            "consents": consents,
            "birth_year": r.choice([1975, 1988, "1992", "unknown", None]),
        }
        roll = r.random()
        if roll < 0.08:
            profile = {"preferences": None, "consents": consents}
        elif roll < 0.12:
            profile = None
        region = sf.region if chance(r, 0.9) else sf.region.lower()
        row = {
            "customer_id": c.customer_id,
            "email": c.email,
            "phone": c.phone,
            "full_name": f"{c.first_name} {c.last_name}"
            if chance(r, 0.95)
            else f"  {c.first_name.lower()}  {c.last_name.upper()}",
            "region_code": region,
            "storefront_code": c.storefront if chance(r, 0.9) else c.storefront.lower(),
            "created_at": in_zone(c.created_at, zone),
            "updated_at": in_zone(c.created_at, zone),
            "profile": profile,
        }
        versions = [(c.created_at, row)]
        if chance(r, 0.25):
            changed = c.created_at + timedelta(days=r.randint(1, 60), hours=r.randint(0, 23))
            if changed.date() <= date(2026, 3, 31):
                versions.append(
                    (
                        changed,
                        {
                            **row,
                            "updated_at": in_zone(changed, zone),
                            "phone": c.phone if chance(r, 0.5) else None,
                        },
                    )
                )
        delete_at = c.created_at + timedelta(days=r.randint(2, 30)) if c.deleted else None
        emit(log, r, versions, delete_at=delete_at)
    return log.rows()


def customer_addresses(world: World) -> Rows:
    r = rng("storefront__customer_addresses")
    log = ChangeLog("storefront__customer_addresses")
    for c in world.customers:
        for index, address_id in enumerate(c.addresses):
            country = c.storefront
            if index == 0 and len(c.addresses) > 1 and chance(r, 0.5):
                country = r.choice(list(CITIES))
            city, postcode = r.choice(CITIES[country])
            if chance(r, 0.3):
                postcode = postcode.lower().replace(" ", "").replace("-", "")
            lat, lon = GEO[country]
            geo: dict[str, object] | None = {
                "lat": lat + r.uniform(-0.2, 0.2),
                "lon": lon + r.uniform(-0.2, 0.2),
            }
            if chance(r, 0.1):
                geo = {"lat": f"{lat:.4f}", "lon": None}
            elif chance(r, 0.08):
                geo = None
            is_default = address_id == c.default_address
            valid_from = c.created_at + timedelta(minutes=5 + index)
            row = {
                "address_id": address_id,
                "customer_id": c.customer_id,
                "line1": f" {r.randint(1, 220)} {r.choice(STREETS)} ",
                "line2": r.choice([None, "", "Flat 2", "  "]),
                "city": r.choice([city, city.lower(), f" {city.upper()}"]),
                "postcode": postcode,
                "country_code": country if chance(r, 0.85) else country.lower(),
                "is_default": True if is_default else r.choice([False, None]),
                "valid_from": in_zone(valid_from, STOREFRONTS[c.storefront].timezone),
                "geo": geo,
            }
            delete_at = None
            if not is_default and chance(r, 0.3):
                delete_at = valid_from + timedelta(days=r.randint(10, 90))
                if delete_at.date() > date(2026, 3, 31):
                    delete_at = None
            emit(log, r, [(valid_from, row)], delete_at=delete_at)
    return log.rows()


def customer_consents(world: World) -> Rows:
    r = rng("storefront__customer_consents")
    log = ChangeLog("storefront__customer_consents")
    count = 0
    for c in world.customers:
        if not chance(r, 0.55):
            continue
        for consent_type in r.sample(
            ["email-marketing", "SMS marketing", "profiling", "email"], k=r.randint(1, 2)
        ):
            count += 1
            captured = c.created_at + timedelta(minutes=r.randint(1, 3000))
            row = {
                "consent_id": f"CS{count:06d}",
                "customer_id": c.customer_id,
                "consent_type": consent_type,
                "granted": r.choice([True, True, False, None]),
                "captured_at": in_zone(captured, zone_of(world, c.customer_id)),
                "source_context": r.choice(
                    [
                        {"surface": "checkout", "policy_url": "https://example.com/privacy/v3.2"},
                        {"surface": "account", "policy_url": "https://example.com/privacy/v4"},
                        {"policy_url": None},
                        None,
                    ]
                ),
            }
            versions = [(captured, row)]
            if chance(r, 0.15):
                withdrawn = captured + timedelta(days=r.randint(5, 40))
                versions.append(
                    (withdrawn, {**row, "granted": False, "captured_at": in_zone(withdrawn, "UTC")})
                )
            emit(log, r, versions)
    return log.rows()


@lru_cache(maxsize=4)
def _loyalty_members(scale: int) -> tuple[str, ...]:
    from python.nodes.loaders._world import build_world

    r = rng("loyalty_members")
    return tuple(
        c.customer_id for c in build_world(scale).customers if not c.deleted and chance(r, 0.6)
    )


def loyalty_accounts(world: World) -> Rows:
    r = rng("storefront__loyalty_accounts")
    log = ChangeLog("storefront__loyalty_accounts")
    members = set(_loyalty_members(world.scale))
    for c in world.customers:
        if c.customer_id not in members:
            continue
        zone = zone_of(world, c.customer_id)
        enrolled = c.created_at + timedelta(minutes=r.randint(1, 600))
        level = 0
        history: list[dict[str, object]] = [
            {"tier": "duckling", "at": in_zone(enrolled, zone).isoformat()}
        ]
        when = enrolled
        for _ in range(r.randint(0, 3)):
            when = when + timedelta(days=r.randint(10, 90))
            if when.date() > date(2026, 3, 31):
                break
            level = max(0, min(3, level + r.choice([1, 1, -1, 0])))
            stamp: str = in_zone(when, zone).isoformat()
            if chance(r, 0.1):
                stamp = when.strftime("%d/%m/%Y")
            history.append(
                {"tier": TIERS[level].lower() if chance(r, 0.5) else TIERS[level], "at": stamp}
            )
        row = {
            "loyalty_account_id": f"L{c.customer_id[1:]}",
            "customer_id": c.customer_id,
            "tier_code": TIERS[level] if chance(r, 0.9) else TIERS[level].lower(),
            "points_balance": r.randint(-20, 4000) if chance(r, 0.95) else None,
            "enrolled_at": in_zone(enrolled, zone),
            "tier_history": history,
        }
        emit(
            log,
            r,
            [(enrolled, row)]
            + ([(when, {**row, "points_balance": r.randint(0, 5000)})] if len(history) > 1 else []),
        )
    return log.rows()


def loyalty_ledger(world: World) -> Rows:
    r = rng("storefront__loyalty_ledger")
    log = ChangeLog("storefront__loyalty_ledger")
    members = set(_loyalty_members(world.scale))
    count = 0
    for order in world.orders:
        if order.customer_id not in members or order.status in ("pending", "cancelled"):
            continue
        count += 1
        paid = order.time_of("paid") or order.placed_at
        row = {
            "entry_id": f"LE{count:06d}",
            "loyalty_account_id": f"L{order.customer_id[1:]}",
            "points_delta": max(1, order.net_minor // (10**order.exponent)),
            "reason_code": r.choice(["earn", "EARN", "order_earn"]),
            "order_id": order.order_id,
            "posted_at": in_zone(paid, "UTC"),
            "metadata": {"expires_on": (paid.date() + timedelta(days=365)).isoformat()},
        }
        emit(log, r, [(paid, row)])
        if chance(r, 0.12):
            count += 1
            when = paid + timedelta(days=r.randint(1, 20))
            if when.date() <= date(2026, 3, 31):
                emit(
                    log,
                    r,
                    [
                        (
                            when,
                            {
                                "entry_id": f"LE{count:06d}",
                                "loyalty_account_id": f"L{order.customer_id[1:]}",
                                "points_delta": -r.choice([100, 250, 500]),
                                "reason_code": "redeem",
                                "order_id": "",
                                "posted_at": in_zone(when, zone_of(world, order.customer_id)),
                                "metadata": {
                                    "campaign": r.choice(["spring-splash", None, " "]),
                                    "expires_on": "not-a-date",
                                },
                            },
                        )
                    ],
                )
    for customer_id in sorted(members)[::7]:
        count += 1
        when = at(date(2026, 3, 1), 2)
        emit(
            log,
            r,
            [
                (
                    when,
                    {
                        "entry_id": f"LE{count:06d}",
                        "loyalty_account_id": f"L{customer_id[1:]}",
                        "points_delta": -r.randint(1, 40),
                        "reason_code": "expire",
                        "order_id": None,
                        "posted_at": in_zone(when, "UTC"),
                        "metadata": None,
                    },
                )
            ],
        )
    return log.rows()


def wishlists(world: World) -> Rows:
    r = rng("storefront__wishlists")
    log = ChangeLog("storefront__wishlists")
    count = 0
    for c in world.customers:
        if c.deleted or not chance(r, 0.3):
            continue
        count += 1
        created = c.created_at + timedelta(days=r.randint(0, 20))
        if created.date() > date(2026, 3, 30):
            created = c.created_at
        items: list[object] = []
        for variant in r.sample(world.variants, k=r.randint(0, 5)):
            added = created + timedelta(hours=r.randint(0, 400))
            items.append({"sku": variant.raw_sku, "added_at": in_zone(added, "UTC").isoformat()})
        if chance(r, 0.1):
            items.append("DUCK-MAL-01")
        if chance(r, 0.1):
            items.append({"sku": None, "added_at": None})
        row = {
            "wishlist_id": f"W{count:05d}",
            "customer_id": c.customer_id,
            "created_at": in_zone(created, zone_of(world, c.customer_id)),
            "items": items if items or chance(r, 0.5) else None,
            "is_public": r.choice([True, False, None]),
        }
        versions = [(created, row)]
        delete_at = created + timedelta(days=3) if chance(r, 0.05) else None
        emit(log, r, versions, delete_at=delete_at)
    return log.rows()


@dataclass(frozen=True)
class GiftCard:
    gift_card_id: str
    currency: str
    exponent: int
    initial_minor: int
    issued_at: datetime
    expires_on: date
    purchaser: str | None
    promotional: bool


@dataclass(frozen=True)
class GiftTender:
    order_id: str
    gift_card_id: str
    amount_minor: int
    at: datetime


@lru_cache(maxsize=4)
def gift_cards_and_tender(scale: int) -> tuple[tuple[GiftCard, ...], tuple[GiftTender, ...]]:
    """Gift cards and the orders part-paid with them; payments and refunds read this too."""

    from python.nodes.loaders._world import build_world

    world = build_world(scale)
    r = rng("gift_cards")
    cards: list[GiftCard] = []
    live = [c for c in world.customers if not c.deleted]
    for i in range(1, 60 * scale + 1):
        purchaser = r.choice(live)
        sf = STOREFRONTS[purchaser.storefront]
        issued = random_instant(r, date(2025, 3, 1), date(2026, 3, 20))
        amount = r.choice([25, 50, 100]) * 10**sf.exponent
        if sf.exponent == 0:
            amount = r.choice([3000, 5000, 10000])
        expires = issued.date() + timedelta(days=r.choice([180, 365, 730]))
        cards.append(
            GiftCard(
                f"G{i:05d}",
                sf.currency,
                sf.exponent,
                amount,
                issued,
                expires,
                None if chance(r, 0.2) else purchaser.customer_id,
                chance(r, 0.15),
            )
        )
    tender: list[GiftTender] = []
    remaining = {card.gift_card_id: card.initial_minor for card in cards}
    for order in world.orders:
        if order.status in ("pending", "cancelled") or not chance(r, 0.05):
            continue
        candidates = [
            card
            for card in cards
            if card.currency == order.currency
            and card.issued_at < order.placed_at
            and card.expires_on > order.placed_at.date()
            and remaining[card.gift_card_id] > 0
        ]
        if not candidates:
            continue
        card = r.choice(candidates)
        amount = min(remaining[card.gift_card_id], order.total_minor)
        if amount <= 0:
            continue
        remaining[card.gift_card_id] -= amount
        tender.append(
            GiftTender(
                order.order_id, card.gift_card_id, amount, order.time_of("paid") or order.placed_at
            )
        )
    return tuple(cards), tuple(tender)


def gift_cards(world: World) -> Rows:
    r = rng("storefront__gift_cards")
    log = ChangeLog("storefront__gift_cards")
    cards, _ = gift_cards_and_tender(world.scale)
    for card in cards:
        suffix = f"{r.randint(0, 9999):04d}"
        row = {
            "gift_card_id": card.gift_card_id,
            "code_masked": r.choice(
                [f"XXXX-XXXX-{suffix}", f"****-****-{suffix}", f"xxxx xxxx {suffix}"]
            ),
            "currency_code": card.currency if chance(r, 0.9) else card.currency.lower(),
            "initial_amount_minor": card.initial_minor,
            "issued_at": in_zone(card.issued_at, "UTC"),
            "expires_on": card.expires_on,
            "purchaser_customer_id": card.purchaser,
            "details": r.choice(
                [
                    {
                        "promotional": card.promotional,
                        "design": r.choice(["pond", "golden-egg", None]),
                    },
                    {"design": "  "},
                    None,
                ]
            ),
        }
        emit(log, r, [(card.issued_at, row)])
    return log.rows()


def gift_card_transactions(world: World) -> Rows:
    r = rng("storefront__gift_card_transactions")
    log = ChangeLog("storefront__gift_card_transactions")
    cards, tender = gift_cards_and_tender(world.scale)
    count = 0
    for t in tender:
        count += 1
        row = {
            "transaction_id": f"GT{count:06d}",
            "gift_card_id": t.gift_card_id,
            "order_id": t.order_id,
            "amount_minor": t.amount_minor if chance(r, 0.7) else -t.amount_minor,
            "transaction_type": r.choice(["redeem", "REDEEM", "Redeem"]),
            "occurred_at": in_zone(t.at, "UTC"),
            "context": r.choice([{"channel": "WEB"}, {"channel": "app"}, None]),
        }
        emit(log, r, [(t.at, row)])
    for card in cards:
        if chance(r, 0.1):
            count += 1
            when = card.issued_at + timedelta(days=r.randint(5, 60))
            if when.date() < date(2026, 3, 31):
                emit(
                    log,
                    r,
                    [
                        (
                            when,
                            {
                                "transaction_id": f"GT{count:06d}",
                                "gift_card_id": card.gift_card_id,
                                "order_id": None,
                                "amount_minor": card.initial_minor // 2,
                                "transaction_type": "top-up",
                                "occurred_at": in_zone(when, "UTC"),
                                "context": {
                                    "channel": "store",
                                    "terminal": {"id": f"T-{r.randint(1, 9)}"},
                                },
                            },
                        )
                    ],
                )
        if card.expires_on <= date(2026, 3, 31):
            count += 1
            when = at(card.expires_on, 0, 5)
            emit(
                log,
                r,
                [
                    (
                        when,
                        {
                            "transaction_id": f"GT{count:06d}",
                            "gift_card_id": card.gift_card_id,
                            "order_id": None,
                            "amount_minor": card.initial_minor // 3,
                            "transaction_type": "Expire",
                            "occurred_at": in_zone(when, "UTC"),
                            "context": {"terminal": {"id": None}},
                        },
                    )
                ],
            )
    first = tender[0] if tender else None
    if first is not None:
        count += 1
        when = first.at + timedelta(hours=2)
        emit(
            log,
            r,
            [
                (
                    when,
                    {
                        "transaction_id": f"GT{count:06d}",
                        "gift_card_id": first.gift_card_id,
                        "order_id": first.order_id,
                        "amount_minor": 100,
                        "transaction_type": "reversal",
                        "occurred_at": in_zone(when, "UTC"),
                        "context": None,
                    },
                )
            ],
        )
    return log.rows()


def _order_row(world: World, order: Order, status: str, r) -> dict[str, object]:
    sf = STOREFRONTS[order.storefront]
    attributes: dict[str, object] = {"line_skus": [line.sku for line in order.lines]}
    if order.channel is not None:
        attributes["channel"] = order.channel if chance(r, 0.9) else order.channel.upper()
    if chance(r, 0.06):
        attributes["gift"] = {"is_gift": True, "message": "Happy splash day!"}
    if order.utm is not None:
        source, medium, campaign = order.utm
        attributes["utm"] = {"source": source, "medium": medium, "campaign": campaign or None}
    attributes["device"] = {"type": r.choice(["desktop", "mobile", "tablet", None])}
    return {
        "order_id": order.order_id,
        "customer_id": order.customer_id,
        "storefront_code": order.storefront if chance(r, 0.95) else order.storefront.lower(),
        "currency_code": order.currency,
        "placed_at": in_zone(order.placed_at, sf.timezone),
        "status": status if chance(r, 0.9) else status.upper(),
        "shipping_address_id": order.address_id,
        "attributes": attributes,
    }


def orders(world: World) -> Rows:
    r = rng("storefront__orders")
    log = ChangeLog("storefront__orders")
    for order in world.orders:
        base = _order_row(world, order, "pending", r)
        versions = [(when, {**base, "status": status}) for status, when in order.status_times]
        emit(log, r, versions, late_share=0.05)
    return log.rows()


def order_lines(world: World) -> Rows:
    r = rng("storefront__order_lines")
    log = ChangeLog("storefront__order_lines")
    for order in world.orders:
        for line in order.lines:
            discounts: object
            if line.discount_minor:
                amount: object = line.discount_minor if chance(r, 0.8) else str(line.discount_minor)
                discounts = [
                    {
                        "code": order.promo_code.lower() if order.promo_code else None,
                        "kind": "promotion",
                        "amount_minor": amount,
                    }
                ]
            else:
                discounts = r.choice([[], None, None, {"note": "no discount"}])
            variant_raw = world.variant_by_sku[line.sku].raw_sku
            row = {
                "order_line_id": line.order_line_id,
                "order_id": order.order_id,
                "line_no": line.line_no,
                "sku": variant_raw if chance(r, 0.85) else f" {line.sku.lower()} ",
                "quantity": line.quantity,
                "unit_amount_minor": line.unit_minor,
                "tax_amount_minor": line.tax_minor if chance(r, 0.97) else None,
                "discounts": discounts,
            }
            emit(log, r, [(order.placed_at, row)])
    return log.rows()


def order_status_events(world: World) -> Rows:
    r = rng("storefront__order_status_events")
    log = ChangeLog("storefront__order_status_events")
    count = 0
    for order in world.orders:
        previous = None
        for status, when in order.status_times:
            count += 1
            actor = {
                "pending": {"type": "customer", "id": order.customer_id},
                "paid": None,
                "packed": {"type": "warehouse", "id": f"picker-{r.randint(1, 12)}"},
                "shipped": {"type": "system"},
                "delivered": {"id": "carrier-webhook"},
                "cancelled": {"type": "agent", "id": f"AG{r.randint(1, 12):03d}"},
                "returned": {"type": "agent", "id": None},
            }[status]
            row = {
                "event_id": f"SE{count:07d}",
                "order_id": order.order_id,
                "from_status": previous,
                "to_status": status if chance(r, 0.85) else status.upper(),
                "changed_at": in_zone(when, STOREFRONTS[order.storefront].timezone),
                "actor": actor,
            }
            emit(log, r, [(when, row)], late_share=0.06, duplicate_share=0.03)
            previous = status
    return log.rows()


DECLINES = [
    "Declined: insufficient funds (DN051)",
    "Card reported stolen - pickup (FR043)",
    "3DS authentication failed (AU201)",
    "Issuer unavailable, try again (TE091)",
    "Do not honour",
]
METHODS = {
    "card": 50,
    "Credit Card": 10,
    "debit-card": 10,
    "phone wallet": 12,
    "browser_wallet": 3,
    "bank transfer": 5,
    "sepa": 3,
    "pay later": 4,
}


@lru_cache(maxsize=4)
def payments_plan(scale: int) -> tuple[tuple[str, str, str, int, datetime, str], ...]:
    """(payment_id, order_id, method, amount_minor, authorised_at, gateway status) per attempt."""

    from python.nodes.loaders._world import build_world

    world = build_world(scale)
    r = rng("payments_plan")
    _, tender = gift_cards_and_tender(scale)
    tendered = {t.order_id: t.amount_minor for t in tender}
    plan: list[tuple[str, str, str, int, datetime, str]] = []
    count = 0
    for order in world.orders:
        due = order.total_minor - tendered.get(order.order_id, 0)
        if due <= 0:
            continue
        method = str(weighted(r, METHODS))
        when = order.placed_at + timedelta(seconds=r.randint(5, 90))
        for _ in range(weighted(r, {0: 85, 1: 11, 2: 4})):  # type: ignore[call-overload]
            count += 1
            plan.append((f"PAY{count:07d}", order.order_id, method, due, when, "declined"))
            when = when + timedelta(minutes=r.randint(1, 15))
        count += 1
        status = {"pending": "authorised", "cancelled": "voided"}.get(order.status, "captured")
        if status == "captured" and chance(r, 0.3):
            status = "settled"
        plan.append((f"PAY{count:07d}", order.order_id, method, due, when, status))
    return tuple(plan)


def gateway_reference(payment_id: str) -> str:
    """The numeric reference the payment gateway echoes back, also quoted in settlement reports."""

    return str(500000 + int(payment_id[3:]))


def provider_for(method: str) -> str:
    lowered = method.lower()
    if "wallet" in lowered:
        return "PROVIDER_B"
    if "bank" in lowered or "sepa" in lowered:
        return "PROVIDER_C"
    return "PROVIDER_A"


def payments(world: World) -> Rows:
    r = rng("storefront__payments")
    log = ChangeLog("storefront__payments")
    orders_by_id = world.order_by_id
    for payment_id, order_id, method, amount, when, status in payments_plan(world.scale):
        order = orders_by_id[order_id]
        is_card = "card" in method.lower()
        response: dict[str, object] = {
            "status": {
                "code": status,
                "message": "Approved" if status != "declined" else r.choice(DECLINES),
            },
            "risk": {
                "score": r.choice([f"{r.uniform(0, 99):.2f}", round(r.uniform(0, 99), 6), "n/a"])
            },
            "reference": gateway_reference(payment_id)
            if chance(r, 0.6)
            else (
                int(gateway_reference(payment_id)) if chance(r, 0.9) else f"REF-{payment_id[-4:]}"
            ),
        }
        if is_card:
            response["three_ds"] = {"outcome": r.choice(["y", "N", "frictionless", None])}
        if chance(r, 0.05):
            response["status"] = status.upper()
        captured_at = None
        if status in ("captured", "settled"):
            captured_at = in_zone(when + timedelta(minutes=r.randint(0, 120)), "UTC")
        row = {
            "payment_id": payment_id,
            "order_id": order_id,
            "method": method,
            "amount_minor": amount,
            "currency_code": order.currency,
            "authorised_at": in_zone(when, STOREFRONTS[order.storefront].timezone),
            "captured_at": captured_at,
            "card_bin": (str(r.randint(400000, 559999)) if chance(r, 0.9) else "4876-53")
            if is_card
            else None,
            "gateway_response": response,
        }
        versions = [(when, row)]
        emit(log, r, versions)
    return log.rows()


REFUND_REASONS = [
    "Item arrived DAMAGED - wing snapped off",
    "Broken in transit, box crushed",
    "Parcel never arrived",
    "Delivery was 9 days late",
    "Chargeback raised by card issuer",
    "Customer changed their mind",
    "  Wrong colour sent (ordered teal, got yellow)  ",
    "Defective squeaker",
]


@lru_cache(maxsize=4)
def refunds_plan(scale: int) -> tuple[tuple[str, str, str, int, datetime, str], ...]:
    """(refund_id, payment_id, order_id, amount_minor, refunded_at, reason) for refunded orders."""

    from python.nodes.loaders._world import build_world

    world = build_world(scale)
    r = rng("refunds_plan")
    successful = {
        order_id: (payment_id, amount)
        for payment_id, order_id, _, amount, _, status in payments_plan(scale)
        if status in ("captured", "settled")
    }
    plan: list[tuple[str, str, str, int, datetime, str]] = []
    count = 0
    for order in world.orders:
        if order.order_id not in successful:
            continue
        payment_id, amount = successful[order.order_id]
        returned = order.time_of("returned")
        delivered = order.time_of("delivered")
        if returned is not None:
            count += 1
            full = chance(r, 0.6)
            plan.append(
                (
                    f"RF{count:06d}",
                    payment_id,
                    order.order_id,
                    amount if full else amount // 2,
                    returned + timedelta(days=r.randint(1, 4)),
                    r.choice(REFUND_REASONS),
                )
            )
        elif delivered is not None and chance(r, 0.03):
            count += 1
            when = delivered + timedelta(days=r.randint(1, 10))
            if when.date() <= date(2026, 3, 31):
                plan.append(
                    (
                        f"RF{count:06d}",
                        payment_id,
                        order.order_id,
                        max(1, amount // 4),
                        when,
                        r.choice(REFUND_REASONS),
                    )
                )
    return tuple(plan)


def refunds(world: World) -> Rows:
    r = rng("storefront__refunds")
    log = ChangeLog("storefront__refunds")
    orders_by_id = world.order_by_id
    for refund_id, payment_id, order_id, amount, when, reason in refunds_plan(world.scale):
        order = orders_by_id[order_id]
        items: list[object] | None = [
            {"order_line_id": line.order_line_id, "qty": line.quantity} for line in order.lines[:2]
        ]
        if chance(r, 0.2):
            items = None
        row = {
            "refund_id": refund_id,
            "payment_id": payment_id,
            "amount_minor": amount if chance(r, 0.8) else -amount,
            "reason_text": reason,
            "refunded_at": in_zone(when, STOREFRONTS[order.storefront].timezone),
            "initiated_by": r.choice(["Agent", "customer", None, "SYSTEM"]),
            "items": items,
        }
        emit(log, r, [(when, row)])
    return log.rows()


def promotions(world: World) -> Rows:
    r = rng("storefront__promotions")
    log = ChangeLog("storefront__promotions")
    rules = {
        "SPLASH10": {
            "eligibility": {"min_order_minor": 2000, "regions": ["EU", "US"]},
            "tiers": [{"pct": 10}],
        },
        "POND15": {
            "eligibility": {"regions": ["EU", "APAC", "ANTARCTICA"]},
            "tiers": [{"pct": 10, "min_minor": 0}, {"pct": 15.5, "min_minor": 10000}],
        },
        "GOLDENEGG": {"tiers": [{"pct": 20}], "eligibility": None},
        "WELCOME5": None,
    }
    names = {
        "SPLASH10": "Spring splash",
        "POND15": "Pond week",
        "GOLDENEGG": "Golden egg hunt",
        "WELCOME5": "Welcome to the flock",
    }
    for code, (starts, ends, _) in world.promotions.items():
        row = {
            "promo_code": f" {code.lower()} " if code == "POND15" else code,
            "promo_name": names[code],
            "starts_at": in_zone(starts, "Europe/London"),
            "ends_at": in_zone(ends, "Europe/London"),
            "is_stackable": None if code == "WELCOME5" else code == "GOLDENEGG",
            "rules": rules[code],
        }
        versions = [(starts - timedelta(days=5), row)]
        if code == "POND15":
            versions.append(
                (starts + timedelta(days=3), {**row, "promo_name": "Pond week (extended)"})
            )
        emit(log, r, versions)
    withdrawn = {
        "promo_code": "WITHDRAWN50",
        "promo_name": "Withdrawn test code",
        "starts_at": in_zone(at(date(2026, 2, 1)), "UTC"),
        "ends_at": None,
        "is_stackable": True,
        "rules": {"tiers": [{"pct": 50}]},
    }
    emit(log, r, [(at(date(2026, 1, 30)), withdrawn)], delete_at=at(date(2026, 1, 31)))
    return log.rows()


def promotion_redemptions(world: World) -> Rows:
    r = rng("storefront__promotion_redemptions")
    log = ChangeLog("storefront__promotion_redemptions")
    count = 0
    for order in world.orders:
        if order.promo_code is None:
            continue
        count += 1
        discount = sum(line.discount_minor for line in order.lines)
        row = {
            "redemption_id": f"PR{count:06d}",
            "promo_code": order.promo_code if chance(r, 0.8) else f"{order.promo_code.lower()} ",
            "order_id": order.order_id,
            "discount_minor": discount if chance(r, 0.8) else -discount,
            "redeemed_at": in_zone(order.placed_at, STOREFRONTS[order.storefront].timezone),
            "context": r.choice([{"entry": "CODE"}, {"entry": "link"}, None]),
        }
        emit(log, r, [(order.placed_at, row)])
        if chance(r, 0.05):
            count += 1
            emit(
                log,
                r,
                [
                    (
                        order.placed_at + timedelta(seconds=30),
                        {
                            **row,
                            "redemption_id": f"PR{count:06d}",
                            "promo_code": "WELCOME5",
                            "discount_minor": 0,
                            "redeemed_at": in_zone(order.placed_at + timedelta(seconds=30), "UTC"),
                        },
                    )
                ],
            )
    return log.rows()


def carts(world: World) -> Rows:
    r = rng("storefront__carts")
    log = ChangeLog("storefront__carts")
    live = [c for c in world.customers if not c.deleted]
    for i in range(1, 700 * world.scale + 1):
        customer = r.choice(live) if chance(r, 0.6) else None
        created = random_instant(r, date(2026, 2, 1), date(2026, 3, 31))
        zone = STOREFRONTS[customer.storefront].timezone if customer else "UTC"
        items: list[dict[str, object]] = []
        for index, variant in enumerate(r.sample(world.variants, k=r.randint(0, 4))):
            qty: object = r.choice([1, 2, 1.0, "3", "two"])
            items.append(
                {
                    "sku": variant.raw_sku,
                    "qty": qty,
                    "unit_minor": r.choice(
                        [str(int(variant.base_usd * 100)), int(variant.base_usd * 100), None]
                    ),
                    "added_at": in_zone(created + timedelta(minutes=index * 3), zone).isoformat()
                    if chance(r, 0.95)
                    else "yesterday",
                }
            )
        status = weighted(r, {"open": 30, "abandoned": 45, "converted": 25})
        updated = created + timedelta(minutes=r.randint(1, 600))
        row = {
            "cart_id": f"CART{i:06d}",
            "customer_id": customer.customer_id if customer else None,
            "anonymous_id": (
                customer.anonymous_id if customer else f"anon-{r.getrandbits(48):012x}"
            ).upper()
            if chance(r, 0.1)
            else (customer.anonymous_id if customer else f"anon-{r.getrandbits(48):012x}"),
            "storefront_code": customer.storefront
            if customer
            else r.choice(list(STOREFRONTS)).lower(),
            "cart_status": status if chance(r, 0.9) else str(status).upper(),
            "created_at": in_zone(created, zone),
            "updated_at": in_zone(updated, zone),
            "contents": {"items": items, "coupon": r.choice([None, "pond15", "", " splash10"])}
            if items
            else r.choice([{"items": []}, None]),
        }
        versions = [
            (created, {**row, "cart_status": "open", "updated_at": in_zone(created, zone)}),
            (updated, row),
        ]
        delete_at = updated + timedelta(days=1) if chance(r, 0.03) else None
        emit(log, r, versions, delete_at=delete_at)
    return log.rows()


REVIEW_BODIES = [
    "Love this duck, great colours and very sturdy.",
    "Arrived broken, the beak snapped off. Very disappointed.",
    "Perfect gift! See photos at https://photos.example.com/duck123",
    "Cheap and flimsy, will return.",
    "Beautiful carving.\nThe paint is excellent.",
    "ok",
    "Rust on the hook after a week in the pond.",
    "",
]


def reviews(world: World) -> Rows:
    r = rng("storefront__reviews")
    log = ChangeLog("storefront__reviews")
    count = 0
    for order in world.orders:
        delivered = order.time_of("delivered")
        if delivered is None or not chance(r, 0.18):
            continue
        line = r.choice(order.lines)
        count += 1
        when = delivered + timedelta(days=r.randint(1, 14))
        if when.date() > date(2026, 3, 31):
            continue
        body = r.choice(REVIEW_BODIES)
        row = {
            "review_id": f"RV{count:05d}",
            "sku": world.variant_by_sku[line.sku].raw_sku,
            "customer_id": order.customer_id if chance(r, 0.95) else None,
            "rating": r.choice([5, 5, 4, 4, 3, 2, 1, 0, 6]),
            "title": r.choice(["  Great duck ", "Not as pictured", None, "Ducky delight"]),
            "body": body,
            "submitted_at": in_zone(when, STOREFRONTS[order.storefront].timezone),
            "moderation": r.choice(
                [
                    {"status": "approved", "flags": []},
                    {"status": "REJECTED", "flags": ["url"]},
                    {"flags": ["profanity", "url"]},
                    None,
                ]
            ),
        }
        emit(log, r, [(when, row)])
    count += 1
    emit(
        log,
        r,
        [
            (
                at(date(2026, 3, 2)),
                {
                    "review_id": f"RV{count:05d}",
                    "sku": world.variants[0].raw_sku,
                    "customer_id": None,
                    "rating": 1,
                    "title": "spam",
                    "body": "Visit http://spam.example.net now",
                    "submitted_at": in_zone(at(date(2026, 3, 2)), "UTC"),
                    "moderation": {"status": "rejected", "flags": ["spam"]},
                },
            )
        ],
        delete_at=at(date(2026, 3, 3)),
    )
    return log.rows()


@lru_cache(maxsize=4)
def subscription_plan(
    scale: int,
) -> tuple[tuple[str, str, str, datetime, list[tuple[str, datetime]]], ...]:
    from python.nodes.loaders._world import build_world

    world = build_world(scale)
    r = rng("subscription_plan")
    consumables = [v for v in world.variants if v.family in ("POND", "TOWEL")]
    live = [c for c in world.customers if not c.deleted]
    plan = []
    for i in range(1, 80 * scale + 1):
        customer = r.choice(live)
        started = max(customer.created_at, at(date(2025, 10, 1))) + timedelta(
            days=r.randint(0, 120)
        )
        if started.date() > date(2026, 3, 25):
            started = at(date(2026, 3, 1), 10)
        events: list[tuple[str, datetime]] = []
        when = started
        state = "active"
        for _ in range(r.randint(0, 4)):
            when = when + timedelta(days=r.randint(5, 40), hours=r.randint(0, 23))
            if when.date() > date(2026, 3, 31):
                break
            if state == "active":
                kind = str(weighted(r, {"PAUSE": 35, "SKIP": 25, "CANCEL": 15, "RENEW": 25}))
            elif state == "paused":
                kind = str(weighted(r, {"RESUME": 70, "CANCEL": 30}))
            else:
                break
            events.append((kind, when))
            state = {"PAUSE": "paused", "RESUME": "active", "CANCEL": "cancelled"}.get(kind, state)
        plan.append(
            (f"SUB{i:05d}", customer.customer_id, r.choice(consumables).raw_sku, started, events)
        )
    return tuple(plan)


def subscriptions(world: World) -> Rows:
    r = rng("storefront__subscriptions")
    log = ChangeLog("storefront__subscriptions")
    for subscription_id, customer_id, sku, started, events in subscription_plan(world.scale):
        sf = STOREFRONTS[world.customer_by_id[customer_id].storefront]
        zone = sf.timezone
        interval = r.choice([30, 30, 60, None, 90])
        amount = {0: 2500, 2: 1999, 3: 6500}[sf.exponent]
        next_bill = started.date() + timedelta(days=interval or 30)
        row = {
            "subscription_id": subscription_id,
            "customer_id": customer_id,
            "plan_code": r.choice(["pond-care-monthly", "TOWEL-REFRESH"]),
            "sku": sku,
            "interval_days": interval,
            "subscription_status": "active",
            "started_at": in_zone(started, zone),
            "cancelled_at": None,
            "billing": {
                "amount_minor": amount,
                "currency": sf.currency.lower(),
                "next_bill_on": next_bill.isoformat()
                if chance(r, 0.9)
                else next_bill.strftime("%d/%m/%Y"),
            },
        }
        versions = [(started, row)]
        state = "active"
        for kind, when in events:
            state = {"PAUSE": "paused", "RESUME": "active", "CANCEL": "cancelled"}.get(kind, state)
            current = {
                **versions[-1][1],
                "subscription_status": state if chance(r, 0.9) else state.upper(),
            }
            if kind == "CANCEL":
                current["cancelled_at"] = in_zone(when, zone)
            versions.append((when, current))
        emit(log, r, versions)
    return log.rows()


def subscription_events(world: World) -> Rows:
    r = rng("storefront__subscription_events")
    log = ChangeLog("storefront__subscription_events")
    count = 0
    for subscription_id, customer_id, _, _, events in subscription_plan(world.scale):
        zone = STOREFRONTS[world.customer_by_id[customer_id].storefront].timezone
        for kind, when in events:
            count += 1
            payload: dict[str, object] | None = None
            if kind == "PAUSE":
                payload = {"pause_days": r.choice([14, "30", "a month"])}
            elif kind == "SKIP":
                payload = {"skip_reason": r.choice(["too many towels", None, " "])}
            row = {
                "event_id": f"SUBE{count:06d}",
                "subscription_id": subscription_id,
                "event_type": kind if chance(r, 0.85) else kind.lower(),
                "occurred_at": in_zone(when, zone),
                "payload": payload,
            }
            emit(log, r, [(when, row)])
    return log.rows()


def store_credit_ledger(world: World) -> Rows:
    r = rng("storefront__store_credit_ledger")
    log = ChangeLog("storefront__store_credit_ledger")
    orders_by_id = world.order_by_id
    count = 0
    for refund_id, _, order_id, amount, when, _ in refunds_plan(world.scale):
        if not chance(r, 0.35):
            continue
        order = orders_by_id[order_id]
        count += 1
        emit(
            log,
            r,
            [
                (
                    when,
                    {
                        "entry_id": f"SC{count:06d}",
                        "customer_id": order.customer_id,
                        "currency_code": order.currency.lower()
                        if chance(r, 0.1)
                        else order.currency,
                        "amount_minor": amount,
                        "reason_code": r.choice(
                            ["return credit", "RETURN-CREDIT", "return_credit"]
                        ),
                        "posted_at": in_zone(when, "UTC"),
                        "context": {
                            "order_id": order_id,
                            "refund_id": refund_id,
                            "expires_on": (when.date() + timedelta(days=365)).isoformat(),
                        },
                    },
                )
            ],
        )
        if chance(r, 0.4):
            count += 1
            spent = when + timedelta(days=r.randint(2, 20))
            if spent.date() <= date(2026, 3, 31):
                emit(
                    log,
                    r,
                    [
                        (
                            spent,
                            {
                                "entry_id": f"SC{count:06d}",
                                "customer_id": order.customer_id,
                                "currency_code": order.currency,
                                "amount_minor": -min(amount, amount // 2 + 1),
                                "reason_code": "spent on order",
                                "posted_at": in_zone(spent, "UTC"),
                                "context": {"order_id": None},
                            },
                        )
                    ],
                )
    for c in world.customers[::40]:
        if c.deleted:
            continue
        count += 1
        when = max(c.created_at, at(date(2026, 1, 15))) + timedelta(days=3)
        emit(
            log,
            r,
            [
                (
                    when,
                    {
                        "entry_id": f"SC{count:06d}",
                        "customer_id": c.customer_id,
                        "currency_code": STOREFRONTS[c.storefront].currency,
                        "amount_minor": 5 * 10 ** STOREFRONTS[c.storefront].exponent,
                        "reason_code": "goodwill gesture!",
                        "posted_at": in_zone(when, "UTC"),
                        "context": None,
                    },
                )
            ],
        )
    return log.rows()


TABLES = {
    "storefront__customers": customers,
    "storefront__customer_addresses": customer_addresses,
    "storefront__customer_consents": customer_consents,
    "storefront__loyalty_accounts": loyalty_accounts,
    "storefront__loyalty_ledger": loyalty_ledger,
    "storefront__wishlists": wishlists,
    "storefront__gift_cards": gift_cards,
    "storefront__gift_card_transactions": gift_card_transactions,
    "storefront__orders": orders,
    "storefront__order_lines": order_lines,
    "storefront__order_status_events": order_status_events,
    "storefront__payments": payments,
    "storefront__refunds": refunds,
    "storefront__promotions": promotions,
    "storefront__promotion_redemptions": promotion_redemptions,
    "storefront__carts": carts,
    "storefront__reviews": reviews,
    "storefront__subscriptions": subscriptions,
    "storefront__subscription_events": subscription_events,
    "storefront__store_credit_ledger": store_credit_ledger,
}
